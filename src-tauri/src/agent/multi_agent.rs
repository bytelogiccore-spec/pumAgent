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

            // AI Hallucination Fallback
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
                    "crawl4ai" => crawler.execute_action(tool, action, args).await,
                    "search" => search_tool.execute_action(tool, action, args).await,
                    "brain" => brain_tool.execute_action(tool, action, args),
                    "knowledge" => knowledge_tool.execute_action(tool, action, args, llm_clone, registry_prompt_clone).await,
                    "terminal" => terminal_tool.execute_action(tool, action, args),
                    "telegram" => telegram_tool.execute_action(tool, action, args).await,
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
