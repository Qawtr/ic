use ic_canister_log::declare_log_buffer;
use serde::{Deserialize, Serialize};

// High-priority messages.
declare_log_buffer!(name = P0, capacity = 1000);

// Low-priority info messages.
declare_log_buffer!(name = P1, capacity = 1000);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Priority {
    P0,
    P1,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub priority: Priority,
    pub file: String,
    pub line: u32,
    pub message: String,
    pub counter: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Log {
    pub entries: Vec<LogEntry>,
}
