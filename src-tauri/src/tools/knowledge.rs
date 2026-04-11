use dbx_core::Database;
use std::sync::Arc;

pub struct KnowledgeTool {
    db: Arc<Database>,
}

impl KnowledgeTool {
    pub fn new(db: Arc<Database>) -> Self {
        KnowledgeTool { db }
    }

    fn resolve_key(&self, domain: &str, name: &str) -> Result<String, String> {
        if !["skills", "rules", "workflows", "schedules", "locales"].contains(&domain) {
            return Err("Invalid domain. Must be 'skills', 'rules', 'workflows', 'schedules', or 'locales'.".to_string());
        }
        let ext_name =
            if !name.ends_with(".md") && !name.ends_with(".json") && !name.ends_with(".txt") {
                format!("{}.md", name)
            } else {
                name.to_string()
            };
        Ok(format!("{}:{}", domain, ext_name))
    }

    pub fn list(&self, domain: &str) -> Result<String, String> {
        if !["skills", "rules", "workflows", "schedules", "locales"].contains(&domain) {
            return Err("Invalid domain.".to_string());
        }
        let prefix = format!("{}:", domain);
        let mut files = Vec::new();

        if let Ok(entries) = self.db.scan("knowledge_base") {
            for (key, _) in entries {
                if let Ok(key_str) = String::from_utf8(key) {
                    if key_str.starts_with(&prefix) {
                        files.push(key_str.replace(&prefix, ""));
                    }
                }
            }
        }

        if files.is_empty() {
            return Ok(format!("No items found in {}.", domain));
        }
        Ok(files.join("\n"))
    }

    pub fn read(&self, domain: &str, name: &str) -> Result<String, String> {
        let key = self.resolve_key(domain, name)?;
        match self.db.get("knowledge_base", key.as_bytes()) {
            Ok(Some(content_bytes)) => match String::from_utf8(content_bytes) {
                Ok(content) => Ok(content),
                Err(e) => Err(format!("UTF-8 parsing error: {}", e)),
            },
            Ok(None) => Err(format!("Item {} not found in {}.", name, domain)),
            Err(e) => Err(format!("Database error reading '{}': {}", name, e)),
        }
    }

    pub fn write(&self, domain: &str, name: &str, content: &str) -> Result<String, String> {
        let mut final_name = name.to_string();
        if domain == "schedules" {
            if !final_name.ends_with(".json") {
                final_name = format!(
                    "{}.json",
                    final_name.trim_end_matches(".md").trim_end_matches(".txt")
                );
            }
            if let Err(e) = serde_json::from_str::<crate::agent::scheduler::ScheduleConfig>(content)
            {
                return Err(format!("CRITICAL ERROR: Schedule content MUST be valid JSON matching the ScheduleConfig schema! Parse error: {}\n\nYou provided plain text or invalid JSON. Please correct it and output ONLY the JSON object.", e));
            }
        }
        let key = self.resolve_key(domain, &final_name)?;
        match self
            .db
            .insert("knowledge_base", key.as_bytes(), content.as_bytes())
        {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!("Successfully wrote {} to {}.", name, domain))
            }
            Err(e) => Err(format!("Failed to write {}: {}", key, e)),
        }
    }

    pub fn delete(&self, domain: &str, name: &str) -> Result<String, String> {
        let key = self.resolve_key(domain, name)?;
        match self.db.delete("knowledge_base", key.as_bytes()) {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!("Successfully deleted {} from {}.", name, domain))
            }
            Err(e) => Err(format!("Failed to delete {}: {}", key, e)),
        }
    }

    pub fn read_all_schedules(&self) -> String {
        let mut all_schedules = Vec::new();
        let prefix = "schedules:";

        if let Ok(entries) = self.db.scan("knowledge_base") {
            for (key, val) in entries {
                if let Ok(key_str) = String::from_utf8(key) {
                    if key_str.starts_with(prefix) {
                        let file_name = key_str.replace(prefix, "");
                        if let Ok(content) = String::from_utf8(val) {
                            all_schedules.push(format!("---\n[{}]\n{}\n", file_name, content));
                        }
                    }
                }
            }
        }

        if all_schedules.is_empty() {
            "No schedules registered yet.".to_string()
        } else {
            all_schedules.join("\n")
        }
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
        llm: Option<crate::agent::llm_client::LLMClient>,
        registry_prompt: Option<String>,
    ) -> crate::agent::multi_agent::ToolResult {
        let domain = args.get("domain").and_then(|d| d.as_str()).unwrap_or("");
        let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");

        match action.as_str() {
            "list" => {
                let result = self.list(domain);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "read" => {
                let result = self.read(domain, name);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "write" => {
                let mut content = args
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                if (domain == "schedules" || domain == "skills" || domain == "rules")
                    && serde_json::from_str::<serde_json::Value>(&content).is_err()
                {
                    if let (Some(client), Some(prompt)) = (&llm, &registry_prompt) {
                        let now_str = chrono::Local::now()
                            .format("%Y-%m-%d %A %H:%M:%S")
                            .to_string();
                        let prompt_with_context = format!("{}\n\n[SYSTEM TIME ANCHOR]\nCurrent System Time: {}\n\nUser Request to convert to JSON:\n{}", prompt, now_str, content);
                        let msgs = vec![crate::agent::llm_client::ChatMessage {
                            role: "user".to_string(),
                            content: prompt_with_context,
                            images_base64: None,
                        }];
                        if let Ok(res) = client.chat(&msgs, 0.1).await {
                            let ai_text = crate::agent::orchestrator::Orchestrator::sanitize_output(
                                &res.content,
                            );
                            let json_blocks = crate::agent::parser::extract_json_blocks(&ai_text);
                            if let Some(first_block) = json_blocks.first() {
                                content =
                                    serde_json::to_string_pretty(first_block).unwrap_or(content);
                            } else {
                                content = ai_text;
                            }
                        }
                    }
                }

                let result = self.write(domain, name, &content);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "delete" => {
                let result = self.delete(domain, name);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            _ => crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unsupported action for knowledge".into(),
            },
        }
    }
}
