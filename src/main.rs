use openmcpgdb::{error::OpenMcpGdbError, run_from_config_file};
use std::path::PathBuf;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), OpenMcpGdbError> {
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.json"));

    run_from_config_file(&config_path).await
}
