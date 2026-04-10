pub mod agent;
pub mod tools;

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

pub struct AgentState {
    pub multi_agent: Arc<MultiAgent>,
    pub base_dir: std::path::PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub db: Arc<Database>,
}

#[derive(Deserialize)]
struct RunPayload {
    api_url: String,
    llm_api_key: Option<String>,
    model: String,
    system_prompt: String,
    planner_prompt: Option<String>,
    critic_prompt: Option<String>,
    writer_prompt: Option<String>,
    reflector_prompt: Option<String>,
    max_loops: u32,
    use_multi_agent_workflow: bool,
    language: String,
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
    let llm = LLMClient::new(
        payload.api_url.clone(),
        payload.model.clone(),
        llm_key.clone(),
    );
    let orchestrator = Orchestrator::new(
        llm,
        Arc::clone(&state.multi_agent),
        state.base_dir.clone(),
        state.db.clone(),
    );

    let (final_answer, history) = orchestrator
        .run_loop(
            &payload.system_prompt,
            payload.planner_prompt.as_deref(),
            payload.critic_prompt.as_deref(),
            payload.writer_prompt.as_deref(),
            payload.messages,
            &payload.language,
            payload.max_loops,
            payload.use_multi_agent_workflow,
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
            let llm = LLMClient::new(payload.api_url, payload.model, llm_key.clone());
            let bg_orchestrator = Orchestrator::new(
                llm,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_is_first_run")]
    pub is_first_run: bool,
    pub api_url: String,
    pub model: String,
    #[serde(default)]
    pub llm_api_key: String,
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
            max_loops: 20,
            system_prompt: "You are a highly capable autonomous AI assistant. Think clearly, step-by-step, and strictly follow the formatting rules to achieve the user's goal.".to_string(),
            search_provider: "duckduckgo".to_string(),
            language: "en".to_string(),
            tavily_api_key: "".to_string(),
            google_api_key: "".to_string(),
            google_cx: "".to_string(),
            use_multi_agent_workflow: false,
            planner_prompt: None,
            critic_prompt: None,
            writer_prompt: None,
            reflector_prompt: None,
            heartbeat_prompt: None,
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
fn list_brain_artifacts(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut files = vec![];
    if let Ok(entries) = state.db.scan("brain_artifacts") {
        for (key, _) in entries {
            if let Ok(name) = String::from_utf8(key) {
                files.push(name);
            }
        }
    }
    Ok(files)
}

#[tauri::command]
fn read_brain_artifact(name: String, state: State<'_, AgentState>) -> Result<String, String> {
    match state.db.get("brain_artifacts", name.as_bytes()) {
        Ok(Some(bytes)) => String::from_utf8(bytes).map_err(|e| e.to_string()),
        Ok(None) => Err("Not found".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn write_brain_artifact(
    name: String,
    content: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let res = state
        .db
        .insert("brain_artifacts", name.as_bytes(), content.as_bytes())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
fn delete_brain_artifact(name: String, state: State<'_, AgentState>) -> Result<(), String> {
    let res = state
        .db
        .delete("brain_artifacts", name.as_bytes())
        .map(|_| ())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
fn list_logs(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut logs = vec![];
    if let Ok(entries) = fs::read_dir(state.base_dir.join("logs")) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                logs.push(name);
            }
        }
    }
    logs.sort_by(|a, b| b.cmp(a)); // Descending sorting
    Ok(logs)
}

#[tauri::command]
fn read_log(name: String, state: State<'_, AgentState>) -> Result<String, String> {
    fs::read_to_string(state.base_dir.join("logs").join(name)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_logs(names: Vec<String>, state: State<'_, AgentState>) -> Result<(), String> {
    for name in names {
        let path = state.base_dir.join("logs").join(&name);
        if let Err(e) = fs::remove_file(path) {
            eprintln!("Failed to delete log {}: {}", name, e);
        }
    }
    Ok(())
}

#[tauri::command]
fn list_knowledge(domain: String, state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let mut files = vec![];
    let prefix = format!("{}:", domain);
    if let Ok(entries) = state.db.scan("knowledge_base") {
        for (key, _) in entries {
            if let Ok(name) = String::from_utf8(key) {
                if name.starts_with(&prefix) {
                    files.push(name.replace(&prefix, ""));
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
fn read_knowledge(
    domain: String,
    name: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let key = format!("{}:{}", domain, name);
    match state.db.get("knowledge_base", key.as_bytes()) {
        Ok(Some(bytes)) => String::from_utf8(bytes).map_err(|e| e.to_string()),
        Ok(None) => Err("Not found".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn write_knowledge(
    domain: String,
    name: String,
    content: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let key = format!("{}:{}", domain, name);
    let res = state
        .db
        .insert("knowledge_base", key.as_bytes(), content.as_bytes())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
fn delete_knowledge(
    domain: String,
    name: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let key = format!("{}:{}", domain, name);
    let res = state
        .db
        .delete("knowledge_base", key.as_bytes())
        .map(|_| ())
        .map_err(|e| e.to_string());
    let _ = state.db.flush();
    res
}

#[tauri::command]
async fn translate_i18n(
    target_lang: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let config = AppConfig::load(&state.base_dir);
    let llm = crate::agent::llm_client::LLMClient::new(config.api_url, config.model, config.llm_api_key);

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
        return Err(format!("Failed to parse LLM output as JSON: {}. Output: {}", e, result_json));
    }

    let key = format!("locales:{}.json", target_lang);
    state.db.insert("knowledge_base", key.as_bytes(), result_json.as_bytes()).map_err(|e| e.to_string())?;
    let _ = state.db.flush();

    Ok(result_json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    let config = AppConfig::load(&base_dir);
                    if config.heartbeat_enabled && config.heartbeat_interval > 0 {
                        if last_tick.elapsed().as_secs() >= config.heartbeat_interval {
                            last_tick = tokio::time::Instant::now();
                            let _ = app_handle.emit("heartbeat_tick", ());
                        }
                    } else {
                        // Keep last_tick fresh if disabled to prevent immediate fire upon re-enable
                        last_tick = tokio::time::Instant::now();
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            execute_agent_tools,
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
            translate_i18n,
            stop_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
