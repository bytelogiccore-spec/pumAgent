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
        .send(format!(
            "i18n:{}",
            serde_json::json!({"key": "log.sys_endpoints_preparing", "args": {}})
        ))
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
    let mut actual_system_prompt = if payload.use_multi_agent_workflow {
        payload
            .worker_prompt
            .unwrap_or_else(|| payload.system_prompt.clone())
    } else {
        payload.system_prompt.clone()
    };
    actual_system_prompt.push_str(&format!(
        "\n\n[GLOBAL RULE]\nCRITICAL: Any content intended for the user (such as final output or Telegram notifications) MUST be composed in {}.",
        payload.language
    ));
    // Inject Visualization capability
    actual_system_prompt.push_str("\n\n[VISUALIZATION DIRECTIVE]\nWhen explaining complex logic, relationships, architecture, or flows, you MUST use mermaid.js diagrams by wrapping them in ```mermaid blocks. Use Flowcharts, Sequence Diagrams, or State Diagrams where appropriate to drastically enhance readability.");

    if !payload.use_think_mode {
        actual_system_prompt.push_str("\n\n!! CRITICAL DIRECTIVE !!\nDO NOT OUTPUT ANY REASONING, THINKING, OR THOUGHT BLOCKS. DO NOT USE <think> OR SIMILAR TAGS. PROVIDE YOUR FINAL RESPONSE DIRECTLY AND IMMEDIATELY.");
    }

    let mut actual_messages = payload.messages;
    if let Some(last_msg) = actual_messages.last_mut() {
        if last_msg.role == "user" {
            if let Ok(re) = regex::Regex::new(r"https?://[^\s]+") {
                let mut appended_context = String::new();
                let content_clone = last_msg.content.clone();
                for cap in re.captures_iter(&content_clone) {
                    let url = cap[0].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/');
                    let _ = tx.send(format!("i18n:{}", serde_json::json!({"key": "log.sys_auto_scraping_started", "args": {"url": url}}))).await;
                    let crawler = crate::tools::crawler::Crawler::new();
                    match crawler.scrape(url).await {
                        Ok(markdown) => {
                            let _ = tx.send(format!("i18n:{}", serde_json::json!({"key": "log.sys_auto_scraping_success", "args": {}}))).await;
                            appended_context.push_str(&format!(
                                "\n\n[SYSTEM PRE-FETCHED CONTENT FOR {}]\n{}\n",
                                url, markdown
                            ));
                        }
                        Err(e) => {
                            let _ = tx.send(format!("i18n:{}", serde_json::json!({"key": "log.sys_auto_scraping_failed", "args": {"err": e.to_string()}}))).await;
                        }
                    }
                }
                if !appended_context.is_empty() {
                    last_msg.content.push_str(&appended_context);
                    last_msg.content.push_str("\n\n[SYSTEM DIRECTIVE]: The system has already Pre-fetched the requested URL(s) above. Use this attached data to resolve the user's request directly. You DO NOT need to use `search` or `crawl4ai` tools again unless strictly necessary!");
                }
            }
        }
    }

    let (final_answer, history) = orchestrator
        .run_loop(
            payload.session_id.clone(),
            &actual_system_prompt,
            payload.planner_prompt.as_deref(),
            payload.critic_prompt.as_deref(),
            payload.writer_prompt.as_deref(),
            actual_messages,
            &payload.language,
            payload.max_loops,
            payload.use_multi_agent_workflow,
            payload.registry_prompt.as_deref(),
            tx.clone(),
            Arc::clone(&state.cancel_flag),
        )
        .await?;

    let _ = tx
        .send(format!(
            "i18n:{}",
            serde_json::json!({"key": "log.sys_execution_ended", "args": {}})
        ))
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
            if msg.starts_with("i18n:") {
                let _ = app_clone.emit("tool_log", msg);
            } else {
                let _ = app_clone.emit("tool_log", format!("[백그라운드] {}", msg));
            }
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
        content: "Draft a concise and clear summary of the provided past conversation and thought history. Focus on key facts, user requests, collected information, and remaining plans to execute. Return only the compressed context without any greetings or introductions. You MUST respond in the same language as the user's latest query.".to_string(),
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
        content: format!(
            "Summarize the following past conversation history:\n\n{}",
            combined_text
        ),
        images_base64: None,
    };

    let history = vec![system_msg, user_msg];

    match llm.chat(&history, 0.4).await {
        Ok(res) => Ok(res.content.trim().to_string()),
        Err(e) => Err(format!("Memory compression error: {}", e)),
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

    // Sub-routine: Ask LLM for the display name explicitly
    let name_msg = vec![
        crate::agent::llm_client::ChatMessage {
            role: "system".to_string(),
            content: "You are a naming assistant. Output EXACTLY EnglishName(NativeName) and nothing else.".to_string(),
            images_base64: None,
        },
        crate::agent::llm_client::ChatMessage {
            role: "user".to_string(),
            content: format!("What is the language '{}'? Format EXACTLY as: EnglishName(NativeName).\nExample: Korean(한국어). Give NO other text.", target_lang),
            images_base64: None,
        },
    ];
    let mut custom_display = target_lang.clone();
    if let Ok(res) = llm.chat(&name_msg, 0.1).await {
        let txt = res.content.trim().to_string();
        if !txt.is_empty() && txt.contains("(") && txt.contains(")") {
            custom_display = txt;
        }
    }

    let system_prompt = "You are a professional JSON translator. Translate all map values into the requested target language. RULES: Keep JSON keys exactly the same. Translate ONLY the values. Return strictly valid JSON with no markdown formatting.".to_string();

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

    let mut parsed_json = match serde_json::from_str::<serde_json::Value>(&result_json) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!(
                "Failed to parse LLM output as JSON: {}. Output: {}",
                e, result_json
            ))
        }
    };

    if let Some(obj) = parsed_json.as_object_mut() {
        obj.insert(
            "settings.lang_custom_display".to_string(),
            serde_json::Value::String(custom_display),
        );
    }

    let final_result = serde_json::to_string_pretty(&parsed_json).unwrap_or(result_json.clone());

    let key = format!("locales:{}.json", target_lang);
    state
        .db
        .insert("knowledge_base", key.as_bytes(), final_result.as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = state.db.flush();

    Ok(final_result)
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
#[tauri::command]
pub async fn list_vault_keys(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let vault = crate::tools::vault_tool::VaultTool::new(state.base_dir.clone());
    Ok(vault.list_keys())
}

#[tauri::command]
pub async fn set_vault_secret(
    state: State<'_, AgentState>,
    key: String,
    value: String,
) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err("Value cannot be empty.".into());
    }
    let vault = crate::tools::vault_tool::VaultTool::new(state.base_dir.clone());
    vault.set_secret(&key, &value)
}

#[tauri::command]
pub async fn delete_vault_secret(state: State<'_, AgentState>, key: String) -> Result<(), String> {
    let vault = crate::tools::vault_tool::VaultTool::new(state.base_dir.clone());
    vault.delete_secret(&key)
}
