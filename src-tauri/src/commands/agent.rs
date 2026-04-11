use crate::agent::llm_client::{ChatMessage, LLMClient};
use crate::agent::orchestrator::Orchestrator;
use crate::config::AppConfig;
use crate::state::AgentState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[derive(Deserialize)]
pub struct RunPayload {
    endpoints: Vec<crate::config::LlmEndpoint>,
    session_id: Option<String>,
    planner_endpoint_id: String,
    critic_endpoint_id: String,
    writer_endpoint_id: String,
    worker_endpoint_id: String,
    reflector_endpoint_id: String,
    registry_endpoint_id: String,
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
pub struct OrchestratorResponse {
    final_output: String,
}

#[tauri::command]
pub async fn execute_agent_tools(
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
        .send("[시스템] 워커 및 각 역할별 엔드포인트 연결을 준비합니다...".to_string())
        .await;

    let orchestrator_routing = crate::agent::orchestrator::OrchestratorRouting {
        endpoints: payload.endpoints.clone(),
        planner_id: payload.planner_endpoint_id.clone(),
        critic_id: payload.critic_endpoint_id.clone(),
        writer_id: payload.writer_endpoint_id.clone(),
        worker_id: payload.worker_endpoint_id.clone(),
        reflector_id: payload.reflector_endpoint_id.clone(),
        registry_id: payload.registry_endpoint_id.clone(),
    };

    let orchestrator = Orchestrator::new(
        orchestrator_routing,
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
            let bg_routing = crate::agent::orchestrator::OrchestratorRouting {
                endpoints: payload.endpoints.clone(),
                planner_id: payload.planner_endpoint_id.clone(),
                critic_id: payload.critic_endpoint_id.clone(),
                writer_id: payload.writer_endpoint_id.clone(),
                worker_id: payload.worker_endpoint_id.clone(),
                reflector_id: payload.reflector_endpoint_id.clone(),
                registry_id: payload.registry_endpoint_id.clone(),
            };
            let bg_orchestrator = Orchestrator::new(
                bg_routing,
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
pub struct BackgroundPayload {
    endpoints: Vec<crate::config::LlmEndpoint>,
    session_id: Option<String>,
    planner_endpoint_id: String,
    critic_endpoint_id: String,
    writer_endpoint_id: String,
    worker_endpoint_id: String,
    reflector_endpoint_id: String,
    registry_endpoint_id: String,
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
pub async fn execute_background_scheduler(
    payload: BackgroundPayload,
    state: State<'_, AgentState>,
    app: AppHandle,
) -> Result<String, String> {
    let scheduler = crate::agent::scheduler::Scheduler::new(state.db.clone());
    let (pending_tasks, _, _) = scheduler.evaluate_schedules();

    if pending_tasks.is_empty() {
        return Ok("No tasks".to_string());
    }

    let bg_routing = crate::agent::orchestrator::OrchestratorRouting {
        endpoints: payload.endpoints.clone(),
        planner_id: payload.planner_endpoint_id.clone(),
        critic_id: payload.critic_endpoint_id.clone(),
        writer_id: payload.writer_endpoint_id.clone(),
        worker_id: payload.worker_endpoint_id.clone(),
        reflector_id: payload.reflector_endpoint_id.clone(),
        registry_id: payload.registry_endpoint_id.clone(),
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

    tokio::spawn(async move {
        let orchestrator = Orchestrator::new(bg_routing, multi_agent, base_dir, db);
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
pub struct CompressPayload {
    endpoints: Vec<crate::config::LlmEndpoint>,
    messages: Vec<ChatMessage>,
}

#[tauri::command]
pub async fn compress_memory(payload: CompressPayload) -> Result<String, String> {
    let endpoint =
        payload
            .endpoints
            .first()
            .cloned()
            .unwrap_or_else(|| crate::config::LlmEndpoint {
                id: "default".to_string(),
                name: "default".to_string(),
                api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
                model: "gemma-4".to_string(),
                api_key: "".to_string(),
                is_enabled: true,
            });
    let llm = LLMClient::new(endpoint.api_url, endpoint.model, endpoint.api_key);
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
pub fn stop_agent(state: State<'_, AgentState>) -> Result<(), String> {
    state.cancel_flag.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn load_config(state: State<'_, AgentState>) -> Result<AppConfig, String> {
    Ok(AppConfig::load(&state.base_dir))
}

#[tauri::command]
pub fn save_config(config: AppConfig, state: State<'_, AgentState>) -> Result<(), String> {
    let config_path = state.base_dir.join("agent_config.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn flush_db(state: State<'_, AgentState>) -> Result<(), String> {
    state.db.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn translate_i18n(
    target_lang: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let config = AppConfig::load(&state.base_dir);
    let endpoint =
        config
            .endpoints
            .first()
            .cloned()
            .unwrap_or_else(|| crate::config::LlmEndpoint {
                id: "default".to_string(),
                name: "default".to_string(),
                api_url: "http://127.0.0.1:8000/v1/chat/completions".to_string(),
                model: "gemma-4".to_string(),
                api_key: "".to_string(),
                is_enabled: true,
            });
    let llm = crate::agent::llm_client::LLMClient::new(
        endpoint.api_url,
        endpoint.model,
        endpoint.api_key,
    );

    let en_content = match state.db.get("knowledge_base", b"locales:en.json") {
        Ok(Some(bytes)) => String::from_utf8(bytes).unwrap_or_default(),
        _ => return Err("en.json base locale not found".to_string()),
    };

    let system_prompt = "You are a professional JSON translator. Translate the given localization JSON map values into the requested language. RULES:\n1. Keep all JSON keys exactly the same.\n2. Translate ONLY the values into the target language.\n3. Return strictly valid JSON with no markdown formatting, no codeblocks and no explanation.\n4. For the key `settings.lang_custom_display`, specify the target language's English name followed by its native name in parentheses, for example: `Korean(한국어)` or `Chinese(中文)`. Do NOT use the user's raw input directly.".to_string();

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

#[tauri::command]
pub async fn test_llm_connection(
    api_url: String,
    model: String,
    api_key: String,
) -> Result<String, String> {
    let client = LLMClient::new(api_url, model, api_key);
    let msg = vec![ChatMessage {
        role: "user".to_string(),
        content: "Ping!".to_string(),
        images_base64: None,
    }];

    match client.chat(&msg, 0.5).await {
        Ok(_) => Ok("Connection successful".to_string()),
        Err(e) => Err(e.to_string()),
    }
}
