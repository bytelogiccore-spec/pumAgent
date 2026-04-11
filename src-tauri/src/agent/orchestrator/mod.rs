
use crate::agent::llm_client::{ChatMessage, LLMClient};
use crate::agent::multi_agent::MultiAgent;
use chrono::Local;
use dbx_core::Database;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct CloudRoutingFlags {
    pub planner: bool,
    pub critic: bool,
    pub writer: bool,
    pub worker: bool,
}

pub struct Orchestrator {
    local_llm: LLMClient,
    cloud_llm: LLMClient,
    routing_flags: CloudRoutingFlags,
    multi_agent: Arc<MultiAgent>,
    base_dir: std::path::PathBuf,
    db: Arc<Database>,
}

impl Orchestrator {
    pub fn new(
        local_llm: LLMClient,
        cloud_llm: LLMClient,
        routing_flags: CloudRoutingFlags,
        multi_agent: Arc<MultiAgent>,
        base_dir: std::path::PathBuf,
        db: Arc<Database>,
    ) -> Self {
        Self {
            local_llm,
            cloud_llm,
            routing_flags,
            multi_agent,
            base_dir,
            db,
        }
    }

    fn get_llm_for_planner(&self) -> &LLMClient {
        if self.routing_flags.planner { &self.cloud_llm } else { &self.local_llm }
    }

    fn get_llm_for_critic(&self) -> &LLMClient {
        if self.routing_flags.critic { &self.cloud_llm } else { &self.local_llm }
    }

    fn get_llm_for_writer(&self) -> &LLMClient {
        if self.routing_flags.writer { &self.cloud_llm } else { &self.local_llm }
    }

    fn get_llm_for_worker(&self) -> &LLMClient {
        if self.routing_flags.worker { &self.cloud_llm } else { &self.local_llm }
    }

    /// Extracts duplicated environment context building (time, brain artifacts, schedules)
    fn build_context(&self) -> (String, String, String, Vec<(String, crate::agent::scheduler::ScheduleConfig)>) {
        let current_time = chrono::Local::now().format("%Y-%m-%d %A %H:%M:%S").to_string();

        let brain_tool = crate::tools::brain::BrainTool::new(self.db.clone());
        let brain_files_md = brain_tool
            .list_artifacts()
            .unwrap_or_else(|_| "No brain artifacts stored yet.".to_string());

        let scheduler = crate::agent::scheduler::Scheduler::new(self.db.clone());
        let (pending_tasks, schedules_summary, status_summary) = scheduler.evaluate_schedules();
        let schedule_files_md = format!(
            "{}\n\n[SCHEDULE STATUS]\n{}",
            schedules_summary, status_summary
        );

        (current_time, brain_files_md, schedule_files_md, pending_tasks)
    }

    /// Strips internal Gemma chain-of-thought tags like `<channel|>` from final user output
    pub fn sanitize_output(text: &str) -> String {
        let mut clean = text.to_string();
        if let Some(pos) = clean.rfind("<channel|>") {
            clean = clean[pos + "<channel|>".len()..].trim().to_string();
        }
        // Also strip self correction checks sometimes emitted by models
        if let Some(pos) = clean.find("*(Self-Correction Check:") {
            clean = clean[..pos].trim().to_string();
        }
        if let Some(pos) = clean.find("*(Self-Correction Check") {
            clean = clean[..pos].trim().to_string();
        }
        clean
    }

    /// Saves the complete session transcript into the logs/ directory
    fn save_transcript(&self, session_id: Option<String>, history: &[ChatMessage]) {
        let logs_dir = self.base_dir.join("logs");
        if !logs_dir.exists() {
            let _ = fs::create_dir_all(&logs_dir);
        }
        
        let filename = match session_id {
            Some(id) if !id.is_empty() => format!("{}_Session.md", id),
            _ => format!("Background_{}.md", Local::now().format("%y%m%d_%H%M%S")),
        };
        
        let filepath = logs_dir.join(&filename);

        let mut out = String::new();
        out.push_str(&format!("# PumAgent Session Log - {}\n\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
        for msg in history {
            out.push_str(&format!("### Role: {}\n", msg.role.to_uppercase()));
            out.push_str(&format!("{}\n\n---\n", msg.content));
        }

        // We truncate(true) because `history` contains the FULL array of messages from Svelte.
        // Overwriting guarantees the log exactly mirrors the active conversation without repeating past blocks.
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)
        {
            let _ = file.write_all(out.as_bytes());
        }
    }
}

mod single_agent;
mod multi_agent;
