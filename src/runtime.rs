use crate::{
    config::ServerConfig,
    error::{OpenMcpGdbError, Result},
    gdb::RealGdbBackendFactory,
    server::{OpenMcpGdbServer, OpenMcpGdbServerFactory},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::{ServiceExt, transport};
use std::{path::Path, sync::Arc};
use tokio_util::sync::CancellationToken;
use url::Url;

pub async fn run_from_config_file(config_path: &Path) -> Result<()> {
    let config = ServerConfig::from_file(config_path)?;
    run_from_config(config).await
}

pub async fn run_from_config(config: ServerConfig) -> Result<()> {
    config.validate()?;

    let backend_factory = Arc::new(RealGdbBackendFactory);
    let server_factory = OpenMcpGdbServerFactory::new(config.clone(), backend_factory);

    if config.mcp_server_url.starts_with("stdio://") {
        return run_stdio_server(server_factory.build()).await;
    }

    let url = Url::parse(&config.mcp_server_url)
        .map_err(|err| OpenMcpGdbError::InvalidUrl(err.to_string()))?;
    match url.scheme() {
        "http" | "https" => run_http_server(url, server_factory).await,
        _ => Err(OpenMcpGdbError::InvalidUrl(
            "mcp_server_url must use stdio://, http://, or https://".to_string(),
        )),
    }
}

pub async fn run_stdio_server(server: OpenMcpGdbServer) -> Result<()> {
    // Stdio transport is the standard mode for Codex/Claude/opencode MCP clients.
    let transport = transport::stdio();
    let running = server
        .serve(transport)
        .await
        .map_err(|err| OpenMcpGdbError::Worker(err.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|err| OpenMcpGdbError::Worker(err.to_string()))?;
    Ok(())
}

async fn run_http_server(url: Url, factory: OpenMcpGdbServerFactory) -> Result<()> {
    let host = url
        .host_str()
        .ok_or_else(|| OpenMcpGdbError::InvalidUrl("missing host in mcp_server_url".to_string()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| OpenMcpGdbError::InvalidUrl("missing port in mcp_server_url".to_string()))?;

    let path = if url.path().is_empty() {
        "/mcp"
    } else {
        url.path()
    };
    let bind_addr = format!("{host}:{port}");

    // rmcp manages per-client sessions for streamable-http mode.
    let cancellation_token = CancellationToken::new();
    let config = StreamableHttpServerConfig::default()
        .with_json_response(true)
        .with_cancellation_token(cancellation_token.child_token());

    let service: StreamableHttpService<OpenMcpGdbServer, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(factory.build()), Default::default(), config);

    let router = axum::Router::new().nest_service(path, service);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(OpenMcpGdbError::Io)?;

    axum::serve(listener, router)
        .await
        .map_err(OpenMcpGdbError::Io)?;
    Ok(())
}
