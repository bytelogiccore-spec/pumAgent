use serde_json::Value;
use std::sync::Arc;
use tokio::task;

use crate::tools::brain::BrainTool;
use crate::tools::crawler::Crawler;
use crate::tools::search::SearchTool;

use crate::tools::knowledge::KnowledgeTool;
use crate::tools::telegram_tool::TelegramTool;
use crate::tools::terminal::TerminalTool;

pub struct MultiAgent {
    crawler: Arc<Crawler>,
    search_tool: Arc<SearchTool>,
    brain_tool: Arc<BrainTool>,
    terminal_tool: Arc<TerminalTool>,
    knowledge_tool: Arc<KnowledgeTool>,
    telegram_tool: Arc<TelegramTool>,
}

pub struct ToolResult {
    pub tool_name: String,
    pub action: String,
    pub ok: bool,
    pub output: String,
}

impl MultiAgent {
    pub fn new(
        crawler: Crawler,
        search_tool: SearchTool,
        brain_tool: BrainTool,
        terminal_tool: TerminalTool,
        knowledge_tool: KnowledgeTool,
        telegram_tool: TelegramTool,
    ) -> Self {
        MultiAgent {
            crawler: Arc::new(crawler),
            search_tool: Arc::new(search_tool),
            brain_tool: Arc::new(brain_tool),
            terminal_tool: Arc::new(terminal_tool),
            knowledge_tool: Arc::new(knowledge_tool),
            telegram_tool: Arc::new(telegram_tool),
        }
    }

    pub async fn execute_tools(
        &self,
        requests: Vec<Value>,
        llm: Option<crate::agent::llm_client::LLMClient>,
        registry_prompt: Option<String>,
    ) -> Vec<ToolResult> {
        let mut handles = vec![];

        for req in requests {
            let mut tool = req
                .get("tool")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .trim_matches(|c| c == ':' || c == '.' || c == ' ' || c == '"' || c == '\'')
                .to_string();
            let mut action = req
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("unknown")
                .trim_matches(|c| c == ':' || c == '.' || c == ' ' || c == '"' || c == '\'')
                .to_string();

            // AI Hallucination Fallback: AI sometimes concatenates tool name and action into `tool`
            if let Some(idx) = tool.find(':').or_else(|| tool.find('.')) {
                if action == "unknown" {
                    action = tool[idx + 1..]
                        .trim_matches(|c| c == ':' || c == '.' || c == ' ' || c == '"' || c == '\'')
                        .to_string();
                }
                tool = tool[..idx]
                    .trim_matches(|c| c == ':' || c == '.' || c == ' ' || c == '"' || c == '\'')
                    .to_string();
            }

            let args = req.get("args").cloned().unwrap_or_else(|| req.clone());

            let crawler = Arc::clone(&self.crawler);
            let search_tool = Arc::clone(&self.search_tool);
            let brain_tool = Arc::clone(&self.brain_tool);
            let terminal_tool = Arc::clone(&self.terminal_tool);
            let knowledge_tool = Arc::clone(&self.knowledge_tool);
            let telegram_tool = Arc::clone(&self.telegram_tool);

            let llm_clone = llm.clone();
            let registry_prompt_clone = registry_prompt.clone();
            let handle = task::spawn(async move {
                match tool.as_str() {
                    "crawl4ai" => {
                        if action == "scrape" {
                            let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            match crawler.scrape(url).await {
                                Ok(content) => ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: true,
                                    output: content,
                                },
                                Err(e) => ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: false,
                                    output: format!("Scrape error: {}", e),
                                },
                            }
                        } else {
                            ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: "Unsupported action".into(),
                            }
                        }
                    }
                    "search" => {
                        if action == "query" {
                            let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("");
                            let time_range = args.get("time_range").and_then(|t| t.as_str());
                            match search_tool.search(query, time_range, 5).await {
                                Ok(items) => {
                                    let content = items
                                        .into_iter()
                                        .map(|item| {
                                            format!(
                                                "Title: {}\nLink: {}\nSnippet: {}\n---",
                                                item.title, item.link, item.snippet
                                            )
                                        })
                                        .collect::<Vec<String>>()
                                        .join("\n");

                                    ToolResult {
                                        tool_name: tool,
                                        action,
                                        ok: true,
                                        output: content,
                                    }
                                }
                                Err(e) => ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: false,
                                    output: format!("Search error: {}", e),
                                },
                            }
                        } else {
                            ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: "Unsupported action".into(),
                            }
                        }
                    }
                    "brain" => {
                        let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");

                        match action.as_str() {
                            "list" => {
                                let result = brain_tool.list_artifacts();
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            "read" => {
                                let result = brain_tool.read_artifact(name);
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            "write_artifact" => {
                                let content =
                                    args.get("content").and_then(|c| c.as_str()).unwrap_or("");
                                let result = brain_tool.write_artifact(name, content);
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            _ => ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: "Unsupported action for brain tool".into(),
                            },
                        }
                    }
                    "knowledge" => {
                        let domain = args.get("domain").and_then(|d| d.as_str()).unwrap_or("");
                        let name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");

                        match action.as_str() {
                            "list" => {
                                let result = knowledge_tool.list(domain);
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            "read" => {
                                let result = knowledge_tool.read(domain, name);
                                ToolResult {
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

                                // Intercept for JSON schemas using Registry Agent
                                if (domain == "schedules" || domain == "skills" || domain == "rules")
                                    && serde_json::from_str::<Value>(&content).is_err() {
                                        if let (Some(client), Some(prompt)) =
                                            (&llm_clone, &registry_prompt_clone)
                                        {
                                            let now_str = chrono::Local::now()
                                                .format("%Y-%m-%d %A %H:%M:%S")
                                                .to_string();
                                            let prompt_with_context = format!("{}\n\n[SYSTEM TIME ANCHOR]\nCurrent System Time: {}\n\nUser Request to convert to JSON:\n{}", prompt, now_str, content);
                                            let msgs =
                                                vec![crate::agent::llm_client::ChatMessage {
                                                    role: "user".to_string(),
                                                    content: prompt_with_context,
                                                    images_base64: None,
                                                }];
                                            if let Ok(res) = client.chat(&msgs, 0.1).await {
                                                let ai_text = crate::agent::orchestrator::Orchestrator::sanitize_output(&res.content);
                                                let json_blocks =
                                                    crate::agent::parser::extract_json_blocks(
                                                        &ai_text,
                                                    );
                                                if let Some(first_block) = json_blocks.first() {
                                                    content =
                                                        serde_json::to_string_pretty(first_block)
                                                            .unwrap_or(content);
                                                } else {
                                                    content = ai_text;
                                                }
                                            }
                                        }
                                    }

                                let result = knowledge_tool.write(domain, name, &content);
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            "delete" => {
                                let result = knowledge_tool.delete(domain, name);
                                ToolResult {
                                    tool_name: tool,
                                    action,
                                    ok: result.is_ok(),
                                    output: result.unwrap_or_else(|e| e),
                                }
                            }
                            _ => ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: "Unsupported action for knowledge".into(),
                            },
                        }
                    }
                    "terminal" => {
                        if action == "execute" {
                            let cmd_string =
                                args.get("command").and_then(|c| c.as_str()).unwrap_or("");
                            let result = terminal_tool.execute(cmd_string);
                            ToolResult {
                                tool_name: tool,
                                action,
                                ok: result.is_ok(),
                                output: result.unwrap_or_else(|e| e),
                            }
                        } else {
                            ToolResult {
                                tool_name: tool,
                                action,
                                ok: false,
                                output: "Unsupported action for terminal".into(),
                            }
                        }
                    }
                    "telegram" => {
                        if action == "send_message" {
                            let message =
                                args.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            let result = telegram_tool.send_message(message).await;
                            ToolResult {
                                tool_name: tool,
                                action,
                                ok: result.contains("successfully"),
                                output: result,
                            }
                        } else {
                            ToolResult {
                                tool_name: tool,
                                action: action.clone(),
                                ok: false,
                                output: format!("Unknown telegram action: {}", action),
                            }
                        }
                    }
                    _ => ToolResult {
                        tool_name: tool,
                        action,
                        ok: false,
                        output: "Unknown tool".into(),
                    },
                }
            });
            handles.push(handle);
        }

        let mut results = vec![];
        for handle in handles {
            if let Ok(res) = handle.await {
                results.push(res);
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::telegram_tool::TelegramTool;
    use serde_json::json;

    #[tokio::test]
    async fn test_execute_crawler_tool() {
        // Arrange
        let crawler = Crawler::new();
        // Provide dummy API keys since we won't invoke search
        let search_tool = SearchTool::new(
            "dummy_key".to_string(),
            "dummy_cx".to_string(),
            std::path::PathBuf::from("."),
        );
        let brain_tool = BrainTool::new(std::path::PathBuf::from("."));
        let terminal_tool = TerminalTool::new(std::path::PathBuf::from("."));
        let knowledge_tool = KnowledgeTool::new(std::path::PathBuf::from("."));
        let telegram_tool = TelegramTool::new(std::path::PathBuf::from("."));

        let agent = MultiAgent::new(
            crawler,
            search_tool,
            brain_tool,
            terminal_tool,
            knowledge_tool,
            telegram_tool,
        );
        let request = json!({
            "tool": "crawl4ai",
            "action": "scrape",
            "args": {
                "url": "https://example.com"
            }
        });

        // Act
        let results = agent.execute_tools(vec![request]).await;

        // Assert
        assert_eq!(results.len(), 1);
        let res = &results[0];
        assert_eq!(res.tool_name, "crawl4ai");
        assert_eq!(res.action, "scrape");
        assert!(
            res.ok,
            "Scraping should succeed, but got error: {}",
            res.output
        );
        assert!(
            res.output.contains("Example Domain"),
            "Output should contain the parsed text from example.com"
        );
    }
}
