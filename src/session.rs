use crate::{
    config::ServerConfig,
    error::{OpenMcpGdbError, Result},
    gdb::GdbBackend,
    protocol::{DebuggerResponse, DebuggerState},
};
use std::{collections::BTreeMap, path::PathBuf, thread};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum ToolOperation {
    Execute { executable_path: String },
    Run,
    TargetRemote { ip: String, port: u16 },
    SetThread { id: i64 },
    SetFrame { id: i64 },
    AddBreakpoint { filename: String, linenumber: u64 },
    ClearBreakpoint { filename: String, linenumber: u64 },
    EnableBreakpoint { filename: String, linenumber: u64 },
    DisableBreakpoint { filename: String, linenumber: u64 },
    ListBreakpoint,
    Next,
    Step,
    Continue,
    AddVariable { var: String },
    DelVariable { var: String },
    DebuggerState,
    VariableList,
    CurrentCode,
    FullBacktrace,
    InfoThreads,
    Print { var: String, value: Option<String> },
    InfoRegs,
    Quit,
    Kill,
    SetDisplayLinesBeforeCurrent { size: usize },
    SetDisplayLinesAfterCurrent { size: usize },
    SetDisplayBacktrace { size: usize },
    SetDisplayVariableList { size: usize },
    Custom { cmd: String },
}

enum WorkerMessage {
    Execute {
        operation: ToolOperation,
        response_tx: oneshot::Sender<Result<DebuggerResponse>>,
    },
}

#[derive(Clone)]
pub struct SessionWorkerHandle {
    request_tx: mpsc::Sender<WorkerMessage>,
}

impl SessionWorkerHandle {
    pub async fn execute(&self, operation: ToolOperation) -> Result<DebuggerResponse> {
        let (response_tx, response_rx) = oneshot::channel();
        self.request_tx
            .send(WorkerMessage::Execute {
                operation,
                response_tx,
            })
            .await
            .map_err(|_| OpenMcpGdbError::SessionClosed)?;
        response_rx
            .await
            .map_err(|_| OpenMcpGdbError::SessionClosed)?
    }
}

pub fn spawn_session_thread(
    config: ServerConfig,
    mut backend: Box<dyn GdbBackend>,
) -> SessionWorkerHandle {
    let (request_tx, mut request_rx) = mpsc::channel::<WorkerMessage>(64);

    // Every client gets a dedicated worker thread and runtime for isolation.
    thread::spawn(move || {
        let runtime_result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();

        let Ok(runtime) = runtime_result else {
            return;
        };

        runtime.block_on(async move {
            let mut core = SessionCore::new(config, &mut backend);
            while let Some(message) = request_rx.recv().await {
                match message {
                    WorkerMessage::Execute {
                        operation,
                        response_tx,
                    } => {
                        let result = core.execute(operation).await;
                        let _ = response_tx.send(result);
                    }
                }
            }
            let _ = backend.stop().await;
        });
    });

    SessionWorkerHandle { request_tx }
}

struct SessionCore<'a> {
    config: ServerConfig,
    backend: &'a mut Box<dyn GdbBackend>,
    debugger_state: DebuggerState,
    watched_variables: Vec<String>,
    executable_path: Option<PathBuf>,
    last_error: String,
}

impl<'a> SessionCore<'a> {
    fn new(config: ServerConfig, backend: &'a mut Box<dyn GdbBackend>) -> Self {
        Self {
            config,
            backend,
            debugger_state: DebuggerState::NotAttached,
            watched_variables: Vec::new(),
            executable_path: None,
            last_error: String::new(),
        }
    }

    async fn execute(&mut self, operation: ToolOperation) -> Result<DebuggerResponse> {
        match operation {
            ToolOperation::Execute { executable_path } => {
                self.execute_attach(executable_path).await
            }
            ToolOperation::Run => {
                self.execute_command("run", Some(DebuggerState::Running))
                    .await
            }
            ToolOperation::TargetRemote { ip, port } => {
                self.execute_command(&format!("target remote {ip}:{port}"), None)
                    .await
            }
            ToolOperation::SetThread { id } => {
                self.execute_command(&format!("thread {id}"), None).await
            }
            ToolOperation::SetFrame { id } => {
                self.execute_command(&format!("frame {id}"), None).await
            }
            ToolOperation::AddBreakpoint {
                filename,
                linenumber,
            } => {
                self.execute_command(
                    &format!("break {filename}:{linenumber}"),
                    Some(DebuggerState::StoppedAtBreakpoint),
                )
                .await
            }
            ToolOperation::ClearBreakpoint {
                filename,
                linenumber,
            } => {
                self.execute_command(&format!("clear {filename}:{linenumber}"), None)
                    .await
            }
            ToolOperation::EnableBreakpoint {
                filename,
                linenumber,
            } => {
                self.execute_command(&format!("enable location {filename}:{linenumber}"), None)
                    .await
            }
            ToolOperation::DisableBreakpoint {
                filename,
                linenumber,
            } => {
                self.execute_command(&format!("disable location {filename}:{linenumber}"), None)
                    .await
            }
            ToolOperation::ListBreakpoint => self.execute_command("info breakpoints", None).await,
            ToolOperation::Next => {
                self.execute_with_full_snapshot("next", DebuggerState::StoppedAtBreakpoint)
                    .await
            }
            ToolOperation::Step => {
                self.execute_with_full_snapshot("step", DebuggerState::StoppedAtBreakpoint)
                    .await
            }
            ToolOperation::Continue => {
                self.execute_with_full_snapshot("continue", DebuggerState::Running)
                    .await
            }
            ToolOperation::AddVariable { var } => {
                if !self
                    .watched_variables
                    .iter()
                    .any(|existing| existing == &var)
                {
                    self.watched_variables.push(var);
                }
                self.variable_list_response().await
            }
            ToolOperation::DelVariable { var } => {
                self.watched_variables.retain(|existing| existing != &var);
                self.variable_list_response().await
            }
            ToolOperation::DebuggerState => Ok(self.base_response()),
            ToolOperation::VariableList => self.variable_list_response().await,
            ToolOperation::CurrentCode => self.current_code_response().await,
            ToolOperation::FullBacktrace => self.full_backtrace_response().await,
            ToolOperation::InfoThreads => self.execute_command("info threads", None).await,
            ToolOperation::Print { var, value } => {
                let cmd = if let Some(value) = value {
                    format!("set variable {var} = {value}")
                } else {
                    format!("print {var}")
                };
                self.execute_command(&cmd, None).await
            }
            ToolOperation::InfoRegs => self.execute_command("info all-registers", None).await,
            ToolOperation::Quit => {
                let _ = self.backend.exec("quit").await;
                let _ = self.backend.stop().await;
                self.debugger_state = DebuggerState::NotAttached;
                self.executable_path = None;
                Ok(self.base_response())
            }
            ToolOperation::Kill => {
                self.execute_command("kill", Some(DebuggerState::SigKill))
                    .await
            }
            ToolOperation::SetDisplayLinesBeforeCurrent { size } => {
                self.config.display_lines_before_current = size;
                Ok(self.base_response())
            }
            ToolOperation::SetDisplayLinesAfterCurrent { size } => {
                self.config.display_lines_after_current = size;
                Ok(self.base_response())
            }
            ToolOperation::SetDisplayBacktrace { size } => {
                self.config.display_backtrace = size;
                Ok(self.base_response())
            }
            ToolOperation::SetDisplayVariableList { size } => {
                self.config.display_variable_list = size;
                Ok(self.base_response())
            }
            ToolOperation::Custom { cmd } => self.execute_command(&cmd, None).await,
        }
    }

    async fn execute_attach(&mut self, executable_path: String) -> Result<DebuggerResponse> {
        let executable = PathBuf::from(executable_path.clone());
        if !executable.is_absolute() {
            self.debugger_state = DebuggerState::FailedToAttach;
            self.last_error = "executable_path must be absolute".to_string();
            return Ok(self.base_response().with_error(self.last_error.clone()));
        }
        match self.backend.start(&executable).await {
            Ok(_) => {
                self.executable_path = Some(executable);
                self.debugger_state = DebuggerState::NotAttached;
                self.last_error.clear();
                Ok(self.base_response())
            }
            Err(err) => {
                self.debugger_state = DebuggerState::FailedToAttach;
                self.last_error = err.to_string();
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
        }
    }

    async fn execute_command(
        &mut self,
        command: &str,
        fallback_state: Option<DebuggerState>,
    ) -> Result<DebuggerResponse> {
        let result = self.backend.exec(command).await;
        match result {
            Ok(output) => {
                self.update_state_from_output(&output, fallback_state);
                self.last_error = String::new();
                Ok(self.base_response())
            }
            Err(err) => {
                self.last_error = err.to_string();
                self.debugger_state = DebuggerState::Error;
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
        }
    }

    async fn execute_with_full_snapshot(
        &mut self,
        command: &str,
        fallback_state: DebuggerState,
    ) -> Result<DebuggerResponse> {
        let _ = self.execute_command(command, Some(fallback_state)).await?;
        let mut response = self.base_response();
        response.variable_list = Some(self.collect_variable_list().await?);
        let (backtrace, current_func) = self.collect_backtrace(true).await?;
        response.backtrace = Some(backtrace);
        response.current_func = current_func;
        let code = self.collect_current_code().await?;
        response.current_code_path = code.0;
        response.current_code_line = code.1;
        response.current_code = code.2;
        Ok(response)
    }

    async fn variable_list_response(&mut self) -> Result<DebuggerResponse> {
        let mut response = self.base_response();
        response.variable_list = Some(self.collect_variable_list().await?);
        Ok(response)
    }

    async fn full_backtrace_response(&mut self) -> Result<DebuggerResponse> {
        let mut response = self.base_response();
        let (backtrace, current_func) = self.collect_backtrace(true).await?;
        response.backtrace = Some(backtrace);
        response.current_func = current_func;
        Ok(response)
    }

    async fn current_code_response(&mut self) -> Result<DebuggerResponse> {
        let mut response = self.base_response();
        let code = self.collect_current_code().await?;
        response.current_code_path = code.0;
        response.current_code_line = code.1;
        response.current_code = code.2;
        Ok(response)
    }

    async fn collect_variable_list(&mut self) -> Result<BTreeMap<String, String>> {
        let mut variables = BTreeMap::new();
        variables.insert(
            "list_size".to_string(),
            self.watched_variables.len().to_string(),
        );

        for variable in self
            .watched_variables
            .iter()
            .take(self.config.display_variable_list)
        {
            let output = self.backend.exec(&format!("print {variable}")).await;
            match output {
                Ok(output) => {
                    let value = normalize_gdb_value(&output);
                    variables.insert(variable.clone(), value);
                }
                Err(err) => {
                    variables.insert(variable.clone(), format!("<error: {err}>"));
                }
            }
        }

        Ok(variables)
    }

    async fn collect_backtrace(
        &mut self,
        full: bool,
    ) -> Result<(BTreeMap<String, String>, Option<String>)> {
        let command = if full { "backtrace full" } else { "backtrace" };
        let output = self.backend.exec(command).await.unwrap_or_default();
        let mut backtrace = BTreeMap::new();

        for line in output.lines().take(self.config.display_backtrace) {
            let trimmed = line.trim_start();
            if !trimmed.starts_with('#') {
                continue;
            }
            let mut pieces = trimmed.split_whitespace();
            let frame = pieces
                .next()
                .and_then(|frame| frame.strip_prefix('#'))
                .unwrap_or("?")
                .to_string();

            let function = if let Some(index) = trimmed.find(" in ") {
                trimmed[(index + 4)..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string()
            } else {
                pieces.next().unwrap_or("unknown").to_string()
            };
            backtrace.insert(frame, function);
        }

        let current_func = backtrace.get("0").cloned();
        Ok((backtrace, current_func))
    }

    async fn collect_current_code(
        &mut self,
    ) -> Result<(
        Option<String>,
        Option<u64>,
        Option<BTreeMap<String, String>>,
    )> {
        let frame = self.backend.exec("frame").await.unwrap_or_default();
        let (path, line) = parse_path_and_line(&frame);

        let mut code_lines = BTreeMap::new();
        if let Some(line) = line {
            let before = self.config.display_lines_before_current as u64;
            let after = self.config.display_lines_after_current as u64;
            let start = line.saturating_sub(before);
            let end = line + after;
            let list_output = self
                .backend
                .exec(&format!("list {start},{end}"))
                .await
                .unwrap_or_default();
            for raw_line in list_output.lines() {
                if let Some((number, source)) = raw_line.split_once('\t') {
                    if number.chars().all(|char| char.is_ascii_digit()) {
                        code_lines.insert(number.trim().to_string(), source.to_string());
                    }
                }
            }
        }

        let current_code = if code_lines.is_empty() {
            None
        } else {
            Some(code_lines)
        };

        Ok((path, line, current_code))
    }

    fn base_response(&self) -> DebuggerResponse {
        let mut response = DebuggerResponse::new(self.debugger_state);
        if !self.last_error.is_empty() {
            response.error = self.last_error.clone();
        }
        response
    }

    fn update_state_from_output(&mut self, output: &str, fallback_state: Option<DebuggerState>) {
        let lower = output.to_ascii_lowercase();
        if lower.contains("sigsegv") {
            self.debugger_state = DebuggerState::SigSegv;
            return;
        }
        if lower.contains("sigterm") {
            self.debugger_state = DebuggerState::SigTerm;
            return;
        }
        if lower.contains("sigkill") {
            self.debugger_state = DebuggerState::SigKill;
            return;
        }
        if lower.contains("breakpoint") {
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }
        if lower.contains("exited") {
            self.debugger_state = DebuggerState::Exited;
            return;
        }
        if let Some(state) = fallback_state {
            self.debugger_state = state;
        }
    }
}

fn normalize_gdb_value(output: &str) -> String {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((_, rhs)) = trimmed.split_once('=') {
            return rhs.trim().to_string();
        }
        return trimmed.to_string();
    }
    String::new()
}

fn parse_path_and_line(frame_output: &str) -> (Option<String>, Option<u64>) {
    for line in frame_output.lines() {
        if let Some(at_idx) = line.find(" at ") {
            let segment = line[(at_idx + 4)..].trim();
            if let Some((path, line_number)) = segment.rsplit_once(':') {
                if let Ok(number) = line_number.parse::<u64>() {
                    return (Some(path.to_string()), Some(number));
                }
            }
        }
    }
    (None, None)
}
