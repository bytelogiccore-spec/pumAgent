pub mod agent;
pub mod tools;
pub mod commands;

use agent::llm_client::{ChatMessage, LLMClient};
use agent::multi_agent::MultiAgent;
use agent::orchestrator::Orchestrator;
use dbx_core::Database;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tools::brain::BrainTool;
use tools::crawler::Crawler;
use tools::knowledge::KnowledgeTool;
use tools::search::SearchTool;
use tools::telegram_tool::TelegramTool;
use tools::terminal::TerminalTool;
use commands::fs::*;

pub struct AgentState {
    pub multi_agent: Arc<MultiAgent>,
    pub base_dir: std::path::PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub db: Arc<Database>,
}

#[derive(Deserialize)]
struct RunPayload {
    api_url: String,
    session_id: Option<String>,
    llm_api_key: Option<String>,
    model: String,
    cloud_api_url: String,
    cloud_model: String,
    cloud_llm_api_key: Option<String>,
    cloud_routing_planner: bool,
    cloud_routing_critic: bool,
    cloud_routing_writer: bool,
    cloud_routing_worker: bool,
    system_prompt: String,
    planner_prompt: Option<String>,
    critic_prompt: Option<String>,
    writer_prompt: Option<String>,
    reflector_prompt: Option<String>,
    max_loops: u32,
    use_multi_agent_workflow: bool,
    use_think_mode: bool,
    language: String,
    worker_prompt: Option<String>,
    registry_prompt: Option<String>,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct OrchestratorResponse {
    final_output: String,
}

#[tauri::command]
async fn execute_agent_tools(
    payload: RunPayload,
    state: State<'_, AgentState>,
    app: AppHandle,
) -> Result<OrchestratorResponse, String> {
    // Create an isolated Tx/Rx channel for emitting logs continuously
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    // Spawn a quick local task to forward rx to Tauri frontend via `app.emit_all("tool_log", msg)`
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let _ = app.emit("tool_log", msg);
        }
    });

    // Reset cancel flag before starting
    state.cancel_flag.store(false, Ordering::Relaxed);

    let _ = tx
        .send(format!(
            "[시스템] {} 모델로 접속을 시도합니다...",
            payload.model
        ))
        .await;

    // Orchestrator needs LLM + MultiAgent + base_dir
    let llm_key = payload.llm_api_key.clone().unwrap_or_default();
    let local_llm = LLMClient::new(
        payload.api_url.clone(),
        payload.model.clone(),
        llm_key.clone(),
    );
    let cloud_key = payload.cloud_llm_api_key.clone().unwrap_or_default();
    let cloud_llm = if payload.cloud_api_url.is_empty() {
        local_llm.clone()
    } else {
        LLMClient::new(
            payload.cloud_api_url.clone(),
            payload.cloud_model.clone(),
            cloud_key,
        )
    };

    let routing_flags = agent::orchestrator::CloudRoutingFlags {
        planner: payload.cloud_routing_planner,
        critic: payload.cloud_routing_critic,
        writer: payload.cloud_routing_writer,
        worker: payload.cloud_routing_worker,
    };

    let orchestrator = Orchestrator::new(
        local_llm,
        cloud_llm,
        routing_flags,
        Arc::clone(&state.multi_agent),
        state.base_dir.clone(),
        state.db.clone(),
    );

    let mut actual_system_prompt = payload.worker_prompt.unwrap_or(payload.system_prompt);
    actual_system_prompt.push_str(&format!(
        "\n\n[GLOBAL RULE]\nCRITICAL: Any content intended for the user (such as final output or Telegram notifications) MUST be composed in {}.",
        payload.language
    ));
    if !payload.use_think_mode {
        actual_system_prompt.push_str("\n\n!! CRITICAL DIRECTIVE !!\nDO NOT OUTPUT ANY REASONING, THINKING, OR THOUGHT BLOCKS. DO NOT USE <think> OR SIMILAR TAGS. PROVIDE YOUR FINAL RESPONSE DIRECTLY AND IMMEDIATELY.");
    }

    let (final_answer, history) = orchestrator
        .run_loop(
            payload.session_id.clone(),
            &actual_system_prompt,
            payload.planner_prompt.as_deref(),
            payload.critic_prompt.as_deref(),
            payload.writer_prompt.as_deref(),
            payload.messages,
            &payload.language,
            payload.max_loops,
            payload.use_multi_agent_workflow,
            payload.registry_prompt.as_deref(),
            tx.clone(),
            Arc::clone(&state.cancel_flag),
        )
        .await?;

    let _ = tx
        .send("[시스템] 응답 처리가 완전히 종료되었습니다.".to_string())
        .await;

    // Spawn Reflector Agent asynchronously if workflow is enabled
    if payload.use_multi_agent_workflow {
        if let Some(reflector_prompt) = payload.reflector_prompt {
            let log_tx = tx.clone();
            // Clone orchestrator state so it can be moved into the async block
            let local_llm = LLMClient::new(payload.api_url, payload.model, llm_key.clone());
            let cloud_key = payload.cloud_llm_api_key.clone().unwrap_or_default();
            let cloud_llm = if payload.cloud_api_url.is_empty() {
                local_llm.clone()
            } else {
                LLMClient::new(payload.cloud_api_url, payload.cloud_model, cloud_key)
            };
            let routing_flags = agent::orchestrator::CloudRoutingFlags {
                planner: payload.cloud_routing_planner,
                critic: payload.cloud_routing_critic,
                writer: payload.cloud_routing_writer,
                worker: payload.cloud_routing_worker,
            };
            let bg_orchestrator = Orchestrator::new(
                local_llm,
                cloud_llm,
                routing_flags,
                Arc::clone(&state.multi_agent),
                state.base_dir.clone(),
                state.db.clone(),
            );
            tokio::spawn(async move {
                bg_orchestrator
                    .run_reflector_pipeline(&reflector_prompt, history, log_tx)
                    .await;
            });
        }
    }

    Ok(OrchestratorResponse {
        final_output: final_answer,
    })
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct BackgroundPayload {
    api_url: String,
    session_id: Option<String>,
    llm_api_key: Option<String>,
    model: String,
    cloud_api_url: String,
    cloud_model: String,
    cloud_llm_api_key: Option<String>,
    cloud_routing_planner: bool,
    cloud_routing_critic: bool,
    cloud_routing_writer: bool,
    cloud_routing_worker: bool,
    system_prompt: String,
    planner_prompt: Option<String>,
    critic_prompt: Option<String>,
    writer_prompt: Option<String>,
    reflector_prompt: Option<String>,
    max_loops: u32,
    use_multi_agent_workflow: bool,
    language: String,
    worker_prompt: Option<String>,
    registry_prompt: Option<String>,
}

#[tauri::command]
async fn execute_background_scheduler(
    payload: BackgroundPayload,
    state: State<'_, AgentState>,
    app: AppHandle,
) -> Result<String, String> {
    let scheduler = crate::agent::scheduler::Scheduler::new(state.db.clone());
    let (pending_tasks, _, _) = scheduler.evaluate_schedules();

    if pending_tasks.is_empty() {
        return Ok("No tasks".to_string());
    }

    let llm_key = payload.llm_api_key.clone().unwrap_or_default();
    let local_llm = LLMClient::new(payload.api_url, payload.model, llm_key.clone());
    let cloud_key = payload.cloud_llm_api_key.clone().unwrap_or_default();
    let cloud_llm = if payload.cloud_api_url.is_empty() {
        local_llm.clone()
    } else {
        LLMClient::new(payload.cloud_api_url, payload.cloud_model, cloud_key)
    };

    let multi_agent = Arc::clone(&state.multi_agent);
    let base_dir = state.base_dir.clone();
    let db = state.db.clone();
    let cancel_flag = state.cancel_flag.clone();

    let sys = payload.system_prompt;
    let planner = payload.planner_prompt;
    let critic = payload.critic_prompt;
    let writer = payload.writer_prompt;
    let max_loops = payload.max_loops;
    let use_multi = payload.use_multi_agent_workflow;
    let lang = payload.language;
    let registry = payload.registry_prompt;
    let worker = payload.worker_prompt;
    let session_id = payload.session_id;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let _ = app_clone.emit("tool_log", format!("[백그라운드] {}", msg));
        }
    });

    let routing_flags = agent::orchestrator::CloudRoutingFlags {
        planner: payload.cloud_routing_planner,
        critic: payload.cloud_routing_critic,
        writer: payload.cloud_routing_writer,
        worker: payload.cloud_routing_worker,
    };

    tokio::spawn(async move {
        let orchestrator = Orchestrator::new(local_llm, cloud_llm, routing_flags, multi_agent, base_dir, db);
        let _ = orchestrator
            .run_loop(
                session_id,
                &worker.unwrap_or(sys),
                planner.as_deref(),
                critic.as_deref(),
                writer.as_deref(),
                vec![], // Empty history naturally triggers step 1 pending injection
                &lang,
                max_loops,
                use_multi,
                registry.as_deref(),
                tx,
                cancel_flag,
            )
            .await;
    });

    Ok("Started".to_string())
}

#[derive(Deserialize)]
struct CompressPayload {
    api_url: String,
    llm_api_key: Option<String>,
    model: String,
    messages: Vec<ChatMessage>,
}

#[tauri::command]
async fn compress_memory(payload: CompressPayload) -> Result<String, String> {
    let llm_key = payload.llm_api_key.clone().unwrap_or_default();
    let llm = LLMClient::new(payload.api_url, payload.model, llm_key);
    let system_msg = ChatMessage {
        role: "system".to_string(),
        content: "제공된 과거의 대화 및 사고 내역을 짧고 명료하게 요약해라. 핵심적인 사실, 유저의 요청 사항, 수집된 주요 정보 및 앞으로 진행해야 할 남은 계획 위주로 압축할 것. 불필요한 인사말이나 서론 없이 압축된 컨텍스트만 한국어로 반환할 것.".to_string(),
        images_base64: None,
    };

    let mut combined_text = String::new();
    for msg in payload.messages {
        combined_text.push_str(&format!(
            "[{}]\n{}\n\n",
            msg.role.to_uppercase(),
            msg.content
        ));
    }

    let user_msg = ChatMessage {
        role: "user".to_string(),
        content: format!("다음 과거 대화 내역을 요약해라:\n\n{}", combined_text),
        images_base64: None,
    };

    let history = vec![system_msg, user_msg];

    match llm.chat(&history, 0.4).await {
        Ok(res) => Ok(res.content.trim().to_string()),
        Err(e) => Err(format!("메모리 압축 오류: {}", e)),
    }
}

#[tauri::command]
fn stop_agent(state: State<'_, AgentState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::Relaxed);
    Ok(())
}

fn default_search_provider() -> String {
    "duckduckgo".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_is_first_run() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_is_first_run")]
    pub is_first_run: bool,
    pub api_url: String,
    pub model: String,
    #[serde(default)]
    pub llm_api_key: String,
    #[serde(default)]
    pub cloud_api_url: String,
    #[serde(default)]
    pub cloud_model: String,
    #[serde(default)]
    pub cloud_llm_api_key: String,
    #[serde(default)]
    pub cloud_routing_planner: bool,
    #[serde(default = "default_true")]
    pub cloud_routing_critic: bool,
    #[serde(default = "default_true")]
    pub cloud_routing_writer: bool,
    #[serde(default)]
    pub cloud_routing_worker: bool,
    pub max_loops: u32,
    pub system_prompt: String,
    #[serde(default = "default_search_provider")]
    pub search_provider: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub tavily_api_key: String,
    #[serde(default)]
    pub google_api_key: String,
    #[serde(default)]
    pub google_cx: String,
    #[serde(default)]
    pub use_multi_agent_workflow: bool,
    #[serde(default)]
    pub use_think_mode: bool,

    #[serde(default)]
    pub planner_prompt: Option<String>,
    #[serde(default)]
    pub critic_prompt: Option<String>,
    #[serde(default)]
    pub writer_prompt: Option<String>,
    #[serde(default)]
    pub reflector_prompt: Option<String>,
    #[serde(default)]
    pub heartbeat_prompt: Option<String>,
    #[serde(default)]
    pub worker_prompt: Option<String>,
    #[serde(default)]
    pub registry_prompt: Option<String>,

    #[serde(default)]
    pub heartbeat_enabled: bool,
    #[serde(default)]
    pub heartbeat_interval: u64,

    #[serde(default)]
    pub telegram_enabled: bool,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_chat_id: String,
}

impl AppConfig {
    pub fn save(&self, base_dir: &std::path::Path) -> Result<(), String> {
        let config_path = base_dir.join("agent_config.json");
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&config_path, content).map_err(|e| e.to_string())
    }

    pub fn load(base_dir: &std::path::Path) -> Self {
        let config_path = base_dir.join("agent_config.json");
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
        AppConfig {
            is_first_run: true,
            api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
            model: "gemma-4".to_string(),
            llm_api_key: "".to_string(),
            cloud_api_url: "".to_string(),
            cloud_model: "anthropic/claude-3-opus-20240229".to_string(),
            cloud_llm_api_key: "".to_string(),
            cloud_routing_planner: false,
            cloud_routing_critic: true,
            cloud_routing_writer: true,
            cloud_routing_worker: false,
            max_loops: 20,
            system_prompt: "You are a highly capable autonomous AI assistant. Think clearly, step-by-step, and strictly follow the formatting rules to achieve the user's goal.".to_string(),
            search_provider: "duckduckgo".to_string(),
            language: "en".to_string(),
            tavily_api_key: "".to_string(),
            google_api_key: "".to_string(),
            google_cx: "".to_string(),
            use_multi_agent_workflow: false,
            use_think_mode: false,
            planner_prompt: None,
            critic_prompt: None,
            writer_prompt: None,
            reflector_prompt: None,
            heartbeat_prompt: None,
            worker_prompt: None,
            registry_prompt: None,
            heartbeat_enabled: false,
            heartbeat_interval: 3600,
            telegram_enabled: false,
            telegram_bot_token: "".to_string(),
            telegram_chat_id: "".to_string(),
        }
    }
}

#[tauri::command]
fn load_config(state: State<'_, AgentState>) -> Result<AppConfig, String> {
    Ok(AppConfig::load(&state.base_dir))
}

#[tauri::command]
fn save_config(config: AppConfig, state: State<'_, AgentState>) -> Result<(), String> {
    let config_path = state.base_dir.join("agent_config.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}



#[tauri::command]
fn flush_db(state: State<'_, AgentState>) -> Result<(), String> {
    state.db.flush().map_err(|e| e.to_string())
}

#[tauri::command]
async fn translate_i18n(
    target_lang: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let config = AppConfig::load(&state.base_dir);
    let llm =
        crate::agent::llm_client::LLMClient::new(config.api_url, config.model, config.llm_api_key);

    let en_content = match state.db.get("knowledge_base", b"locales:en.json") {
        Ok(Some(bytes)) => String::from_utf8(bytes).unwrap_or_default(),
        _ => return Err("en.json base locale not found".to_string()),
    };

    let system_prompt = "You are a professional JSON translator. Translate the given localization JSON map values into the requested language. RULES:\n1. Keep all JSON keys exactly the same.\n2. Translate ONLY the values into the target language.\n3. Return strictly valid JSON with no markdown formatting, no codeblocks and no explanation.".to_string();

    let messages = vec![
        crate::agent::llm_client::ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
            images_base64: None,
        },
        crate::agent::llm_client::ChatMessage {
            role: "user".to_string(),
            content: format!("Target Language: {}\n\nJSON:\n{}", target_lang, en_content),
            images_base64: None,
        },
    ];

    let mut result_json = match llm.chat(&messages, 0.1).await {
        Ok(res) => res.content,
        Err(e) => return Err(format!("LLM Error: {}", e)),
    };

    result_json = result_json.trim().to_string();
    if result_json.starts_with("```json") {
        result_json = result_json.trim_start_matches("```json").to_string();
    } else if result_json.starts_with("```") {
        result_json = result_json.trim_start_matches("```").to_string();
    }
    if result_json.ends_with("```") {
        result_json = result_json.trim_end_matches("```").to_string();
    }
    result_json = result_json.trim().to_string();

    if let Err(e) = serde_json::from_str::<serde_json::Value>(&result_json) {
        return Err(format!(
            "Failed to parse LLM output as JSON: {}. Output: {}",
            e, result_json
        ));
    }

    let key = format!("locales:{}.json", target_lang);
    state
        .db
        .insert("knowledge_base", key.as_bytes(), result_json.as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = state.db.flush();

    Ok(result_json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = window.try_state::<AgentState>() {
                    let _ = state.db.flush();
                    println!("[PumAgent] Window closing, performed DB flush for safety.");
                }
            }
        })
        .setup(|app| {
            #[cfg(debug_assertions)]
            let base_dir = std::env::current_dir()
                .unwrap_or_default()
                .join("..")
                .join("..")
                .join("PumAgentData");

            #[cfg(not(debug_assertions))]
            let base_dir = std::env::current_dir()
                .unwrap_or_default()
                .join("..")
                .join("PumAgentData");

            if !base_dir.exists() {
                let _ = fs::create_dir_all(&base_dir);
            }

            // Initialize DBX Core
            let db_path = base_dir.join("pumagent_store.dbx");
            let db = Database::open(&db_path).expect("Failed to initialize DBX engine.");

            // --- One Time Migration Script ---
            // Let's migrate legacy `knowledge/schedules` if they exist
            let sched_dir = base_dir.join("knowledge").join("schedules");
            if sched_dir.exists() {
                if let Ok(entries) = fs::read_dir(&sched_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let fname = p.file_name().unwrap().to_string_lossy().to_string();
                            if let Ok(content) = fs::read_to_string(&p) {
                                let key = format!("schedules:{}", fname);
                                let _ =
                                    db.insert("knowledge_base", key.as_bytes(), content.as_bytes());
                            }
                        }
                    }
                }
                let _ = fs::rename(
                    &sched_dir,
                    base_dir.join("knowledge").join("schedules_migrated"),
                );
            }

            // Legacy Brain migration
            let brain_dir = base_dir.join("brain");
            if brain_dir.exists() {
                if let Ok(entries) = fs::read_dir(&brain_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            let fname = p.file_name().unwrap().to_string_lossy().to_string();
                            if let Ok(content) = fs::read_to_string(&p) {
                                let _ = db.insert(
                                    "brain_artifacts",
                                    fname.as_bytes(),
                                    content.as_bytes(),
                                );
                            }
                        }
                    }
                }
                let _ = fs::rename(&brain_dir, base_dir.join("brain_migrated"));
            }
            // ---------------------------------

            let crawler = Crawler::new();
            let search_tool =
                SearchTool::new("API_KEY".to_string(), "CX".to_string(), base_dir.clone());
            let brain_tool = BrainTool::new(db.clone());
            let terminal_tool = TerminalTool::new(base_dir.clone());
            let knowledge_tool = KnowledgeTool::new(db.clone());
            let telegram_tool = TelegramTool::new(base_dir.clone());

            let state = AgentState {
                multi_agent: Arc::new(MultiAgent::new(
                    crawler,
                    search_tool,
                    brain_tool,
                    terminal_tool,
                    knowledge_tool,
                    telegram_tool,
                )),
                base_dir: base_dir.clone(),
                cancel_flag: Arc::new(AtomicBool::new(false)),
                db: db.clone(),
            };
            let multi_agent_ref = state.multi_agent.clone();
            app.manage(state);

            let app_handle = app.handle().clone();

            let initial_config = AppConfig::load(&base_dir);
            if initial_config.telegram_enabled && !initial_config.telegram_bot_token.is_empty() {
                let ma_clone = multi_agent_ref.clone();
                let bd_clone = base_dir.clone();
                let ah_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    crate::agent::telegram::start_telegram_bot(
                        initial_config,
                        bd_clone,
                        ma_clone,
                        ah_clone,
                        db.clone(),
                    )
                    .await;
                });
            }

            tauri::async_runtime::spawn(async move {
                let mut last_tick = tokio::time::Instant::now();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    let config = AppConfig::load(&base_dir);
                    if config.heartbeat_enabled && config.heartbeat_interval > 0 {
                        let elapsed = last_tick.elapsed().as_secs();
                        let remaining = config.heartbeat_interval.saturating_sub(elapsed);
                        let _ = app_handle.emit("heartbeat_progress", remaining);

                        if elapsed >= config.heartbeat_interval {
                            last_tick = tokio::time::Instant::now();
                            let _ = app_handle.emit("heartbeat_tick", ());
                        }
                    } else {
                        last_tick = tokio::time::Instant::now();
                        let _ = app_handle.emit("heartbeat_progress", 0u64);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            execute_agent_tools,
            execute_background_scheduler,
            compress_memory,
            load_config,
            save_config,
            list_brain_artifacts,
            read_brain_artifact,
            write_brain_artifact,
            delete_brain_artifact,
            list_logs,
            read_log,
            delete_logs,
            list_knowledge,
            read_knowledge,
            write_knowledge,
            delete_knowledge,
            flush_db,
            translate_i18n,
            stop_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_dbx_contents() {
        let db_path = std::env::current_dir()
            .unwrap()
            .join("..")
            .join("PumAgentData")
            .join("pumagent_store.dbx");
        
        let db = dbx_core::Database::open(&db_path).unwrap();
        println!("--- BRAIN ARTIFACTS ---");
        if let Ok(entries) = db.scan("brain_artifacts") {
            for (k, v) in entries {
                println!("KEY: {}", String::from_utf8_lossy(&k));
                println!("VAL: {}\n", String::from_utf8_lossy(&v));
            }
        }
        println!("--- SCHEDULES (knowledge_base) ---");
        if let Ok(entries) = db.scan("knowledge_base") {
            for (k, v) in entries {
                if k.starts_with(b"schedules:") {
                    println!("KEY: {}", String::from_utf8_lossy(&k));
                    println!("VAL: {}\n", String::from_utf8_lossy(&v));
                }
            }
        }
    }
}
