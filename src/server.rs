use crate::{
    config::ServerConfig,
    gdb::GdbBackendFactory,
    protocol::{DebuggerResponse, DebuggerState},
    session::{SessionWorkerHandle, ToolOperation, spawn_session_thread},
};
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExecuteArgs {
    executable_path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TargetRemoteArgs {
    ip: String,
    port: u16,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GdbServerArgs {
    ip: String,
    port: u16,
    pid: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IdArgs {
    id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BreakpointArgs {
    filename: String,
    linenumber: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct VariableArgs {
    var: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PrintArgs {
    var: String,
    value: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetVarArgs {
    var: String,
    value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SizeArgs {
    size: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CustomArgs {
    cmd: String,
}

#[derive(Clone)]
pub struct OpenMcpGdbServerFactory {
    config: ServerConfig,
    backend_factory: Arc<dyn GdbBackendFactory>,
}

impl OpenMcpGdbServerFactory {
    pub fn new(config: ServerConfig, backend_factory: Arc<dyn GdbBackendFactory>) -> Self {
        Self {
            config,
            backend_factory,
        }
    }

    pub fn build(&self) -> OpenMcpGdbServer {
        OpenMcpGdbServer::new(self.config.clone(), Arc::clone(&self.backend_factory))
    }
}

#[derive(Clone)]
pub struct OpenMcpGdbServer {
    config: ServerConfig,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    worker: SessionWorkerHandle,
}

impl OpenMcpGdbServer {
    pub fn new(config: ServerConfig, backend_factory: Arc<dyn GdbBackendFactory>) -> Self {
        // Spawn one dedicated thread for this client session.
        let backend = backend_factory.create(&config);
        let worker = spawn_session_thread(config.clone(), backend);

        Self {
            config,
            tool_router: Self::tool_router(),
            worker,
        }
    }

    async fn call_operation(&self, operation: ToolOperation) -> Json<DebuggerResponse> {
        match self.worker.execute(operation).await {
            Ok(response) => Json(response),
            Err(err) => {
                Json(DebuggerResponse::new(DebuggerState::Error).with_error(err.to_string()))
            }
        }
    }
}

#[tool_router]
impl OpenMcpGdbServer {
    #[tool(name = "openmcpgdb_execute", description = "Start gdb on executable")]
    async fn openmcpgdb_execute(
        &self,
        Parameters(args): Parameters<ExecuteArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Execute {
            executable_path: args.executable_path,
        })
        .await
    }

    #[tool(name = "openmcpgdb_run", description = "Run executable in gdb")]
    async fn openmcpgdb_run(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Run).await
    }

    #[tool(
        name = "openmcpgdb_gdbserver",
        description = "Start gdbserver and attach to a pid"
    )]
    async fn openmcpgdb_gdbserver(
        &self,
        Parameters(args): Parameters<GdbServerArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::GdbServer {
            ip: args.ip,
            port: args.port,
            pid: args.pid,
        })
        .await
    }

    #[tool(
        name = "openmcpgdb_target_remote",
        description = "Connect to remote target"
    )]
    async fn openmcpgdb_target_remote(
        &self,
        Parameters(args): Parameters<TargetRemoteArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::TargetRemote {
            ip: args.ip,
            port: args.port,
        })
        .await
    }

    #[tool(name = "openmcpgdb_set_thread", description = "Set current thread")]
    async fn openmcpgdb_set_thread(
        &self,
        Parameters(args): Parameters<IdArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetThread { id: args.id })
            .await
    }

    #[tool(name = "openmcpgdb_set_frame", description = "Set current frame")]
    async fn openmcpgdb_set_frame(
        &self,
        Parameters(args): Parameters<IdArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetFrame { id: args.id })
            .await
    }

    #[tool(name = "openmcpgdb_add_breakpoint", description = "Add breakpoint")]
    async fn openmcpgdb_add_breakpoint(
        &self,
        Parameters(args): Parameters<BreakpointArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::AddBreakpoint {
            filename: args.filename,
            linenumber: args.linenumber,
        })
        .await
    }

    #[tool(name = "openmcpgdb_clear_breakpoint", description = "Clear breakpoint")]
    async fn openmcpgdb_clear_breakpoint(
        &self,
        Parameters(args): Parameters<BreakpointArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::ClearBreakpoint {
            filename: args.filename,
            linenumber: args.linenumber,
        })
        .await
    }

    #[tool(
        name = "openmcpgdb_enable_breakpoint",
        description = "Enable breakpoint"
    )]
    async fn openmcpgdb_enable_breakpoint(
        &self,
        Parameters(args): Parameters<BreakpointArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::EnableBreakpoint {
            filename: args.filename,
            linenumber: args.linenumber,
        })
        .await
    }

    #[tool(
        name = "openmcpgdb_disable_breakpoint",
        description = "Disable breakpoint"
    )]
    async fn openmcpgdb_disable_breakpoint(
        &self,
        Parameters(args): Parameters<BreakpointArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::DisableBreakpoint {
            filename: args.filename,
            linenumber: args.linenumber,
        })
        .await
    }

    #[tool(name = "openmcpgdb_list_breakpoint", description = "List breakpoints")]
    async fn openmcpgdb_list_breakpoint(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::ListBreakpoint).await
    }

    #[tool(name = "openmcpgdb_next", description = "Step over")]
    async fn openmcpgdb_next(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Next).await
    }

    #[tool(name = "openmcpgdb_step", description = "Step into")]
    async fn openmcpgdb_step(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Step).await
    }

    #[tool(name = "openmcpgdb_continue", description = "Continue execution")]
    async fn openmcpgdb_continue(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Continue).await
    }

    #[tool(
        name = "openmcpgdb_add_variable_list",
        description = "Add variable to watch list"
    )]
    async fn openmcpgdb_add_variable_list(
        &self,
        Parameters(args): Parameters<VariableArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::AddVariable { var: args.var })
            .await
    }

    #[tool(
        name = "openmcpgdb_del_variable_list",
        description = "Delete variable from watch list"
    )]
    async fn openmcpgdb_del_variable_list(
        &self,
        Parameters(args): Parameters<VariableArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::DelVariable { var: args.var })
            .await
    }

    #[tool(name = "openmcpgdb_debugger_state", description = "Get debugger state")]
    async fn openmcpgdb_debugger_state(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::DebuggerState).await
    }

    #[tool(
        name = "openmcpgdb_variable_list",
        description = "Get watched variable list"
    )]
    async fn openmcpgdb_variable_list(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::VariableList).await
    }

    #[tool(
        name = "openmcpgdb_current_code",
        description = "Get current source code context"
    )]
    async fn openmcpgdb_current_code(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::CurrentCode).await
    }

    #[tool(name = "openmcpgdb_full_backtrace", description = "Get full backtrace")]
    async fn openmcpgdb_full_backtrace(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::FullBacktrace).await
    }

    #[tool(
        name = "openmcpgdb_info_threads",
        description = "Get thread information"
    )]
    async fn openmcpgdb_info_threads(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::InfoThreads).await
    }

    #[tool(
        name = "openmcpgdb_print",
        description = "Print variable or set variable"
    )]
    async fn openmcpgdb_print(
        &self,
        Parameters(args): Parameters<PrintArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Print {
            var: args.var,
            value: args.value,
        })
        .await
    }

    #[tool(
        name = "openmcpgdb_set_var",
        description = "Set variable value (gdb set variable <var> = <value>)"
    )]
    async fn openmcpgdb_set_var(
        &self,
        Parameters(args): Parameters<SetVarArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Print {
            var: args.var,
            value: Some(args.value),
        })
        .await
    }

    #[tool(
        name = "openmcpgdb_info_regs",
        description = "Get register information"
    )]
    async fn openmcpgdb_info_regs(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::InfoRegs).await
    }

    #[tool(name = "openmcpgdb_quit", description = "Quit gdb")]
    async fn openmcpgdb_quit(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Quit).await
    }

    #[tool(name = "openmcpgdb_kill", description = "Kill debuggee")]
    async fn openmcpgdb_kill(&self) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Kill).await
    }

    #[tool(
        name = "openmcpgdb_display_lines_before_current",
        description = "Set lines before current"
    )]
    async fn openmcpgdb_display_lines_before_current(
        &self,
        Parameters(args): Parameters<SizeArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetDisplayLinesBeforeCurrent { size: args.size })
            .await
    }

    #[tool(
        name = "openmcpgdb_display_lines_after_current",
        description = "Set lines after current"
    )]
    async fn openmcpgdb_display_lines_after_current(
        &self,
        Parameters(args): Parameters<SizeArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetDisplayLinesAfterCurrent { size: args.size })
            .await
    }

    #[tool(
        name = "openmcpgdb_display_backtrace",
        description = "Set backtrace depth"
    )]
    async fn openmcpgdb_display_backtrace(
        &self,
        Parameters(args): Parameters<SizeArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetDisplayBacktrace { size: args.size })
            .await
    }

    #[tool(
        name = "openmcpgdb_display_variable_list",
        description = "Set variable list size"
    )]
    async fn openmcpgdb_display_variable_list(
        &self,
        Parameters(args): Parameters<SizeArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::SetDisplayVariableList { size: args.size })
            .await
    }

    #[tool(name = "openmcpgdb_custom", description = "Run custom gdb command")]
    async fn openmcpgdb_custom(
        &self,
        Parameters(args): Parameters<CustomArgs>,
    ) -> Json<DebuggerResponse> {
        self.call_operation(ToolOperation::Custom { cmd: args.cmd })
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for OpenMcpGdbServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            format!(
                "{} - MCP server for GDB debugging",
                self.config.mcp_server_name
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gdb::{MockBackendHandle, MockGdbBackendFactory},
        protocol::DebuggerState,
    };
    use rmcp::{
        ClientHandler,
        model::ClientInfo,
    };

    fn test_config() -> ServerConfig {
        ServerConfig {
            gdb_path: "/usr/bin/gdb".into(),
            gdb_options: String::new(),
            codebase_dir: "/tmp".into(),
            executable_path: "/tmp/exe".into(),
            mcp_server_name: "MCP GDB Server".to_string(),
            mcp_server_url: "https://localhost:9443".to_string(),
            display_lines_before_current: 7,
            display_lines_after_current: 8,
            display_backtrace: 6,
            display_variable_list: 9,
            display_join_current_code: false,
        }
    }

    fn test_server_with_mock() -> (OpenMcpGdbServer, MockBackendHandle) {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("backtrace full", "#0 compute_pi\n#1 run_math\n#2 main\n");
        handle.set_response("frame", "#0 compute_pi at /tmp/main.c:55\n");
        handle.set_response(
            "list 48,63",
            "48\tline48\n49\tline49\n50\tline50\n55\tline55\n",
        );
        handle.set_response("print a", "$1 = 10\n");
        handle.set_response("info threads", "  Id   Target Id         Frame\n* 1    Thread 0x1 main\n");
        handle.set_response(
            "info all-registers",
            "rax            0x1                 1\nrbx            0x2                 2\n",
        );
        let factory = Arc::new(MockGdbBackendFactory::new(handle.clone()));
        let server = OpenMcpGdbServer::new(test_config(), factory);
        (server, handle)
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    fn test_server_with_mock_joined_code() -> OpenMcpGdbServer {
        let mut config = test_config();
        config.display_join_current_code = true;

        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("backtrace full", "#0 compute_pi\n#1 run_math\n#2 main\n");
        handle.set_response("frame", "#0 compute_pi at /tmp/main.c:55\n");
        handle.set_response(
            "list 48,63",
            "48\tline48\n49\tline49\n50\tline50\n55\tline55\n",
        );
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        OpenMcpGdbServer::new(config, factory)
    }

    #[tokio::test]
    async fn test_all_tool_calls_return_state() {
        let (server, _) = test_server_with_mock();

        let result = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await
            .0;
        assert_eq!(result.debugger_state, DebuggerState::Attached);

        let calls = vec![
            server.openmcpgdb_run().await.0,
            server
                .openmcpgdb_gdbserver(Parameters(GdbServerArgs {
                    ip: "127.0.0.1".to_string(),
                    port: 1234,
                    pid: -1,
                }))
                .await
                .0,
            server
                .openmcpgdb_target_remote(Parameters(TargetRemoteArgs {
                    ip: "127.0.0.1".to_string(),
                    port: 1234,
                }))
                .await
                .0,
            server
                .openmcpgdb_set_thread(Parameters(IdArgs { id: 1 }))
                .await
                .0,
            server
                .openmcpgdb_set_frame(Parameters(IdArgs { id: 0 }))
                .await
                .0,
            server
                .openmcpgdb_add_breakpoint(Parameters(BreakpointArgs {
                    filename: "/tmp/main.c".to_string(),
                    linenumber: 10,
                }))
                .await
                .0,
            server
                .openmcpgdb_clear_breakpoint(Parameters(BreakpointArgs {
                    filename: "/tmp/main.c".to_string(),
                    linenumber: 10,
                }))
                .await
                .0,
            server
                .openmcpgdb_enable_breakpoint(Parameters(BreakpointArgs {
                    filename: "/tmp/main.c".to_string(),
                    linenumber: 10,
                }))
                .await
                .0,
            server
                .openmcpgdb_disable_breakpoint(Parameters(BreakpointArgs {
                    filename: "/tmp/main.c".to_string(),
                    linenumber: 10,
                }))
                .await
                .0,
            server.openmcpgdb_list_breakpoint().await.0,
            server.openmcpgdb_next().await.0,
            server.openmcpgdb_step().await.0,
            server.openmcpgdb_continue().await.0,
            server
                .openmcpgdb_add_variable_list(Parameters(VariableArgs {
                    var: "a".to_string(),
                }))
                .await
                .0,
            server.openmcpgdb_variable_list().await.0,
            server
                .openmcpgdb_del_variable_list(Parameters(VariableArgs {
                    var: "a".to_string(),
                }))
                .await
                .0,
            server.openmcpgdb_debugger_state().await.0,
            server.openmcpgdb_current_code().await.0,
            server.openmcpgdb_full_backtrace().await.0,
            server.openmcpgdb_info_threads().await.0,
            server
                .openmcpgdb_print(Parameters(PrintArgs {
                    var: "a".to_string(),
                    value: None,
                }))
                .await
                .0,
            server
                .openmcpgdb_print(Parameters(PrintArgs {
                    var: "a".to_string(),
                    value: Some("12".to_string()),
                }))
                .await
                .0,
            server
                .openmcpgdb_set_var(Parameters(SetVarArgs {
                    var: "a".to_string(),
                    value: "13".to_string(),
                }))
                .await
                .0,
            server.openmcpgdb_info_regs().await.0,
            server.openmcpgdb_kill().await.0,
            server
                .openmcpgdb_display_lines_before_current(Parameters(SizeArgs { size: 4 }))
                .await
                .0,
            server
                .openmcpgdb_display_lines_after_current(Parameters(SizeArgs { size: 5 }))
                .await
                .0,
            server
                .openmcpgdb_display_backtrace(Parameters(SizeArgs { size: 3 }))
                .await
                .0,
            server
                .openmcpgdb_display_variable_list(Parameters(SizeArgs { size: 2 }))
                .await
                .0,
            server
                .openmcpgdb_custom(Parameters(CustomArgs {
                    cmd: "info files".to_string(),
                }))
                .await
                .0,
            server.openmcpgdb_quit().await.0,
        ];

        for response in calls {
            assert!(
                matches!(
                    response.debugger_state,
                    DebuggerState::NotAttached
                        | DebuggerState::FailedToAttach
                        | DebuggerState::GdbServerAttached
                        | DebuggerState::Attached
                        | DebuggerState::Running
                        | DebuggerState::StoppedAtBreakpoint
                        | DebuggerState::SigAbrt
                        | DebuggerState::SigBus
                        | DebuggerState::SigFpe
                        | DebuggerState::SigIll
                        | DebuggerState::SigTrap
                        | DebuggerState::SigKill
                        | DebuggerState::Exited
                        | DebuggerState::Error
                ),
                "unexpected debugger state"
            );
        }
    }

    #[derive(Debug, Clone, Default)]
    #[allow(dead_code)]
    struct DummyClient;

    impl ClientHandler for DummyClient {
        fn get_info(&self) -> ClientInfo {
            ClientInfo::default()
        }
    }

    #[tokio::test]
    async fn test_add_breakpoint_does_not_set_stopped_state() {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server
            .openmcpgdb_add_breakpoint(Parameters(BreakpointArgs {
                filename: "/tmp/main.c".to_string(),
                linenumber: 10,
            }))
            .await
            .0;

        assert_ne!(
            response.debugger_state,
            DebuggerState::StoppedAtBreakpoint,
            "add_breakpoint should not set state to StoppedAtBreakpoint"
        );
    }

    #[tokio::test]
    async fn test_command_output_strips_gdb_prompt() {
        let handle = MockBackendHandle::with_default_response("ok");
        handle.set_response(
            "info threads",
            "(gdb)   Id   Target Id         Frame\n* 1    Thread 0x1 main\n",
        );
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server.openmcpgdb_info_threads().await.0;
        let output = response.command_output.unwrap_or_default();
        assert!(
            !output.starts_with("(gdb)"),
            "command_output should not start with (gdb) prompt, got: {output}"
        );
    }

    #[tokio::test]
    async fn test_variable_list_handles_out_of_scope() {
        let handle = MockBackendHandle::with_default_response("ok");
        handle.set_response("print step", "$1 = {<text variable, no debug info>} 0x7ffff7f03920 <step>\n");
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server
            .openmcpgdb_add_variable_list(Parameters(VariableArgs {
                var: "step".to_string(),
            }))
            .await
            .0;

        let var_list = response.variable_list.expect("variable_list should be present");
        let step_value = var_list.get("step").expect("step should be in variable_list");
        assert!(
            step_value.contains("<text variable") || step_value.contains("<error:"),
            "out-of-scope variable should be indicated, got: {step_value}"
        );
    }

    #[tokio::test]
    async fn test_collect_current_code_propagates_frame_error() {
        let handle = MockBackendHandle::with_default_response("ok");
        handle.set_response("frame", "No stack.\n");
        handle.set_response("backtrace full", "#0 main\n");
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server.openmcpgdb_current_code().await.0;
        assert!(
            response.current_code_path.is_none(),
            "current_code_path should be None when frame has no stack"
        );
        assert!(
            response.current_code_line.is_none(),
            "current_code_line should be None when frame has no stack"
        );
        assert!(
            response.current_code.is_none(),
            "current_code should be None when frame has no stack"
        );
    }

    #[tokio::test]
    async fn test_custom_returns_command_output() {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("info locals", "x = 10\ny = 20\n");
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server
            .openmcpgdb_custom(Parameters(CustomArgs {
                cmd: "info locals".to_string(),
            }))
            .await
            .0;

        let output = response.command_output.expect("custom should return command_output");
        assert!(
            output.contains("x = 10"),
            "command_output should contain gdb result, got: {output}"
        );
    }

    #[tokio::test]
    async fn test_execute_with_full_snapshot_propagates_errors() {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("run", "\nBreakpoint 1, main () at src/main.c:10\n");
        handle.set_response("next", "Program received signal SIGSEGV, Segmentation fault.\n");
        handle.set_response("backtrace full", "#0 main\n");
        handle.set_response("frame", "#0 main at /tmp/main.c:10\n");
        handle.set_response("list 3,18", "10\tline10\n");
        handle.set_response("print step", "$1 = 0\n");

        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server.openmcpgdb_next().await.0;

        assert_eq!(
            response.debugger_state,
            DebuggerState::SigSegv,
            "next should report SigSegv state when gdb detects segfault, got: {:?}",
            response.debugger_state
        );
        assert!(
            response.variable_list.is_none(),
            "full snapshot should not be taken on error state"
        );
    }

    #[tokio::test]
    async fn test_add_breakpoint_preserves_current_state() {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("break /tmp/main.c:10", "Breakpoint 1 at 0x1234\n");
        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        let response = server
            .openmcpgdb_add_breakpoint(Parameters(BreakpointArgs {
                filename: "/tmp/main.c".to_string(),
                linenumber: 10,
            }))
            .await
            .0;

        assert_eq!(
            response.debugger_state,
            DebuggerState::Attached,
            "add_breakpoint should preserve Attached state, got: {:?}",
            response.debugger_state
        );
    }

    #[tokio::test]
    async fn test_run_returns_fresh_snapshot_not_stale() {
        let handle = MockBackendHandle::with_default_response("Breakpoint 1 at 0x0");
        handle.set_response("run", "\nBreakpoint 1, app_run () at src/main.c:30\n");
        handle.set_response("backtrace full", "#0 app_run\n#1 main\n");
        handle.set_response("frame", "#0 app_run at /tmp/main.c:30\n");
        handle.set_response(
            "list 23,45",
            "23\tline23\n24\tline24\n30\tline30\n31\tline31\n",
        );
        handle.set_response("print step", "$1 = 0\n");

        let factory = Arc::new(MockGdbBackendFactory::new(handle));
        let server = OpenMcpGdbServer::new(test_config(), factory);

        let _ = server
            .openmcpgdb_execute(Parameters(ExecuteArgs {
                executable_path: "/tmp/exe".to_string(),
            }))
            .await;

        server
            .openmcpgdb_add_variable_list(Parameters(VariableArgs {
                var: "step".to_string(),
            }))
            .await;

        let run_response = server.openmcpgdb_run().await.0;

        assert_eq!(
            run_response.debugger_state,
            DebuggerState::StoppedAtBreakpoint
        );

        let var_list = run_response.variable_list.expect("variable_list should be present");
        let step_value = var_list.get("step").expect("step should be in variable_list");
        assert_eq!(
            step_value, "0",
            "step should be 0 at breakpoint on first run, got: {step_value}"
        );
    }
}
