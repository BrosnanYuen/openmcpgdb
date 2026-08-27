use thiserror::Error;

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, OpenMcpGdbError>;

#[derive(Debug, Error)]
pub enum OpenMcpGdbError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "config file not found: {path}\n  help: pass a JSON config file as the first argument,\n        or create one (an empty JSON object {{}} is valid),\n        or run without arguments to start with built-in defaults"
    )]
    ConfigNotFound { path: PathBuf },
    #[error("failed to parse config file {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("gdb error: {0}")]
    Gdb(String),
    #[error("session closed")]
    SessionClosed,
    #[error("worker error: {0}")]
    Worker(String),
}
