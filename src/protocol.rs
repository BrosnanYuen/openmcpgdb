use rmcp::schemars;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub enum DebuggerState {
    #[serde(rename = "not attached")]
    NotAttached,
    #[serde(rename = "failed to attach")]
    FailedToAttach,
    #[serde(rename = "attached")]
    Attached,
    #[serde(rename = "stopped at breakpoint")]
    StoppedAtBreakpoint,
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

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DebuggerResponse {
    pub debugger_state: DebuggerState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_list: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breakpoints: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrace: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_func: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_code: Option<CurrentCodePayload>,
    #[serde(default)]
    pub error: String,
}

impl DebuggerResponse {
    pub fn new(debugger_state: DebuggerState) -> Self {
        Self {
            debugger_state,
            variable_list: None,
            breakpoints: None,
            backtrace: None,
            current_func: None,
            current_code_path: None,
            current_code_line: None,
            current_code: None,
            error: String::new(),
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}
