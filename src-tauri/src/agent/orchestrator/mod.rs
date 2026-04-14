use crate::agent::llm_client::{ChatMessage, LLMClient};
use crate::agent::multi_agent::MultiAgent;
use chrono::Local;
use dbx_core::Database;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;

pub struct OrchestratorRouting {
    pub endpoints: Vec<crate::config::LlmEndpoint>,
    pub planner_id: String,
    pub critic_id: String,
    pub writer_id: String,
    pub worker_id: String,
    pub reflector_id: String,
    pub registry_id: String,
}

pub struct Orchestrator {
    routing: OrchestratorRouting,
    multi_agent: Arc<MultiAgent>,
    base_dir: std::path::PathBuf,
    db: Arc<Database>,
}

impl Orchestrator {
    pub fn new(
        routing: OrchestratorRouting,
        multi_agent: Arc<MultiAgent>,
        base_dir: std::path::PathBuf,
        db: Arc<Database>,
    ) -> Self {
        Self {
            routing,
            multi_agent,
            base_dir,
            db,
        }
    }

    fn get_llm_client_by_id(&self, target_id: &str) -> LLMClient {
        // Find the endpoint by ID or fallback to the first active one, or a default.
        let endpoint = self
            .routing
            .endpoints
            .iter()
            .find(|e| e.id == target_id && e.is_enabled)
            .or_else(|| self.routing.endpoints.iter().find(|e| e.is_enabled))
            .cloned()
            .unwrap_or_else(|| crate::config::LlmEndpoint {
                id: "default".to_string(),
                name: "Fallback Default".to_string(),
                api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
                model: "gemma-4".to_string(),
                api_key: "".to_string(),
                is_enabled: true,
            });

        LLMClient::new(endpoint.api_url, endpoint.model, endpoint.api_key)
    }

    fn get_llm_for_planner(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.planner_id)
    }

    pub fn get_llm_for_critic(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.critic_id)
    }

    pub fn get_llm_for_writer(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.writer_id)
    }

    pub fn get_llm_for_reflector(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.reflector_id)
    }

    pub fn get_llm_for_registry(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.registry_id)
    }

    fn get_llm_for_worker(&self) -> LLMClient {
        self.get_llm_client_by_id(&self.routing.worker_id)
    }

    pub fn get_fallback_endpoints(&self, primary_id: &str) -> Vec<crate::config::LlmEndpoint> {
        let mut fallbacks = Vec::new();
        // Return all enabled endpoints EXCEPT the primary one we just tried
        for ep in &self.routing.endpoints {
            if ep.is_enabled && ep.id != primary_id {
                fallbacks.push(ep.clone());
            }
        }
        fallbacks
    }

    pub fn get_lang_display(&self, language: &str) -> String {
        let key = format!("locales:{}.json", language);
        if let Ok(Some(bytes)) = self.db.get("knowledge_base", key.as_bytes()) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(display) = json
                    .get("settings.lang_custom_display")
                    .and_then(|v| v.as_str())
                {
                    return display.to_string();
                }
            }
        }
        match language {
            "en" => "English(Native)".to_string(),
            "ko" => "Korean(한국어)".to_string(),
            _ => language.to_string(),
        }
    }

    pub fn resolve_i18n(log_msg: &str, language: &str, db: &Database) -> String {
        if !log_msg.starts_with("i18n:") {
            return log_msg.to_string();
        }
        let json_str = &log_msg[5..];
        let val: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => return log_msg.to_string(),
        };

        let key = val.get("key").and_then(|k| k.as_str()).unwrap_or("");
        let args = val.get("args").and_then(|a| a.as_object());

        let locale_key = format!("locales:{}.json", language);
        let mut resolved = key.to_string();

        if let Ok(Some(bytes)) = db.get("knowledge_base", locale_key.as_bytes()) {
            if let Ok(locale_json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                // Handle nested keys like "log.planner_planning"
                let mut current = &locale_json;
                let mut found = true;
                for part in key.split('.') {
                    if let Some(next) = current.get(part) {
                        current = next;
                    } else {
                        found = false;
                        break;
                    }
                }

                if found {
                    if let Some(template) = current.as_str() {
                        resolved = template.to_string();
                        if let Some(args_map) = args {
                            for (k, v) in args_map {
                                let placeholder = format!("{{{}}}", k);
                                let val_str = match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => v.to_string(),
                                };
                                resolved = resolved.replace(&placeholder, &val_str);
                            }
                        }
                    }
                }
            }
        }
        resolved
    }

    /// Extracts duplicated environment context building (time, brain artifacts, schedules)
    fn build_context(
        &self,
    ) -> (
        String,
        String,
        String,
        Vec<(String, crate::agent::scheduler::ScheduleConfig)>,
        String,
    ) {
        let current_time = chrono::Local::now()
            .format("%Y-%m-%d %A %H:%M:%S")
            .to_string();

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

        let mut rules_str = String::from("[GLOBAL BEHAVIOR RULES]\nThese rules apply to ALL your actions and final outputs. You MUST obey them:\n- [ANTI-SPLINTERING POLICY]: When saving data to the `brain` or `knowledge` tool (skills, workflows, schedules), ALWAYS check existing item lists first. If an item for the same topic or purpose already exists, you MUST `read` it first, merge the new data, and overwrite it using the EXACT SAME name. DO NOT create fragmented versions like `topic_2.md` or `schedule_v2`.\n");
        let mut modules_str = String::from("\n[AVAILABLE CUSTOM SKILLS & WORKFLOWS]\nThe following modules are registered in your database. If a requested task matches these names, you MUST use the `knowledge` tool (action=\"read\") to read their exact instructions before starting:\n");

        if let Ok(records) = self.db.scan("knowledge_base") {
            let mut rules_count = 0;
            let mut modules_count = 0;

            for (key, val) in records {
                if val == b"__PUM_DELETED__" {
                    continue;
                }
                let k_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = k_str.split(':').collect();
                if parts.len() == 2 {
                    if k_str.starts_with("rules:") {
                        let content = String::from_utf8_lossy(&val);
                        rules_str.push_str(&format!(
                            "- RULE [{}]:\n{}\n\n",
                            parts[1],
                            content.trim()
                        ));
                        rules_count += 1;
                    } else if k_str.starts_with("skills:") || k_str.starts_with("workflows:") {
                        let content = String::from_utf8_lossy(&val);
                        let mut preview = content.trim().replace('\n', " ");
                        if preview.chars().count() > 100 {
                            let snippet: String = preview.chars().take(100).collect();
                            preview = format!("{}...", snippet);
                        }
                        modules_str.push_str(&format!(
                            "- Domain: {}, Name: {} (Preview: {})\n",
                            parts[0], parts[1], preview
                        ));
                        modules_count += 1;
                    }
                }
            }

            if rules_count == 0 {
                rules_str.push_str("- No global rules defined.\n");
            }
            if modules_count == 0 {
                modules_str.push_str("- No custom skills or workflows learned yet.\n");
            }
        } else {
            rules_str.push_str("- No global rules defined.\n");
            modules_str.push_str("- No custom skills or workflows learned yet.\n");
        }

        let mut rules_combined = format!("{}{}", rules_str, modules_str);
        
        // Add follow-up suggestion instruction
        rules_combined.push_str("\n[FOLLOW-UP SUGGESTIONS]\n");
        rules_combined.push_str(crate::agent::prompts::get_suggestion_instruction());
        rules_combined.push_str("\n");

        let combined_rules_modules = rules_combined;

        (
            current_time,
            brain_files_md,
            schedule_files_md,
            pending_tasks,
            combined_rules_modules,
        )
    }

    pub fn sanitize_output(text: &str) -> String {
        // 1. Aggressively strip thinking/thought blocks first
        let text = crate::agent::parser::strip_thinking_blocks(text);
        let mut clean = text.to_string();

        // 2. Strip Gemma specific chain-of-thought tokens (only tool processing remnants)
        if let Some(pos) = clean.rfind("<|tool_response>") {
            clean = clean[pos + "<|tool_response>".len()..].to_string();
            if clean.starts_with("thought ") {
                clean = clean["thought ".len()..].to_string();
            }
        }

        while let Some(start) = clean.find("<|tool_call>") {
            if let Some(end) = clean.find("<tool_call|>") {
                let block = &clean[start..end + "<tool_call|>".len()];
                clean = clean.replace(block, "");
            } else {
                clean = clean.replace("<|tool_call>", "");
                break;
            }
        }

        if let Some(pos) = clean.rfind("<channel|>") {
            clean = clean[pos + "<channel|>".len()..].to_string();
        }
        // 3. Strip self correction checks sometimes emitted by models
        if let Some(pos) = clean.find("*(Self-Correction Check:") {
            clean = clean[..pos].to_string();
        }
        if let Some(pos) = clean.find("*(Self-Correction Check") {
            clean = clean[..pos].to_string();
        }
        
        clean.trim().to_string()
    }

    /// Saves the complete session transcript into the logs/ directory
    fn save_transcript(&self, session_id: Option<String>, history: &[ChatMessage]) {
        let logs_dir = self.base_dir.join("logs");
        if !logs_dir.exists() {
            let _ = fs::create_dir_all(&logs_dir);
        }

        let filename = match session_id {
            Some(id) if !id.is_empty() => format!("{}.md", id),
            _ => format!("Background_{}.md", Local::now().format("%y%m%d_%H%M%S")),
        };

        let filepath = logs_dir.join(&filename);

        let mut out = String::new();
        out.push_str(&format!(
            "# PumAgent Session Log - {}\n\n",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        for msg in history {
            out.push_str(&format!("### Role: {}\n", msg.role.to_uppercase()));

            // Strip out <think> blocks for a cleaner markdown log using the robust parser
            let clean_content = crate::agent::parser::strip_thinking_blocks(&msg.content);
            out.push_str(&format!("{}\n\n---\n", clean_content.trim()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_output_with_think() {
        let input = "<think>Secret thoughts</think>Final Answer";
        let output = Orchestrator::sanitize_output(input);
        assert_eq!(output, "Final Answer");
    }

    #[test]
    fn test_sanitize_output_complex() {
        let input = "<thought>Thinking...</thought>Hello World <|tool_call|>json{}<tool_call|>";
        let output = Orchestrator::sanitize_output(input);
        assert_eq!(output, "Hello World");
    }
}

pub mod critic;
mod multi_agent;
pub mod planner;
pub mod reflector;
mod single_agent;
pub mod writer;
