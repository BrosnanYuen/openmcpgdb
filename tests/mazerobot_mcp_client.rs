use anyhow::Result;
use openmcpgdb::{
    ServerConfig,
    gdb::RealGdbBackendFactory,
    server::OpenMcpGdbServerFactory,
};
use rmcp::{
    ClientHandler, ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
};
use std::{path::Path, sync::Arc};

const MAZE_CODEBASE_DIR: &str = "/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot";
const MAZE_BINARY_PATH: &str = "/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot";

#[derive(Debug, Clone, Default)]
struct MazeTestClient;

impl ClientHandler for MazeTestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

fn mazerobot_config() -> ServerConfig {
    ServerConfig {
        gdb_path: "/usr/bin/gdb".into(),
        gdb_options: String::new(),
        codebase_dir: MAZE_CODEBASE_DIR.into(),
        executable_path: MAZE_BINARY_PATH.into(),
        mcp_server_name: "MCP GDB Server".to_string(),
        mcp_server_url: "stdio://local".to_string(),
        display_lines_before_current: 7,
        display_lines_after_current: 8,
        display_backtrace: 6,
        display_variable_list: 9,
    }
}

fn has_required_paths() -> bool {
    Path::new("/usr/bin/gdb").exists()
        && Path::new(MAZE_CODEBASE_DIR).exists()
        && Path::new(MAZE_BINARY_PATH).exists()
}

#[tokio::test]
async fn test_mcp_server_with_mazerobot_binary() -> Result<()> {
    if !has_required_paths() {
        eprintln!("Skipping test_mcp_server_with_mazerobot_binary: required paths missing");
        return Ok(());
    }

    let config = mazerobot_config();
    config.validate()?;

    let factory = OpenMcpGdbServerFactory::new(config, Arc::new(RealGdbBackendFactory));
    let server = factory.build();

    let (server_transport, client_transport) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(async move {
        if let Ok(running) = server.serve(server_transport).await {
            let _ = running.waiting().await;
        }
    });

    let client = MazeTestClient.serve(client_transport).await?;

    let tools = client.list_all_tools().await?;
    assert!(
        tools.iter().any(|tool| tool.name == "openmcpgdb_execute"),
        "openmcpgdb_execute tool should be registered"
    );

    let execute_result = client
        .call_tool(
            CallToolRequestParams::new("openmcpgdb_execute").with_arguments(
                serde_json::json!({ "executable_path": MAZE_BINARY_PATH })
                    .as_object()
                    .expect("execute args should be object")
                    .clone(),
            ),
        )
        .await?;

    assert_eq!(execute_result.is_error, Some(false));
    assert!(
        !execute_result.content.is_empty(),
        "execute tool should return debugger response"
    );

    let state_result = client
        .call_tool(CallToolRequestParams::new("openmcpgdb_debugger_state"))
        .await?;

    assert_eq!(state_result.is_error, Some(false));
    assert!(
        !state_result.content.is_empty(),
        "debugger_state tool should return debugger response"
    );

    let _ = client
        .call_tool(CallToolRequestParams::new("openmcpgdb_quit"))
        .await?;

    client.cancel().await?;
    let _ = server_task.await;
    Ok(())
}
