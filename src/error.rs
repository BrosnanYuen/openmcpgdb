use thiserror::Error;

pub type Result<T> = std::result::Result<T, OpenMcpGdbError>;

#[derive(Debug, Error)]
pub enum OpenMcpGdbError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
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
