use crate::agent::multi_agent::MultiAgent;
use dbx_core::Database;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
pub struct SidecarState(pub std::sync::Mutex<Option<tauri_plugin_shell::process::CommandChild>>);

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub content: String,
    pub tool_events: Vec<String>,
    pub timestamp: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionTree {
    pub session_id: String,
    pub forked_from_session_id: Option<String>,
    pub active_node_id: Option<String>,
    pub nodes: Vec<SessionNode>,
}

pub struct AgentState {
    pub multi_agent: Arc<MultiAgent>,
    pub base_dir: std::path::PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub db: Arc<Database>,
}
