use crate::error::{OpenMcpGdbError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Server configuration.
///
/// Every field has a sensible default: an empty JSON object (`{}`) is a valid
/// config file, and the file itself is optional. Treat the config as a way to
/// override defaults rather than a required checklist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// GDB binary. May be an absolute path or a command name resolved via `PATH`.
    #[serde(default = "default_gdb_path")]
    pub gdb_path: PathBuf,
    #[serde(default)]
    pub gdb_options: String,
    /// Source root used to resolve relative source paths reported by gdb.
    /// Defaults to the directory the server was started from.
    #[serde(default = "default_codebase_dir")]
    pub codebase_dir: PathBuf,
    /// Optional default binary to attach to when `gdb_run`/`gdb_target_remote`
    /// is called before any `gdb_execute`. Usually unnecessary because
    /// `gdb_execute` receives the executable per call.
    #[serde(default)]
    pub executable_path: PathBuf,
    #[serde(default = "default_server_name")]
    pub mcp_server_name: String,
    /// Transport endpoint. `stdio://` (default) serves MCP over stdin/stdout
    /// for direct registration with MCP clients; use `http://host:port` to
    /// serve streamable HTTP instead.
    #[serde(default = "default_server_url")]
    pub mcp_server_url: String,
    #[serde(default = "default_lines_before")]
    pub display_lines_before_current: usize,
    #[serde(default = "default_lines_after")]
    pub display_lines_after_current: usize,
    #[serde(default = "default_backtrace")]
    pub display_backtrace: usize,
    #[serde(default = "default_variable_list")]
    pub display_variable_list: usize,
    #[serde(default)]
    pub display_join_current_code: bool,
}

fn default_gdb_path() -> PathBuf {
    PathBuf::from("gdb")
}

fn default_codebase_dir() -> PathBuf {
    PathBuf::from(".")
}

fn default_server_name() -> String {
    "MCP GDB Server".to_string()
}

fn default_server_url() -> String {
    // stdio is the standard transport for spawning MCP servers from clients.
    "stdio://".to_string()
}

fn default_lines_before() -> usize {
    7
}

fn default_lines_after() -> usize {
    8
}

fn default_backtrace() -> usize {
    6
}

fn default_variable_list() -> usize {
    9
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            gdb_path: default_gdb_path(),
            gdb_options: String::new(),
            codebase_dir: default_codebase_dir(),
            executable_path: PathBuf::new(),
            mcp_server_name: default_server_name(),
            mcp_server_url: default_server_url(),
            display_lines_before_current: default_lines_before(),
            display_lines_after_current: default_lines_after(),
            display_backtrace: default_backtrace(),
            display_variable_list: default_variable_list(),
            display_join_current_code: false,
        }
    }
}

impl ServerConfig {
    /// Load and validate a config file. Errors name the offending file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => OpenMcpGdbError::ConfigNotFound {
                path: path.to_path_buf(),
            },
            _ => OpenMcpGdbError::Io(err),
        })?;
        let mut config: Self =
            serde_json::from_str(&data).map_err(|source| OpenMcpGdbError::ConfigParse {
                path: path.to_path_buf(),
                source,
            })?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration and normalize paths to absolute form so
    /// they stay valid regardless of later working-directory changes.
    pub fn validate(&mut self) -> Result<()> {
        self.gdb_path = resolve_gdb_path(&self.gdb_path)?;
        self.codebase_dir = make_absolute(&self.codebase_dir);
        if !self.executable_path.as_os_str().is_empty() {
            self.executable_path = make_absolute(&self.executable_path);
        }
        if self.display_backtrace == 0 {
            return Err(OpenMcpGdbError::InvalidConfig(
                "display_backtrace must be > 0".to_string(),
            ));
        }
        if self.display_variable_list == 0 {
            return Err(OpenMcpGdbError::InvalidConfig(
                "display_variable_list must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Resolve the gdb binary: absolute/relative paths are checked for existence,
/// bare command names are looked up in `PATH` and expanded to an absolute path.
fn resolve_gdb_path(gdb_path: &Path) -> Result<PathBuf> {
    let candidate = if gdb_path.is_absolute() {
        gdb_path.to_path_buf()
    } else if gdb_path.components().count() > 1 {
        // Contains a directory component (e.g. ./gdb or tools/gdb): relative to cwd.
        make_absolute(gdb_path)
    } else {
        find_in_path(gdb_path).ok_or_else(|| {
            OpenMcpGdbError::InvalidConfig(format!(
                "gdb binary {:?} not found in PATH; set gdb_path in the config to an absolute gdb location",
                gdb_path.display()
            ))
        })?
    };

    if !candidate.is_file() {
        return Err(OpenMcpGdbError::InvalidConfig(format!(
            "gdb_path {:?} does not exist or is not a regular file",
            candidate.display()
        )));
    }
    Ok(candidate)
}

fn find_in_path(command: &Path) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(command))
        .find(|candidate| candidate.is_file())
}

fn make_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::OpenMcpGdbError;

    /// A config pointing gdb_path at an existing regular file so validation
    /// does not depend on gdb being installed on the test machine.
    fn valid_config_with_stub_gdb(dir: &Path) -> ServerConfig {
        let stub = dir.join("gdb-stub");
        std::fs::write(&stub, "#!/bin/sh\n").expect("write gdb stub");
        ServerConfig {
            gdb_path: stub,
            ..Default::default()
        }
    }

    #[test]
    fn default_config_matches_documented_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.gdb_path, PathBuf::from("gdb"));
        assert_eq!(config.mcp_server_url, "stdio://");
        assert_eq!(config.mcp_server_name, "MCP GDB Server");
        assert_eq!(config.codebase_dir, PathBuf::from("."));
        assert!(config.executable_path.as_os_str().is_empty());
        assert_eq!(config.display_lines_before_current, 7);
        assert_eq!(config.display_lines_after_current, 8);
        assert_eq!(config.display_backtrace, 6);
        assert_eq!(config.display_variable_list, 9);
        assert!(!config.display_join_current_code);
    }

    #[test]
    fn empty_json_object_is_a_valid_config() {
        let mut config: ServerConfig = serde_json::from_str("{}").expect("{} should parse");
        assert_eq!(config.mcp_server_url, "stdio://");
        assert_eq!(config.display_backtrace, 6);

        // Validation only fails because the default gdb binary may be absent
        // in the test environment; point it at a stub to prove {} validates.
        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        config.gdb_path = valid_config_with_stub_gdb(&dir).gdb_path;
        config
            .validate()
            .expect("empty object config should validate");
    }

    #[test]
    fn partial_config_overrides_only_given_fields() {
        let json = r#"{ "display_backtrace": 42, "mcp_server_url": "http://localhost:9000" }"#;
        let mut config: ServerConfig =
            serde_json::from_str(json).expect("partial config should parse");
        assert_eq!(config.display_backtrace, 42);
        assert_eq!(config.mcp_server_url, "http://localhost:9000");
        // Untouched fields keep their defaults.
        assert_eq!(config.display_variable_list, 9);
        assert_eq!(config.gdb_path, PathBuf::from("gdb"));

        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        config.gdb_path = valid_config_with_stub_gdb(&dir).gdb_path;
        config.validate().expect("partial config should validate");
    }

    #[test]
    fn from_file_missing_file_names_the_path() {
        let err = ServerConfig::from_file(Path::new("/nonexistent/dir/config.json"))
            .expect_err("missing file should error");
        let message = err.to_string();
        match err {
            OpenMcpGdbError::ConfigNotFound { path } => {
                assert_eq!(path, PathBuf::from("/nonexistent/dir/config.json"));
                assert!(
                    message.contains("/nonexistent/dir/config.json"),
                    "error should include the path: {message}"
                );
                assert!(
                    message.to_lowercase().contains("help"),
                    "error should include guidance: {message}"
                );
            }
            other => panic!("expected ConfigNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn from_file_invalid_json_reports_file_and_location() {
        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("broken.json");
        std::fs::write(&path, "{ not json").expect("write broken config");

        let err = ServerConfig::from_file(&path).expect_err("invalid json should error");
        let message = err.to_string();
        match err {
            OpenMcpGdbError::ConfigParse { path: p, source } => {
                assert_eq!(p, path);
                assert!(
                    message.contains("line 1"),
                    "parse error should point at the location: {source}"
                );
            }
            other => panic!("expected ConfigParse, got: {other:?}"),
        }
    }

    #[test]
    fn validate_resolves_relative_paths_against_cwd() {
        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let mut config = valid_config_with_stub_gdb(&dir);
        config.codebase_dir = PathBuf::from("sub/dir");
        config.executable_path = PathBuf::from("bin/app");

        config.validate().expect("validate should succeed");

        let cwd = std::env::current_dir().expect("cwd");
        assert!(config.codebase_dir.is_absolute());
        assert_eq!(config.codebase_dir, cwd.join("sub/dir"));
        assert_eq!(config.executable_path, cwd.join("bin/app"));
    }

    #[test]
    fn validate_keeps_unset_executable_path_empty() {
        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let mut config = valid_config_with_stub_gdb(&dir);
        config.validate().expect("validate should succeed");
        assert!(config.executable_path.as_os_str().is_empty());
    }

    #[test]
    fn validate_reports_missing_gdb_command_informatively() {
        let mut config = ServerConfig {
            gdb_path: PathBuf::from("definitely-not-a-real-gdb-binary-xyz"),
            ..Default::default()
        };

        let err = config.validate().expect_err("unknown command should fail");
        let message = err.to_string();
        assert!(
            message.contains("not found in PATH") && message.contains("gdb_path"),
            "error should explain the PATH lookup failure: {message}"
        );
    }

    #[test]
    fn validate_rejects_nonexistent_absolute_gdb_path() {
        let mut config = ServerConfig {
            gdb_path: PathBuf::from("/nonexistent/gdb"),
            ..Default::default()
        };

        let err = config.validate().expect_err("bad path should fail");
        assert!(
            err.to_string().contains("/nonexistent/gdb"),
            "error should include resolved path: {err}"
        );
    }

    #[test]
    fn validate_rejects_zero_display_limits() {
        let dir = std::env::temp_dir().join("openmcpgdb-config-tests");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let mut config = valid_config_with_stub_gdb(&dir);
        config.display_backtrace = 0;
        assert!(config.validate().is_err());

        let mut config = valid_config_with_stub_gdb(&dir);
        config.display_variable_list = 0;
        assert!(config.validate().is_err());
    }
}
