use rmcp::schemars;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub enum DebuggerState {
    #[serde(rename = "not attached")]
    NotAttached,
    #[serde(rename = "failed to attach")]
    FailedToAttach,
    #[serde(rename = "gdbserver attached")]
    GdbServerAttached,
    #[serde(rename = "gdbserver failed to attach")]
    GdbServerFailedToAttach,
    #[serde(rename = "attached")]
    Attached,
    #[serde(rename = "stopped at breakpoint")]
    StoppedAtBreakpoint,
    #[serde(rename = "stopped at stepping")]
    StoppedAtStepping,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "sigsegv")]
    SigSegv,
    #[serde(rename = "sigabrt")]
    SigAbrt,
    #[serde(rename = "sigbus")]
    SigBus,
    #[serde(rename = "sigfpe")]
    SigFpe,
    #[serde(rename = "sigill")]
    SigIll,
    #[serde(rename = "sigtrap")]
    SigTrap,
    #[serde(rename = "sigterm")]
    SigTerm,
    #[serde(rename = "sigkill")]
    SigKill,
    #[serde(rename = "exited")]
    Exited,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CurrentCodePayload {
    Lines(BTreeMap<u64, String>),
    Joined(String),
}

/// One row of gdb's breakpoint/watchpoint listing.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BreakpointEntry {
    /// Breakpoint number ("2", or sub-id "2.1" for multiple locations).
    pub number: String,
    /// Entry kind as reported by gdb: "breakpoint", "hw watchpoint", ...
    pub kind: String,
    pub enabled: bool,
    /// Address/symbol/location and any extra notes for this entry.
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DebuggerResponse {
    pub debugger_state: DebuggerState,
    /// Why the debugger last stopped, e.g. "breakpoint 1", "watchpoint 2",
    /// "sigsegv", "exited". Present only when a specific cause was detected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_list: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakpoints: Option<Vec<BreakpointEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<BTreeMap<String, (String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_func: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code_line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code: Option<CurrentCodePayload>,
    /// Examined memory rows (address -> values) from gdb_examine_memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_output: Option<String>,
    #[serde(default)]
    pub error: String,
}

impl DebuggerResponse {
    pub fn new(debugger_state: DebuggerState) -> Self {
        Self {
            debugger_state,
            stop_reason: None,
            variable_list: None,
            breakpoints: None,
            backtrace: None,
            current_func: None,
            current_code_path: None,
            current_code_line: None,
            current_code: None,
            memory: None,
            command_output: None,
            error: String::new(),
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}
