use app_lib::agent::llm_client::ChatMessage;
use app_lib::agent::multi_agent::MultiAgent;
use app_lib::agent::orchestrator::{Orchestrator, OrchestratorRouting};
use app_lib::config::AppConfig;
use app_lib::tools::brain::BrainTool;
use app_lib::tools::crawler::Crawler;
use app_lib::tools::knowledge::KnowledgeTool;
use app_lib::tools::search::SearchTool;
use app_lib::tools::telegram_tool::TelegramTool;
use app_lib::tools::terminal::TerminalTool;
use app_lib::tools::vault_tool::VaultTool;
use dbx_core::Database;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<String>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct RpcResponse {
    id: Option<String>,
    ok: bool,
    result: serde_json::Value,
}

#[tokio::main]
async fn main() {
    let base_dir = resolve_base_dir();
    if !base_dir.exists() {
        let _ = std::fs::create_dir_all(&base_dir);
    }

    let db_path = base_dir.join("pumagent_store.dbx");
    let db = Database::open(&db_path).expect("Failed to initialize DBX engine.");
    let multi_agent = Arc::new(MultiAgent::new(
        base_dir.clone(),
        Crawler::new(),
        SearchTool::new(db.clone(), base_dir.clone()),
        BrainTool::new(db.clone()),
        TerminalTool::new(base_dir.clone(), Some(db.clone())),
        KnowledgeTool::new(db.clone()),
        TelegramTool::new(base_dir.clone()),
        app_lib::tools::http_tool::HttpTool::new(),
        app_lib::tools::scripting_tool::ScriptingTool::new(),
        app_lib::tools::moltbook_tool::MoltbookTool::new(base_dir.clone()),
        app_lib::tools::pumai_tool::PumaiTool::new(base_dir.clone()),
        VaultTool::new(base_dir.clone()),
    ));
    let _ = multi_agent.refresh_external_tools();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_request(
            line,
            &base_dir,
            db.clone(),
            multi_agent.clone(),
            cancel_flag.clone(),
        )
        .await;
        let _ = writeln!(
            stdout,
            "{}",
            serde_json::to_string(&response).unwrap_or_else(|_| "{\"ok\":false}".to_string())
        );
        let _ = stdout.flush();
    }
}

async fn handle_request(
    line: String,
    base_dir: &std::path::Path,
    db: Arc<Database>,
    multi_agent: Arc<MultiAgent>,
    cancel_flag: Arc<AtomicBool>,
) -> RpcResponse {
    let req = match serde_json::from_str::<RpcRequest>(&line) {
        Ok(v) => v,
        Err(e) => {
            return RpcResponse {
                id: None,
                ok: false,
                result: serde_json::json!({ "error": format!("Invalid JSON request: {}", e) }),
            }
        }
    };

    match req.method.as_str() {
        "list_tools" => RpcResponse {
            id: req.id,
            ok: true,
            result: serde_json::json!({ "tools": multi_agent.get_tool_schemas() }),
        },
        "run_once" => {
            let config = AppConfig::load(base_dir);
            let message = req
                .params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let language = req
                .params
                .get("language")
                .and_then(|v| v.as_str())
                .unwrap_or(&config.language)
                .to_string();
            let session_id = req
                .params
                .get("session_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let routing = OrchestratorRouting {
                endpoints: config.endpoints.clone(),
                planner_id: config.planner_endpoint_id.clone(),
                critic_id: config.critic_endpoint_id.clone(),
                writer_id: config.writer_endpoint_id.clone(),
                worker_id: config.worker_endpoint_id.clone(),
                reflector_id: config.reflector_endpoint_id.clone(),
                registry_id: config.registry_endpoint_id.clone(),
            };
            let orchestrator =
                Orchestrator::new(routing, multi_agent, base_dir.to_path_buf(), db.clone());
            let (tx, _rx) = tokio::sync::mpsc::channel::<String>(32);
            match orchestrator
                .run_loop(
                    session_id,
                    "rpc",
                    &config.system_prompt,
                    config.planner_prompt.as_deref(),
                    config.critic_prompt.as_deref(),
                    config.writer_prompt.as_deref(),
                    vec![ChatMessage {
                        role: "user".to_string(),
                        content: message,
                        images_base64: None,
                    }],
                    &language,
                    config.max_loops,
                    config.use_multi_agent_workflow,
                    config.use_think_mode,
                    config.registry_prompt.as_deref(),
                    tx,
                    cancel_flag,
                )
                .await
            {
                Ok((final_output, _)) => RpcResponse {
                    id: req.id,
                    ok: true,
                    result: serde_json::json!({ "final_output": final_output }),
                },
                Err(e) => RpcResponse {
                    id: req.id,
                    ok: false,
                    result: serde_json::json!({ "error": e }),
                },
            }
        }
        _ => RpcResponse {
            id: req.id,
            ok: false,
            result: serde_json::json!({ "error": "Unknown method" }),
        },
    }
}

fn resolve_base_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("PUMAGENT_DATA_DIR") {
        return std::path::PathBuf::from(path);
    }
    std::env::current_dir()
        .unwrap_or_default()
        .join("..")
        .join("PumAgentData")
}
