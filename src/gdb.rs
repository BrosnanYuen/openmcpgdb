use crate::{
    config::{ServerConfig, split_gdb_options},
    error::{OpenMcpGdbError, Result},
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    time::timeout,
};

#[async_trait]
pub trait GdbBackend: Send + Sync {
    async fn start(&mut self, executable_path: &Path) -> Result<()>;
    async fn exec(&mut self, command: &str) -> Result<String>;
    async fn stop(&mut self) -> Result<()>;
    /// Send SIGINT to the GDB process to interrupt a running debuggee.
    async fn interrupt(&mut self) -> Result<()>;
}

pub trait GdbBackendFactory: Send + Sync {
    fn create(&self, config: &ServerConfig) -> Box<dyn GdbBackend>;
}

#[derive(Debug, Clone, Default)]
pub struct RealGdbBackendFactory;

impl GdbBackendFactory for RealGdbBackendFactory {
    fn create(&self, config: &ServerConfig) -> Box<dyn GdbBackend> {
        Box::new(RealGdbBackend::new(config.clone()))
    }
}

pub struct RealGdbBackend {
    config: ServerConfig,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    stderr: Option<ChildStderr>,
    /// Set when an exec() timed out before observing the sentinel. GDB may
    /// still have output (including a queued sentinel printf) pending, so the
    /// next exec() must drain the stream first to stay aligned.
    needs_resync: bool,
}

impl RealGdbBackend {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            stdout: None,
            stderr: None,
            needs_resync: false,
        }
    }

    fn ensure_started(&self) -> Result<()> {
        if self.child.is_none() {
            return Err(OpenMcpGdbError::Gdb(
                "gdb process not started, call gdb_execute first".to_string(),
            ));
        }
        Ok(())
    }

    /// Realign the gdb command stream after an exec() timeout. Interrupts the
    /// inferior so gdb returns to the prompt, then drains queued output (which
    /// may include a stale sentinel printf from the timed-out command) until
    /// the pipe goes quiet. The next command then reads a clean stream.
    async fn resync_stream(&mut self) -> Result<()> {
        let _ = self.interrupt().await;
        if let Some(stdout) = self.stdout.as_mut() {
            let deadline = Instant::now() + Duration::from_secs(5);
            let quiet_window = Duration::from_millis(300);
            let mut last_data: Option<Instant> = None;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                if let Some(last) = last_data
                    && now - last >= quiet_window
                {
                    // No output for a quiet window: gdb has consumed all
                    // queued commands, so the stream is aligned again.
                    break;
                }
                let mut line = String::new();
                let remaining = deadline.saturating_duration_since(now);
                match timeout(remaining, stdout.read_line(&mut line)).await {
                    Ok(Ok(0)) => break, // gdb exited
                    Ok(Ok(_)) => last_data = Some(Instant::now()),
                    Ok(Err(err)) => return Err(OpenMcpGdbError::Io(err)),
                    Err(_) => break,
                }
            }
        }
        self.needs_resync = false;
        Ok(())
    }
}

#[async_trait]
impl GdbBackend for RealGdbBackend {
    async fn start(&mut self, executable_path: &Path) -> Result<()> {
        // If gdb is already running, stop it first before starting a new target.
        if self.child.is_some() {
            self.stop().await?;
        }

        // Build the process command from configured gdb binary and options.
        let mut command = Command::new(&self.config.gdb_path);
        for option in split_gdb_options(&self.config.gdb_options) {
            command.arg(option);
        }
        // An empty path starts bare gdb (used by attach-to-pid, which loads
        // symbol information from the live process instead).
        if !executable_path.as_os_str().is_empty() {
            command.arg(executable_path);
        }
        command
            .arg("--quiet")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Spawn gdb and extract piped streams used for command/response exchange.
        let mut child = command.spawn().map_err(OpenMcpGdbError::Io)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| OpenMcpGdbError::Gdb("failed to get gdb stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| OpenMcpGdbError::Gdb("failed to get gdb stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| OpenMcpGdbError::Gdb("failed to get gdb stderr".to_string()))?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.stderr = Some(stderr);
        self.child = Some(child);
        // A fresh gdb process starts with an aligned stream.
        self.needs_resync = false;

        // Normalize gdb behavior for stable machine-driven command parsing.
        let _ = self.exec("set pagination off").await;
        let _ = self.exec("set confirm off").await;
        // Route inferior stdout/stderr away from gdb console to avoid mixing tool responses
        // with target program output.
        let _ = self.exec("set inferior-tty /dev/null").await;
        Ok(())
    }

    async fn exec(&mut self, command: &str) -> Result<String> {
        self.ensure_started()?;

        // A previous command may have timed out before its sentinel printf was
        // consumed by gdb, leaving queued output on the wire. Drain it first so
        // this command's response is not polluted (or truncated by a stale
        // sentinel).
        if self.needs_resync {
            self.resync_stream().await?;
        }

        // Send the user command and append a sentinel printf to delimit the response.
        let marker = "__OPENMCPGDB_DONE__";
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| OpenMcpGdbError::Gdb("gdb stdin unavailable".to_string()))?;
        stdin
            .write_all(format!("{command}\n").as_bytes())
            .await
            .map_err(OpenMcpGdbError::Io)?;
        stdin
            .write_all(format!("printf \"{marker}\\n\"\n").as_bytes())
            .await
            .map_err(OpenMcpGdbError::Io)?;
        stdin.flush().await.map_err(OpenMcpGdbError::Io)?;

        // Drain gdb output lines until the sentinel is observed.
        // Some commands (notably `continue`) may not promptly return to the prompt,
        // so we use a bounded wait and fall back to state inference.
        // Remote `target remote` needs a longer handshake over the network.
        let command_timeout = command_timeout_for(command);
        let command_start = Instant::now();
        let mut output = String::new();
        let mut saw_marker = false;
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| OpenMcpGdbError::Gdb("gdb stdout unavailable".to_string()))?;
        loop {
            let elapsed = command_start.elapsed();
            if elapsed >= command_timeout {
                break;
            }

            let mut line = String::new();
            let remaining = command_timeout
                .checked_sub(elapsed)
                .unwrap_or(Duration::from_millis(1));
            let count = match timeout(remaining, stdout.read_line(&mut line)).await {
                Ok(result) => result.map_err(OpenMcpGdbError::Io)?,
                Err(_) => break,
            };
            if count == 0 {
                break;
            }
            if line.contains(marker) {
                saw_marker = true;
                break;
            }
            output.push_str(&line);
        }

        // If we never saw this command's sentinel, gdb may still have queued
        // output. Arrange for the next exec() to resync the stream first.
        self.needs_resync = !saw_marker;

        // Collect available stderr output to preserve command failures that GDB emits on stderr.
        // Remote failures (e.g. connection refused) can appear slightly delayed.
        if let Some(stderr) = self.stderr.as_mut() {
            loop {
                let mut buf = [0u8; 1024];
                match timeout(Duration::from_millis(150), stderr.read(&mut buf)).await {
                    Ok(Ok(0)) => break,
                    Ok(Ok(count)) => {
                        output.push_str(&String::from_utf8_lossy(&buf[..count]));
                        if count < buf.len() {
                            break;
                        }
                    }
                    Ok(Err(err)) => return Err(OpenMcpGdbError::Io(err)),
                    Err(_) => break,
                }
            }
        }

        Ok(output)
    }

    async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() {
            // Best effort graceful shutdown by asking gdb to quit first.
            if let Some(stdin) = self.stdin.as_mut() {
                let _ = stdin.write_all(b"quit\ny\n").await;
                let _ = stdin.flush().await;
            }
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill().await;
            }
        }

        self.child = None;
        self.stdin = None;
        self.stdout = None;
        self.stderr = None;
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        if let Some(child) = self.child.as_mut()
            && let Some(pid) = child.id()
        {
            // Send SIGINT to the gdb process to interrupt it.
            // This will cause gdb to send an interrupt packet to a remote stub
            // (for `target remote`) or stop the local inferior.
            let _ = unsafe { libc::kill(pid as i32, libc::SIGINT) };
            // Remote stubs need more time for the interrupt packet round-trip.
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok(())
    }
}

/// Per-command GDB timeout. `continue`/`run` fall back to `Running` quickly
/// (local case) while `target remote` needs time for network handshake and
/// symbol loading. Other commands get a moderate 10s window.
fn command_timeout_for(command: &str) -> Duration {
    let trimmed = command.trim_start();
    if trimmed.starts_with("target remote") || trimmed.starts_with("target extended-remote") {
        Duration::from_secs(15)
    } else if trimmed == "continue" || trimmed == "run" {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(10)
    }
}

#[derive(Clone, Default)]
pub struct MockBackendHandle {
    inner: Arc<Mutex<MockBackendState>>,
}

#[derive(Default)]
struct MockBackendState {
    pub started: bool,
    pub commands: Vec<String>,
    pub responses: HashMap<String, String>,
    pub errors: HashMap<String, String>,
    pub default_response: String,
}

impl MockBackendHandle {
    pub fn with_default_response(response: impl Into<String>) -> Self {
        let handle = Self::default();
        if let Ok(mut state) = handle.inner.lock() {
            state.default_response = response.into();
        }
        handle
    }

    pub fn set_response(&self, command: &str, response: &str) {
        if let Ok(mut state) = self.inner.lock() {
            state
                .responses
                .insert(command.to_string(), response.to_string());
        }
    }

    pub fn set_error(&self, command: &str, error: &str) {
        if let Ok(mut state) = self.inner.lock() {
            state.errors.insert(command.to_string(), error.to_string());
        }
    }

    pub fn commands(&self) -> Vec<String> {
        if let Ok(state) = self.inner.lock() {
            return state.commands.clone();
        }
        Vec::new()
    }
}

pub struct MockGdbBackend {
    handle: MockBackendHandle,
}

impl MockGdbBackend {
    pub fn new(handle: MockBackendHandle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl GdbBackend for MockGdbBackend {
    async fn start(&mut self, _executable_path: &Path) -> Result<()> {
        let mut state = self
            .handle
            .inner
            .lock()
            .map_err(|_| OpenMcpGdbError::Worker("mock backend poisoned".to_string()))?;
        state.started = true;
        Ok(())
    }

    async fn exec(&mut self, command: &str) -> Result<String> {
        let mut state = self
            .handle
            .inner
            .lock()
            .map_err(|_| OpenMcpGdbError::Worker("mock backend poisoned".to_string()))?;
        state.commands.push(command.to_string());
        if let Some(error) = state.errors.get(command) {
            return Err(OpenMcpGdbError::Gdb(error.clone()));
        }
        if let Some(value) = state.responses.get(command) {
            return Ok(value.clone());
        }
        Ok(state.default_response.clone())
    }

    async fn stop(&mut self) -> Result<()> {
        let mut state = self
            .handle
            .inner
            .lock()
            .map_err(|_| OpenMcpGdbError::Worker("mock backend poisoned".to_string()))?;
        state.started = false;
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct MockGdbBackendFactory {
    handle: MockBackendHandle,
}

impl MockGdbBackendFactory {
    pub fn new(handle: MockBackendHandle) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> MockBackendHandle {
        self.handle.clone()
    }
}

impl GdbBackendFactory for MockGdbBackendFactory {
    fn create(&self, _config: &ServerConfig) -> Box<dyn GdbBackend> {
        Box::new(MockGdbBackend::new(self.handle.clone()))
    }
}
