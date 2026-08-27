use openmcpgdb::{
    ServerConfig,
    error::OpenMcpGdbError,
    runtime::{run_from_config, run_from_config_file},
};
use std::path::PathBuf;

fn print_usage() {
    println!(
        "openmcpgdb {} - Interactive MCP server to control gdb

Usage:
  openmcpgdb [CONFIG_PATH | --help | --version]

Arguments:
  CONFIG_PATH    Path to a JSON config file. Every field is optional; a
                 minimal config is just {{}}. With no argument, built-in
                 defaults apply (stdio transport, gdb resolved from PATH);
                 a config file is never read implicitly.

Defaults (all overridable via config):
  gdb_path                 \"gdb\" (resolved via PATH)
  mcp_server_url           \"stdio://\" (MCP over stdin/stdout; use
                           http://host:port for streamable HTTP)

Example minimal config.json:
  {{
    \"gdb_path\": \"/usr/bin/gdb\",
    \"codebase_dir\": \"/path/to/project/src\"
  }}",
        env!("CARGO_PKG_VERSION"),
    );
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), OpenMcpGdbError> {
    // Print errors with their user-facing Display text (including guidance)
    // instead of the Debug fallback used by the Result return path.
    let exit = real_main().await;
    if let Err(err) = &exit {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
    exit
}

async fn real_main() -> Result<(), OpenMcpGdbError> {
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            print_usage();
            Ok(())
        }
        Some("-V" | "--version") => {
            println!("openmcpgdb {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(path) => {
            // An explicitly provided config must exist; explain what went wrong.
            let config_path = PathBuf::from(path);
            if !config_path.exists() {
                return Err(OpenMcpGdbError::ConfigNotFound { path: config_path });
            }
            run_from_config_file(&config_path).await
        }
        // No argument: built-in defaults. A config file is only ever read
        // when explicitly passed, so startup never depends on the working
        // directory (deterministic for MCP client registration).
        None => run_from_config(ServerConfig::default()).await,
    }
}
