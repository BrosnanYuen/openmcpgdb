use crate::{
    config::ServerConfig,
    error::{OpenMcpGdbError, Result},
    gdb::GdbBackend,
    protocol::{CurrentCodePayload, DebuggerResponse, DebuggerState},
};
use std::{collections::BTreeMap, path::PathBuf, process::Stdio, thread};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, sleep};

#[derive(Debug)]
pub enum ToolOperation {
    Execute { executable_path: String },
    Run,
    GdbServer { ip: String, port: u16, pid: i64 },
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
            let _ = core.shutdown().await;
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
    gdbserver_child: Option<tokio::process::Child>,
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
            gdbserver_child: None,
            last_error: String::new(),
        }
    }

    async fn execute(&mut self, operation: ToolOperation) -> Result<DebuggerResponse> {
        match operation {
            ToolOperation::Execute { executable_path } => {
                println!(
                    "[openmcpgdb_execute] requested executable_path={}",
                    executable_path
                );
                self.execute_attach(executable_path).await
            }
            ToolOperation::Run => {
                println!("[openmcpgdb_run] requested");
                self.execute_run().await
            }
            ToolOperation::GdbServer { ip, port, pid } => self.execute_gdbserver(ip, port, pid).await,
            ToolOperation::TargetRemote { ip, port } => {
                self.execute_command(
                    &format!("target remote {ip}:{port}"),
                    Some(DebuggerState::Attached),
                )
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
                    None,
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
            ToolOperation::ListBreakpoint => self.list_breakpoint_response().await,
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
            ToolOperation::InfoThreads => self.execute_command_with_output("info threads", None).await,
            ToolOperation::Print { var, value } => {
                if let Some(value) = value {
                    self.execute_command(&format!("set variable {var} = {value}"), None)
                        .await
                } else {
                    self.execute_command_with_output(&format!("print {var}"), None)
                        .await
                }
            }
            ToolOperation::InfoRegs => self
                .execute_command_with_output("info all-registers", None)
                .await,
            ToolOperation::Quit => {
                let _ = self.stop_gdbserver_process().await;
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
            ToolOperation::Custom { cmd } => self.execute_command_with_output(&cmd, None).await,
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.stop_gdbserver_process().await
    }

    async fn execute_attach(&mut self, executable_path: String) -> Result<DebuggerResponse> {
        let executable = PathBuf::from(executable_path.clone());
        if !executable.is_absolute() {
            self.debugger_state = DebuggerState::FailedToAttach;
            self.last_error = "executable_path must be absolute".to_string();
            eprintln!(
                "[openmcpgdb_execute] failed: executable path is not absolute: {}",
                executable_path
            );
            return Ok(self.base_response().with_error(self.last_error.clone()));
        }
        match self.backend.start(&executable).await {
            Ok(_) => {
                self.executable_path = Some(executable);
                self.debugger_state = DebuggerState::Attached;
                self.last_error.clear();
                println!(
                    "[openmcpgdb_execute] success: gdb started for {}",
                    executable_path
                );
                Ok(self.base_response())
            }
            Err(err) => {
                self.debugger_state = DebuggerState::FailedToAttach;
                self.last_error = err.to_string();
                eprintln!(
                    "[openmcpgdb_execute] failed to start gdb for {}: {}",
                    executable_path, self.last_error
                );
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
        }
    }

    async fn execute_command(
        &mut self,
        command: &str,
        fallback_state: Option<DebuggerState>,
    ) -> Result<DebuggerResponse> {
        self.execute_command_internal(command, fallback_state, false)
            .await
    }

    async fn execute_command_with_output(
        &mut self,
        command: &str,
        fallback_state: Option<DebuggerState>,
    ) -> Result<DebuggerResponse> {
        self.execute_command_internal(command, fallback_state, true)
            .await
    }

    async fn execute_command_internal(
        &mut self,
        command: &str,
        fallback_state: Option<DebuggerState>,
        include_output: bool,
    ) -> Result<DebuggerResponse> {
        if let Err(err) = self.ensure_backend_started().await {
            self.last_error = err.to_string();
            self.debugger_state = DebuggerState::FailedToAttach;
            if command == "run" {
                eprintln!("[openmcpgdb_run] failed to prepare backend: {}", self.last_error);
            }
            return Ok(self.base_response().with_error(self.last_error.clone()));
        }

        let result = self.backend.exec(command).await;
        match result {
            Ok(output) => {
                self.update_state_from_output(&output, fallback_state);
                self.last_error = String::new();
                if command == "run" {
                    println!(
                        "[openmcpgdb_run] success: debugger_state={:?}, gdb_output={}",
                        self.debugger_state,
                        output.trim()
                    );
                }
                let mut response = self.base_response();
                if include_output {
                    response.command_output = normalized_command_output(&output);
                }
                Ok(response)
            }
            Err(err) => {
                self.last_error = err.to_string();
                self.debugger_state = DebuggerState::Error;
                if command == "run" {
                    eprintln!("[openmcpgdb_run] failed: {}", self.last_error);
                }
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
        }
    }

    async fn execute_run(&mut self) -> Result<DebuggerResponse> {
        let response = self
            .execute_command("run", Some(DebuggerState::Running))
            .await?;

        // Per spec, when run stops at a breakpoint, return the full normal response.
        if response.debugger_state == DebuggerState::StoppedAtBreakpoint {
            return self.full_snapshot_response().await;
        }

        Ok(response)
    }

    async fn execute_gdbserver(
        &mut self,
        ip: String,
        port: u16,
        pid: i64,
    ) -> Result<DebuggerResponse> {
        if pid <= 0 {
            self.debugger_state = DebuggerState::FailedToAttach;
            self.last_error = "pid must be > 0".to_string();
            return Ok(self.base_response().with_error(self.last_error.clone()));
        }

        let _ = self.stop_gdbserver_process().await;

        let endpoint = format!("{ip}:{port}");
        let mut command = tokio::process::Command::new("gdbserver");
        command
            .arg("--attach")
            .arg(&endpoint)
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child_result = command.spawn();
        let mut child = match child_result {
            Ok(child) => child,
            Err(err) => {
                self.debugger_state = DebuggerState::FailedToAttach;
                self.last_error = format!("failed to start gdbserver: {err}");
                return Ok(self.base_response().with_error(self.last_error.clone()));
            }
        };

        sleep(Duration::from_millis(100)).await;
        match child.try_wait().map_err(OpenMcpGdbError::Io)? {
            Some(status) => {
                self.debugger_state = DebuggerState::FailedToAttach;
                self.last_error = format!("gdbserver exited early with status: {status}");
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
            None => {
                self.gdbserver_child = Some(child);
                self.debugger_state = DebuggerState::GdbServerAttached;
                self.last_error.clear();
                Ok(self.base_response())
            }
        }
    }

    async fn execute_with_full_snapshot(
        &mut self,
        command: &str,
        fallback_state: DebuggerState,
    ) -> Result<DebuggerResponse> {
        let response = self.execute_command(command, Some(fallback_state)).await?;
        match response.debugger_state {
            DebuggerState::Error
            | DebuggerState::SigSegv
            | DebuggerState::SigAbrt
            | DebuggerState::SigBus
            | DebuggerState::SigFpe
            | DebuggerState::SigIll
            | DebuggerState::SigTrap
            | DebuggerState::SigTerm
            | DebuggerState::SigKill
            | DebuggerState::Exited => return Ok(response),
            _ => {}
        }
        self.full_snapshot_response().await
    }

    async fn full_snapshot_response(&mut self) -> Result<DebuggerResponse> {
        let mut response = self.base_response();
        response.variable_list = Some(self.collect_variable_list().await?);
        let (backtrace, current_func) = self.collect_backtrace(true).await?;
        response.backtrace = Some(backtrace);
        response.current_func = current_func;
        let code = self.collect_current_code().await?;
        response.current_code_path = code.0;
        response.current_code_line = code.1.and_then(|line| i64::try_from(line).ok());
        response.current_code = code.2.map(|lines| self.transform_current_code(lines));
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

    async fn list_breakpoint_response(&mut self) -> Result<DebuggerResponse> {
        if let Err(err) = self.ensure_backend_started().await {
            self.last_error = err.to_string();
            self.debugger_state = DebuggerState::FailedToAttach;
            return Ok(self.base_response().with_error(self.last_error.clone()));
        }

        let output = self.backend.exec("info breakpoints").await;
        match output {
            Ok(output) => {
                let lines: Vec<String> = output
                    .lines()
                    .map(str::trim_end)
                    .filter(|line| !line.is_empty())
                    .map(|line| line.strip_prefix("(gdb) ").unwrap_or(line).to_string())
                    .collect();

                let mut response = self.base_response();
                response.breakpoints = Some(lines);
                Ok(response)
            }
            Err(err) => {
                self.last_error = err.to_string();
                self.debugger_state = DebuggerState::Error;
                Ok(self.base_response().with_error(self.last_error.clone()))
            }
        }
    }

    async fn current_code_response(&mut self) -> Result<DebuggerResponse> {
        let mut response = self.base_response();
        let code = self.collect_current_code().await?;
        response.current_code_path = code.0;
        response.current_code_line = code.1.and_then(|line| i64::try_from(line).ok());
        response.current_code = code.2.map(|lines| self.transform_current_code(lines));
        Ok(response)
    }

    async fn collect_variable_list(&mut self) -> Result<BTreeMap<String, String>> {
        let mut variables = BTreeMap::new();

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
        let mut output = self.backend.exec(command).await?;
        let mut backtrace = BTreeMap::new();
        parse_backtrace_lines(&output, self.config.display_backtrace, &mut backtrace);

        // Some targets or GDB settings can yield sparse/empty "backtrace full" output.
        // Fall back to plain backtrace to keep frame info available to MCP clients.
        if full && backtrace.is_empty() {
            output = self.backend.exec("backtrace").await?;
            parse_backtrace_lines(&output, self.config.display_backtrace, &mut backtrace);
        }

        let current_func = if let Some(func) = backtrace.get("0") {
            Some(func.clone())
        } else {
            let mut best: Option<(u64, String)> = None;
            for (frame_key, func) in &backtrace {
                if let Ok(frame_num) = frame_key.parse::<u64>() {
                    match &best {
                        Some((best_num, _)) if frame_num >= *best_num => {}
                        _ => {
                            best = Some((frame_num, func.clone()));
                        }
                    }
                }
            }
            best.map(|(_, func)| func)
                .or_else(|| backtrace.values().next().cloned())
        };
        Ok((backtrace, current_func))
    }

    async fn collect_current_code(
        &mut self,
    ) -> Result<(
        Option<String>,
        Option<u64>,
        Option<BTreeMap<u64, String>>,
    )> {
        let frame = self.backend.exec("frame").await?;
        let (path, line) = parse_path_and_line(&frame);

        let mut code_lines = BTreeMap::new();
        if let Some(line) = line {
            let before = self.config.display_lines_before_current as u64;
            let after = self.config.display_lines_after_current as u64;
            let start = std::cmp::max(1, line.saturating_sub(before));
            let end = line + after;
            let list_output = self
                .backend
                .exec(&format!("list {start},{end}"))
                .await?;
            for raw_line in list_output.lines() {
                if let Some((number, source)) = parse_gdb_list_line(raw_line) {
                    code_lines.insert(number, source.to_string());
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

        // Signal detection: check both explicit signal names and descriptive messages.
        if lower.contains("sigsegv") || lower.contains("segmentation fault") {
            self.debugger_state = DebuggerState::SigSegv;
            return;
        }
        if lower.contains("sigabrt")
            || (lower.contains("signal received") && lower.contains("sigabrt"))
            || lower.contains("program received signal sigabrt")
        {
            self.debugger_state = DebuggerState::SigAbrt;
            return;
        }
        if lower.contains("sigbus") || lower.contains("bus error") {
            self.debugger_state = DebuggerState::SigBus;
            return;
        }
        if lower.contains("sigfpe") || lower.contains("floating point exception") {
            self.debugger_state = DebuggerState::SigFpe;
            return;
        }
        if lower.contains("sigill") || lower.contains("illegal instruction") {
            self.debugger_state = DebuggerState::SigIll;
            return;
        }
        if lower.contains("sigtrap") {
            self.debugger_state = DebuggerState::SigTrap;
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
        if lower.contains("sighup") {
            self.debugger_state = DebuggerState::SigTerm;
            return;
        }
        if lower.contains("sigpipe") {
            self.debugger_state = DebuggerState::SigTerm;
            return;
        }
        if lower.contains("sigxcpu") || lower.contains("sigxfsz") {
            self.debugger_state = DebuggerState::SigTerm;
            return;
        }
        if lower.contains("sigint") && !lower.contains("breakpoint") {
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }

        // Generic "program received signal" pattern (catches any unhandled signal).
        if lower.contains("program received signal") {
            self.debugger_state = DebuggerState::Error;
            return;
        }
        if lower.contains("terminated with signal") {
            self.debugger_state = DebuggerState::Error;
            return;
        }

        // Breakpoint, watchpoint, and catchpoint stop detection.
        if lower.contains("breakpoint") {
            let is_creation = lower.contains("breakpoint ")
                && (lower.contains(" at 0x") || lower.contains(" num "));
            if is_creation {
                return;
            }
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }
        if lower.contains("watchpoint") || lower.contains("hardware watchpoint") {
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }
        if lower.contains("catchpoint") {
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }

        // Program termination and exit detection.
        if lower.contains("exited normally")
            || lower.contains("exited with code")
            || lower.contains("exited abnormally")
            || (lower.contains("inferior") && lower.contains("exited"))
        {
            self.debugger_state = DebuggerState::Exited;
            return;
        }

        // Running state detection from GDB output.
        if lower.contains("continuing") || lower.contains("starting program") {
            self.debugger_state = DebuggerState::Running;
            return;
        }

        // Program not running / no context detection.
        if lower.contains("no stack") || lower.contains("no registers") {
            self.debugger_state = DebuggerState::Exited;
            return;
        }
        if lower.contains("the program is not being run")
            || lower.contains("no inferior")
            || lower.contains("the program has no registers now")
        {
            self.debugger_state = DebuggerState::NotAttached;
            return;
        }

        // User interrupt detection.
        if lower.contains("interrupted") {
            self.debugger_state = DebuggerState::StoppedAtBreakpoint;
            return;
        }

        // Memory access error detection.
        if lower.contains("cannot access memory") {
            self.debugger_state = DebuggerState::Error;
            return;
        }

        // Detaching / process finished detection.
        if lower.contains("detaching") || lower.contains("process finished") {
            self.debugger_state = DebuggerState::NotAttached;
            return;
        }

        if let Some(state) = fallback_state {
            self.debugger_state = state;
        }
    }

    fn transform_current_code(&self, lines: BTreeMap<u64, String>) -> CurrentCodePayload {
        if self.config.display_join_current_code {
            let mut joined = String::new();
            let mut first = true;
            for (line_no, source) in lines {
                if !first {
                    joined.push('\n');
                }
                first = false;
                joined.push_str(&format!("{line_no} | {source}"));
            }
            CurrentCodePayload::Joined(joined)
        } else {
            CurrentCodePayload::Lines(lines)
        }
    }

    async fn ensure_backend_started(&mut self) -> Result<()> {
        if self.executable_path.is_some() {
            return Ok(());
        }

        let configured_path = self.config.executable_path.clone();
        if !configured_path.is_absolute() {
            return Err(OpenMcpGdbError::InvalidConfig(
                "configured executable_path must be absolute".to_string(),
            ));
        }

        self.backend.start(&configured_path).await?;
        self.executable_path = Some(configured_path);
        self.debugger_state = DebuggerState::Attached;
        Ok(())
    }

    async fn stop_gdbserver_process(&mut self) -> Result<()> {
        if let Some(mut child) = self.gdbserver_child.take() {
            let _ = child.kill().await;
        }
        Ok(())
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

fn normalized_command_output(output: &str) -> Option<String> {
    let trimmed = output.trim();
    let stripped = trimmed.strip_prefix("(gdb) ").unwrap_or(trimmed);
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
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

fn parse_gdb_list_line(line: &str) -> Option<(u64, &str)> {
    let trimmed = line.trim_start();
    let digits_len = trimmed
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return None;
    }

    let (digits, rest) = trimmed.split_at(digits_len);
    let number = digits.parse::<u64>().ok()?;
    // Preserve original source indentation: strip only the first field separator.
    let source = if let Some(stripped) = rest.strip_prefix('\t') {
        stripped
    } else if let Some(stripped) = rest.strip_prefix(' ') {
        stripped
    } else {
        rest
    };
    Some((number, source))
}

fn parse_backtrace_lines(output: &str, limit: usize, backtrace: &mut BTreeMap<String, String>) {
    for line in output.lines() {
        if backtrace.len() >= limit {
            break;
        }
        if let Some((frame_number, function)) = parse_backtrace_frame_line(line) {
            backtrace.insert(frame_number.to_string(), function);
        }
    }
}

fn parse_backtrace_frame_line(line: &str) -> Option<(u64, String)> {
    let hash_idx = line.find('#')?;
    let frame_section = &line[(hash_idx + 1)..];
    let digits_len = frame_section
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return None;
    }

    let frame_number = frame_section[..digits_len].parse::<u64>().ok()?;
    let mut rest = frame_section[digits_len..].trim_start();

    if let Some(index) = rest.find(" in ") {
        rest = &rest[(index + 4)..];
    } else if let Some(stripped) = rest.strip_prefix("in ") {
        rest = stripped;
    }

    let mut function = rest.split_whitespace().next().unwrap_or("unknown");
    if function == "in" {
        function = rest
            .split_whitespace()
            .nth(1)
            .unwrap_or("unknown");
    }
    if let Some((name, _)) = function.split_once('(') {
        function = name;
    }

    Some((frame_number, function.to_string()))
}

#[cfg(test)]
mod parse_tests {
    use super::{parse_backtrace_frame_line, parse_gdb_list_line};

    #[test]
    fn test_parse_gdb_list_line_preserves_space_indentation() {
        let parsed = parse_gdb_list_line("23\t    simulator_init(&g_sim, rows, cols, seed);");
        assert!(parsed.is_some(), "line should parse");
        let (line_no, source) = parsed.expect("parsed line");
        assert_eq!(line_no, 23);
        assert_eq!(source, "    simulator_init(&g_sim, rows, cols, seed);");
    }

    #[test]
    fn test_parse_gdb_list_line_preserves_tab_indentation() {
        let parsed = parse_gdb_list_line("24\t\trobot_init(&g_robot, &g_sim);");
        assert!(parsed.is_some(), "line should parse");
        let (line_no, source) = parsed.expect("parsed line");
        assert_eq!(line_no, 24);
        assert_eq!(source, "\trobot_init(&g_robot, &g_sim);");
    }

    #[test]
    fn test_parse_backtrace_frame_line_with_address_and_in() {
        let parsed = parse_backtrace_frame_line(
            "#0  0x00005555555551cb in compute_pi (value=3) at /tmp/main.c:55",
        );
        assert!(parsed.is_some(), "frame should parse");
        let (frame_no, function) = parsed.expect("parsed frame");
        assert_eq!(frame_no, 0);
        assert_eq!(function, "compute_pi");
    }

    #[test]
    fn test_list_breakpoint_strips_gdb_prompt() {
        let input = "(gdb) Num     Type           Disp Enb Address            What\n1       breakpoint     keep y   0x00005555555551cb in main at src/main.c:10\n";
        let lines: Vec<String> = input
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
            .map(|line| line.strip_prefix("(gdb) ").unwrap_or(line).to_string())
            .collect();

        assert_eq!(lines.len(), 2);
        assert!(!lines[0].starts_with("(gdb)"));
        assert!(!lines[1].starts_with("(gdb)"));
        assert_eq!(
            lines[0],
            "Num     Type           Disp Enb Address            What"
        );
    }
}
