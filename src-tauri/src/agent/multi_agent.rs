use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::task;

use crate::tools::brain::BrainTool;
use crate::tools::crawler::Crawler;
use crate::tools::search::SearchTool;

use crate::tools::http_tool::HttpTool;
use crate::tools::knowledge::KnowledgeTool;
use crate::tools::moltbook_tool::MoltbookTool;
use crate::tools::pumai_tool::PumaiTool;
use crate::tools::scripting_tool::ScriptingTool;
use crate::tools::telegram_tool::TelegramTool;
use crate::tools::terminal::TerminalTool;
use crate::tools::vault_tool::VaultTool;

pub struct MultiAgent {
    base_dir: PathBuf,
    crawler: Arc<Crawler>,
    search_tool: Arc<SearchTool>,
    brain_tool: Arc<BrainTool>,
    terminal_tool: Arc<TerminalTool>,
    knowledge_tool: Arc<KnowledgeTool>,
    telegram_tool: Arc<TelegramTool>,
    http_tool: Arc<HttpTool>,
    script_tool: Arc<ScriptingTool>,
    moltbook_tool: Arc<MoltbookTool>,
    pumai_tool: Arc<PumaiTool>,
    vault_tool: Arc<VaultTool>,
    external_tools: Arc<RwLock<HashMap<String, ExternalToolDefinition>>>,
}

pub struct ToolResult {
    pub tool_name: String,
    pub action: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ExternalToolMetadata {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub source: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ExternalToolDefinition {
    name: String,
    description: String,
    #[serde(default)]
    actions: Vec<String>,
    #[serde(default = "default_external_tool_kind")]
    kind: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    static_response: String,
    #[serde(default)]
    source: String,
}

fn default_external_tool_kind() -> String {
    "echo".to_string()
}

impl MultiAgent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_dir: PathBuf,
        crawler: Crawler,
        search_tool: SearchTool,
        brain_tool: BrainTool,
        terminal_tool: TerminalTool,
        knowledge_tool: KnowledgeTool,
        telegram_tool: TelegramTool,
        http_tool: HttpTool,
        script_tool: ScriptingTool,
        moltbook_tool: MoltbookTool,
        pumai_tool: PumaiTool,
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
            pumai_tool: Arc::new(pumai_tool),
            vault_tool: Arc::new(vault_tool),
            external_tools: Arc::new(RwLock::new(HashMap::new())),
            base_dir,
        }
    }

    pub fn refresh_external_tools(&self) -> Result<usize, String> {
        let config = crate::config::AppConfig::load(&self.base_dir);
        let mut loaded: HashMap<String, ExternalToolDefinition> = HashMap::new();
        let disabled: HashSet<String> = config.disabled_extensions.iter().cloned().collect();

        for rel_path in &config.extension_search_paths {
            let dir = self.base_dir.join(rel_path);
            if !dir.exists() {
                continue;
            }
            let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(mut def) = serde_json::from_str::<ExternalToolDefinition>(&content) {
                        if def.name.trim().is_empty() || disabled.contains(&def.name) {
                            continue;
                        }
                        if def.description.trim().is_empty() {
                            def.description = "External extension tool".to_string();
                        }
                        if def.actions.is_empty() {
                            def.actions = vec!["invoke".to_string()];
                        }
                        def.source = path.to_string_lossy().to_string();
                        loaded.insert(def.name.clone(), def);
                    }
                }
            }
        }

        let count = loaded.len();
        if let Ok(mut guard) = self.external_tools.write() {
            *guard = loaded;
        }
        Ok(count)
    }

    pub fn list_external_tools(&self) -> Vec<ExternalToolMetadata> {
        let mut out = Vec::new();
        if let Ok(guard) = self.external_tools.read() {
            for tool in guard.values() {
                out.push(ExternalToolMetadata {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    enabled: true,
                    source: tool.source.clone(),
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn lint_prompt_tool_alignment(&self, prompts: &[String]) -> Vec<String> {
        let schemas = self.get_tool_schemas();
        let mut available_tools = std::collections::HashSet::new();
        for s in schemas {
            if let Some(name) = s
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                available_tools.insert(name.to_string());
            }
        }
        let mut warnings = Vec::new();
        let re = regex::Regex::new(r"`([a-zA-Z0-9_\-]+)`").unwrap();
        for (idx, prompt) in prompts.iter().enumerate() {
            for cap in re.captures_iter(prompt) {
                let token = cap[1].to_string();
                if token == "DONE" || token == "JSON" {
                    continue;
                }
                if !available_tools.contains(&token)
                    && ["tool", "tools", "action", "planner", "writer", "critic"]
                        .iter()
                        .all(|kw| token != *kw)
                {
                    warnings.push(format!(
                        "Prompt {} references `{}` but no matching tool schema exists.",
                        idx + 1,
                        token
                    ));
                }
            }
        }
        warnings
    }

    pub fn get_tool_schemas(&self) -> Vec<serde_json::Value> {
        let mut schemas = vec![
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "search",
                    "description": "Google Search for finding external information.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["query"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "crawl4ai",
                    "description": "Crawl and extract full, structured content from a specific URL. Use this to gather detailed data from links found via search to satisfy data compilation requests.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["scrape"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "http",
                    "description": "Perform raw HTTP requests (GET, POST, etc.). Use this for direct API interactions or when standard crawling is blocked.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["request"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "scripting",
                    "description": "Execute internal Rhai scripts for complex data processing, calculations, or multi-step logic.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["run_rhai"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "vault",
                    "description": "Manage secure credentials and API keys in the system vault.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["request", "delete"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "brain",
                    "description": "Manage long-term memory artifacts (list, read, write_artifact).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["list", "read", "write_artifact"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "terminal",
                    "description": "Execute Shell commands.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["execute"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "knowledge",
                    "description": "Manage rules, workflows, schedules.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["read", "write", "list", "delete"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "telegram",
                    "description": "Send notifications to Telegram.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["send_message"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "moltbook",
                    "description": "Interact with Moltbook API (social network). actions include 'status', 'register', 'home', 'search', 'feed', 'create_post', 'create_comment', 'verify'.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["status", "register", "home", "search", "feed", "create_post", "create_comment", "verify"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": "pumai",
                    "description": "Read-only access to PumAI marketplace and knowledge endpoints. actions: health, market_list, market_get, knowledge_fetch.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["health", "market_list", "market_get", "knowledge_fetch"] },
                            "args": { "type": "object" }
                        },
                        "required": ["action", "args"]
                    }
                }
            }),
        ];

        if let Ok(guard) = self.external_tools.read() {
            for ext in guard.values() {
                schemas.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": ext.name,
                        "description": ext.description,
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "action": { "type": "string", "enum": ext.actions },
                                "args": { "type": "object" }
                            },
                            "required": ["action", "args"]
                        }
                    }
                }));
            }
        }

        schemas
    }

    async fn execute_external_tool(
        external_tools: Arc<RwLock<HashMap<String, ExternalToolDefinition>>>,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> Option<ToolResult> {
        let def = {
            if let Ok(guard) = external_tools.read() {
                guard.get(&tool).cloned()
            } else {
                None
            }
        }?;

        if !def.actions.iter().any(|a| a == &action) {
            return Some(ToolResult {
                tool_name: tool,
                action,
                ok: false,
                output: "Unknown action for external extension tool.".to_string(),
            });
        }

        if def.kind == "http" {
            let method = args
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET")
                .to_uppercase();
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let body = args.get("body").cloned().unwrap_or(serde_json::Value::Null);
            let mut url = def.endpoint.trim_end_matches('/').to_string();
            if !path.is_empty() {
                url.push('/');
                url.push_str(path.trim_start_matches('/'));
            }

            let client = rquest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build();
            let client = match client {
                Ok(c) => c,
                Err(e) => {
                    return Some(ToolResult {
                        tool_name: tool,
                        action,
                        ok: false,
                        output: format!("External tool HTTP client error: {}", e),
                    });
                }
            };

            let req = match method.as_str() {
                "POST" => client.post(url).json(&body),
                "PUT" => client.put(url).json(&body),
                "DELETE" => client.delete(url),
                _ => client.get(url),
            };
            return Some(match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    ToolResult {
                        tool_name: tool,
                        action,
                        ok: status.is_success(),
                        output: text,
                    }
                }
                Err(e) => ToolResult {
                    tool_name: tool,
                    action,
                    ok: false,
                    output: format!("External tool call failed: {}", e),
                },
            });
        }

        let mut output = def.static_response;
        if output.trim().is_empty() {
            output = format!(
                "Extension tool '{}' executed action '{}' with args: {}",
                tool, action, args
            );
        }
        Some(ToolResult {
            tool_name: tool,
            action,
            ok: true,
            output,
        })
    }

    pub async fn execute_tools(
        &self,
        requests: Vec<Value>,
        llm: Option<crate::agent::llm_client::LLMClient>,
        registry_prompt: Option<String>,
        trace_id: Option<String>,
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
            if let Some(tid) = trace_id.clone() {
                if let Some(obj) = args.as_object_mut() {
                    if !obj.contains_key("trace_id") {
                        obj.insert("trace_id".to_string(), serde_json::Value::String(tid));
                    }
                }
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
            let pumai_tool = Arc::clone(&self.pumai_tool);
            let vault_tool = Arc::clone(&self.vault_tool);
            let external_tools = Arc::clone(&self.external_tools);

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
                    let terminal_reason = if tool == "terminal" {
                        terminal_tool.analyze_risk(cmd_str)
                    } else {
                        None
                    };
                    let ds = cmd_str.to_lowercase();
                    if terminal_reason.is_some()
                        || ds.contains("drop")
                        || ds.contains("truncate")
                        || tool == "http"
                    {
                        require_approval = true;
                        let reason = terminal_reason
                            .unwrap_or_else(|| "high-risk scripting/http operation".to_string());
                        cmd_preview = crate::agent::approval::build_approval_payload(
                            &tool,
                            &action,
                            &format!("{}\nRiskReason: {}", cmd_str, reason),
                        );
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
                    "brain" => {
                        brain_tool
                            .execute_action(tool, action, args, llm_clone)
                            .await
                    }
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
                    "pumai" => pumai_tool.execute_action(tool, action, args).await,
                    "vault" => vault_tool.execute_action(tool.clone(), action, args),
                    _ => match Self::execute_external_tool(
                        external_tools,
                        tool.clone(),
                        action.clone(),
                        args.clone(),
                    )
                    .await
                    {
                        Some(external_res) => external_res,
                        None => ToolResult {
                            tool_name: tool,
                            action,
                            ok: false,
                            output: "Unknown tool".into(),
                        },
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
