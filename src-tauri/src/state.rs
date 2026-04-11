use crate::agent::multi_agent::MultiAgent;
use dbx_core::Database;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AgentState {
    pub multi_agent: Arc<MultiAgent>,
    pub base_dir: std::path::PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub db: Arc<Database>,
}
