use serde_json::Value;
use std::sync::Arc;
use tokio::task;

use crate::tools::brain::BrainTool;
use crate::tools::crawler::Crawler;
use crate::tools::search::SearchTool;

use crate::tools::http_tool::HttpTool;
use crate::tools::knowledge::KnowledgeTool;
use crate::tools::moltbook_tool::MoltbookTool;
use crate::tools::scripting_tool::ScriptingTool;
use crate::tools::telegram_tool::TelegramTool;
use crate::tools::terminal::TerminalTool;
use crate::tools::vault_tool::VaultTool;

pub struct MultiAgent {
    crawler: Arc<Crawler>,
    search_tool: Arc<SearchTool>,
    brain_tool: Arc<BrainTool>,
    terminal_tool: Arc<TerminalTool>,
    knowledge_tool: Arc<KnowledgeTool>,
    telegram_tool: Arc<TelegramTool>,
    http_tool: Arc<HttpTool>,
    script_tool: Arc<ScriptingTool>,
    moltbook_tool: Arc<MoltbookTool>,
    vault_tool: Arc<VaultTool>,
}

pub struct ToolResult {
    pub tool_name: String,
    pub action: String,
    pub ok: bool,
    pub output: String,
}

impl MultiAgent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        crawler: Crawler,
        search_tool: SearchTool,
        brain_tool: BrainTool,
        terminal_tool: TerminalTool,
        knowledge_tool: KnowledgeTool,
        telegram_tool: TelegramTool,
        http_tool: HttpTool,
        script_tool: ScriptingTool,
        moltbook_tool: MoltbookTool,
        vault_tool: VaultTool,
    ) -> Self {
        MultiAgent {
            crawler: Arc::new(crawler),
            search_tool: Arc::new(search_tool),
            brain_tool: Arc::new(brain_tool),
            terminal_tool: Arc::new(terminal_tool),
            knowledge_tool: Arc::new(knowledge_tool),
            telegram_tool: Arc::new(telegram_tool),
            http_tool: Arc::new(http_tool),
            script_tool: Arc::new(script_tool),
            moltbook_tool: Arc::new(moltbook_tool),
            vault_tool: Arc::new(vault_tool),
        }
    }

    pub async fn execute_tools(
        &self,
        requests: Vec<Value>,
        llm: Option<crate::agent::llm_client::LLMClient>,
        registry_prompt: Option<String>,
    ) -> Vec<ToolResult> {
        let mut handles = vec![];
        let vault_re = regex::Regex::new(r"\{\{vault:([a-zA-Z0-9_\-]+)\}\}").unwrap();

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

            // Vault Macro Interpolation
            let mut args = req.get("args").cloned().unwrap_or_else(|| req.clone());
            let args_str = args.to_string();

            let mut used_secrets = std::collections::HashSet::new();
            let mut final_args_str = args_str.clone();

            for cap in vault_re.captures_iter(&args_str) {
                if let Some(key) = cap.get(1) {
                    if let Ok(secret) =
                        crate::tools::vault_tool::VaultTool::get_secret(key.as_str())
                    {
                        final_args_str = final_args_str.replace(&cap[0], &secret);
                        used_secrets.insert(secret);
                    }
                }
            }
            if let Ok(parsed_args) = serde_json::from_str(&final_args_str) {
                args = parsed_args;
            }

            let crawler = Arc::clone(&self.crawler);
            let search_tool = Arc::clone(&self.search_tool);
            let brain_tool = Arc::clone(&self.brain_tool);
            let terminal_tool = Arc::clone(&self.terminal_tool);
            let knowledge_tool = Arc::clone(&self.knowledge_tool);
            let telegram_tool = Arc::clone(&self.telegram_tool);
            let http_tool = Arc::clone(&self.http_tool);
            let script_tool = Arc::clone(&self.script_tool);
            let moltbook_tool = Arc::clone(&self.moltbook_tool);
            let vault_tool = Arc::clone(&self.vault_tool);

            let llm_clone = llm.clone();
            let registry_prompt_clone = registry_prompt.clone();

            let handle = task::spawn(async move {
                let mut require_approval = false;
                let mut cmd_preview = String::new();

                if tool == "terminal" || tool == "scripting" || tool == "http" {
                    let cmd_str = args
                        .get("command")
                        .or_else(|| args.get("code"))
                        .or_else(|| args.get("url"))
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("");
                    let ds = cmd_str.to_lowercase();
                    if ds.contains("rm ")
                        || ds.contains("del ")
                        || ds.contains("remove-item")
                        || ds.contains("format ")
                        || ds.contains("curl")
                        || ds.contains("wget")
                        || ds.contains("sudo")
                        || ds.contains("drop")
                        || tool == "http"
                    {
                        require_approval = true;
                        cmd_preview = format!("Tool: {} \nPayload: {}", tool, cmd_str);
                    }
                }

                if require_approval {
                    let approved =
                        crate::agent::approval::request_approval(&telegram_tool, &cmd_preview)
                            .await;
                    if !approved {
                        return ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: "[SECURITY LOCK] User rejected execution of this command."
                                .to_string(),
                        };
                    }
                }

                let mut res = match tool.as_str() {
                    "crawl4ai" => crawler.execute_action(tool, action, args).await,
                    "search" => search_tool.execute_action(tool, action, args).await,
                    "brain" => brain_tool.execute_action(tool, action, args),
                    "knowledge" => {
                        knowledge_tool
                            .execute_action(tool, action, args, llm_clone, registry_prompt_clone)
                            .await
                    }
                    "terminal" => terminal_tool.execute_action(tool, action, args),
                    "telegram" => telegram_tool.execute_action(tool, action, args).await,
                    "http" => http_tool.execute_action(tool, action, args).await,
                    "scripting" => script_tool.execute_action(tool, action, args).await,
                    "moltbook" => moltbook_tool.execute_action(tool, action, args).await,
                    "vault" => vault_tool.execute_action(tool.clone(), action, args),
                    _ => ToolResult {
                        tool_name: tool,
                        action,
                        ok: false,
                        output: "Unknown tool".into(),
                    },
                };

                for secret in used_secrets {
                    if !secret.trim().is_empty() {
                        res.output = res.output.replace(&secret, "[REDACTED_SECRET]");
                    }
                }

                res
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
