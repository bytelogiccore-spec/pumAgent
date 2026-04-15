use crate::agent::llm_client::LLMClient;
use dbx_core::Database;
use std::sync::Arc;

pub struct BrainTool {
    db: Arc<Database>,
}

impl BrainTool {
    pub fn new(db: Arc<Database>) -> Self {
        BrainTool { db }
    }

    pub fn list_artifacts(&self) -> Result<String, String> {
        let mut list_md = String::from("### Available Brain Artifacts:\n");
        let mut count = 0;

        if let Ok(entries) = self.db.scan("brain_artifacts") {
            for (key, val) in entries {
                if val != b"__PUM_DELETED__" {
                    if let Ok(name) = String::from_utf8(key) {
                        let content = String::from_utf8_lossy(&val);
                        let mut preview = content.trim().replace('\n', " ");
                        if preview.chars().count() > 100 {
                            let snippet: String = preview.chars().take(100).collect();
                            preview = format!("{}...", snippet);
                        }
                        list_md.push_str(&format!("- **{}** (Preview: {})\n", name, preview));
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            return Ok("No brain artifacts stored yet.".to_string());
        }

        Ok(list_md)
    }

    pub fn read_artifact(&self, name: &str) -> Result<String, String> {
        match self.db.get("brain_artifacts", name.as_bytes()) {
            Ok(Some(content_bytes)) => match String::from_utf8(content_bytes) {
                Ok(content) => Ok(content),
                Err(e) => Err(format!("UTF-8 parsing error: {}", e)),
            },
            Ok(None) => Err(format!("Artifact '{}' not found.", name)),
            Err(e) => Err(format!("Database error reading '{}': {}", name, e)),
        }
    }

    pub fn write_artifact(&self, name: &str, content: &str) -> Result<String, String> {
        let trimmed_name = name.trim();
        let final_name = if trimmed_name.is_empty() {
            format!("Memory_{}", chrono::Local::now().format("%y%m%d_%H%M%S"))
        } else {
            trimmed_name.to_string()
        };

        let safe_name = if !final_name.ends_with(".md") {
            format!("{}.md", final_name)
        } else {
            final_name
        };

        match self
            .db
            .insert("brain_artifacts", safe_name.as_bytes(), content.as_bytes())
        {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(format!(
                    "Brain artifact '{}' successfully written/updated in DB.",
                    safe_name
                ))
            }
            Err(e) => Err(format!("Failed to write artifact '{}': {}", safe_name, e)),
        }
    }

    pub fn upsert_structured_memory(
        &self,
        facts: &[String],
        preferences: &[String],
        todos: &[String],
    ) -> Result<(), String> {
        let payload = serde_json::json!({
            "facts": facts,
            "preferences": preferences,
            "todos": todos,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        self.db
            .insert(
                "brain_artifacts",
                b"StructuredMemory.json",
                payload.to_string().as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        let _ = self.db.flush();
        Ok(())
    }

    pub fn get_structured_memory_context(&self) -> String {
        match self.db.get("brain_artifacts", b"StructuredMemory.json") {
            Ok(Some(bytes)) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
                Ok(v) => {
                    let mut out = String::from("[STRUCTURED MEMORY]\n");
                    let facts = v
                        .get("facts")
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let prefs = v
                        .get("preferences")
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    let todos = v
                        .get("todos")
                        .and_then(|x| x.as_array())
                        .cloned()
                        .unwrap_or_default();
                    out.push_str("Facts:\n");
                    for f in facts.into_iter().take(10) {
                        if let Some(s) = f.as_str() {
                            out.push_str(&format!("- {}\n", s));
                        }
                    }
                    out.push_str("Preferences:\n");
                    for p in prefs.into_iter().take(10) {
                        if let Some(s) = p.as_str() {
                            out.push_str(&format!("- {}\n", s));
                        }
                    }
                    out.push_str("Todos:\n");
                    for t in todos.into_iter().take(10) {
                        if let Some(s) = t.as_str() {
                            out.push_str(&format!("- {}\n", s));
                        }
                    }
                    out
                }
                Err(_) => "[STRUCTURED MEMORY]\nUnavailable.".to_string(),
            },
            _ => "[STRUCTURED MEMORY]\nNone.".to_string(),
        }
    }

    pub fn delete_artifact(&self, name: &str) -> Result<(), String> {
        match self.db.delete("brain_artifacts", name.as_bytes()) {
            Ok(_) => {
                let _ = self.db.flush();
                Ok(())
            }
            Err(e) => Err(format!("Failed to delete artifact '{}': {}", name, e)),
        }
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
        llm: Option<LLMClient>,
    ) -> crate::agent::multi_agent::ToolResult {
        let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");
        match action.as_str() {
            "summarize" => {
                let llm = match llm {
                    Some(l) => l,
                    None => {
                        return crate::agent::multi_agent::ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: "LLM Client not available for summarization.".to_string(),
                        }
                    }
                };

                let content = match self.read_artifact(name) {
                    Ok(c) => c,
                    Err(e) => {
                        return crate::agent::multi_agent::ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: e,
                        }
                    }
                };

                // Extract tags from content (e.g., # Tags: tag1, tag2)
                let tags = if let Some(line) = content
                    .lines()
                    .find(|l| l.to_lowercase().starts_with("# tags:"))
                {
                    line.to_string()
                } else {
                    "No specific tags".to_string()
                };

                let prompt = format!(
                    "Summarize the following brain artifact based on its tags [{}]. \
                    Focus on distilling key facts, decisions, and data while stripping conversational filler or redundant details. \
                    The goal is to keep it concise and high-density for future reference by an AI agent. \
                    Return ONLY the summarized content, no introductions or filler. \n\nCONTENT:\n{}",
                    tags, content
                );

                let msgs = vec![crate::agent::llm_client::ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                    images_base64: None,
                }];

                match llm.chat(&msgs, 0.4).await {
                    Ok(res) => {
                        let summary = res.content.trim();
                        // Overwrite original
                        match self.write_artifact(name, summary) {
                            Ok(_) => crate::agent::multi_agent::ToolResult {
                                tool_name: tool,
                                action,
                                ok: true,
                                output: format!("Successfully summarized and replaced '{}'.", name),
                            },
                            Err(e) => crate::agent::multi_agent::ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: e,
                            },
                        }
                    }
                    Err(e) => crate::agent::multi_agent::ToolResult {
                        tool_name: tool,
                        action,
                        ok: false,
                        output: format!("LLM Summarization failed: {}", e),
                    },
                }
            }
            "list" => {
                let result = self.list_artifacts();
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "read" => {
                let result = self.read_artifact(name);
                crate::agent::multi_agent::ToolResult {
                    tool_name: tool,
                    action,
                    ok: result.is_ok(),
                    output: result.unwrap_or_else(|e| e),
                }
            }
            "write_artifact" => {
                let content = args.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let result = self.write_artifact(name, content);
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
                output: "Unsupported action for brain tool".into(),
            },
        }
    }
}
