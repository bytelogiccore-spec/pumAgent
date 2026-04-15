use crate::agent::llm_client::{ChatMessage, LLMClient};
use crate::agent::orchestrator::Orchestrator;
use crate::config::AppConfig;
use crate::state::AgentState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

fn generate_trace_id() -> String {
    format!(
        "trace-{}-{:04x}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

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
    #[allow(dead_code)]
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
    let trace_id = generate_trace_id();
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
            serde_json::json!({"key": "log.sys_endpoints_preparing", "args": {"trace_id": trace_id}})
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

    let (final_answer, _history) = orchestrator
        .run_loop(
            payload.session_id.clone(),
            &trace_id,
            &actual_system_prompt,
            payload.planner_prompt.as_deref(),
            payload.critic_prompt.as_deref(),
            payload.writer_prompt.as_deref(),
            actual_messages,
            &payload.language,
            payload.max_loops,
            payload.use_multi_agent_workflow,
            payload.use_think_mode,
            payload.registry_prompt.as_deref(),
            tx.clone(),
            Arc::clone(&state.cancel_flag),
        )
        .await?;

    let _ = tx
        .send(format!(
            "i18n:{}",
            serde_json::json!({"key": "log.sys_execution_ended", "args": {"trace_id": trace_id}})
        ))
        .await;

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
                let _ = app_clone.emit(
                    "tool_log",
                    format!(
                        "i18n:{}",
                        serde_json::json!({"key": "log.sys_background", "args": {"msg": msg}})
                    ),
                );
            }
        }
    });

    tokio::spawn(async move {
        let trace_id = generate_trace_id();
        let orchestrator = Orchestrator::new(bg_routing, multi_agent, base_dir, db);
        let _ = orchestrator
            .run_loop(
                session_id,
                &trace_id,
                &worker.unwrap_or(sys),
                planner.as_deref(),
                critic.as_deref(),
                writer.as_deref(),
                vec![], // Empty history naturally triggers step 1 pending injection
                &lang,
                max_loops,
                use_multi,
                false, // background scheduler never uses think mode
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
pub fn get_terminal_blocklist(state: State<'_, AgentState>) -> Result<String, String> {
    match state.db.get("config", b"terminal_blocklist") {
        Ok(Some(bytes)) => {
            let json_str = String::from_utf8(bytes).unwrap_or_default();
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(&json_str) {
                Ok(arr.join(", "))
            } else {
                Ok("".to_string())
            }
        }
        _ => Ok("".to_string()),
    }
}

#[tauri::command]
pub fn save_terminal_blocklist(csv: String, state: State<'_, AgentState>) -> Result<(), String> {
    let arr: Vec<String> = csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let json_str = serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
    state
        .db
        .insert("config", b"terminal_blocklist", json_str.as_bytes())
        .map_err(|e| e.to_string())?;
    let _ = state.db.flush();
    Ok(())
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

#[tauri::command]
pub async fn summarize_log_file(
    name: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let config = AppConfig::load(&state.base_dir);
    let endpoint = config
        .endpoints
        .iter()
        .find(|e| e.is_enabled)
        .cloned()
        .ok_or_else(|| "No enabled LLM endpoints found.".to_string())?;

    let llm = LLMClient::new(endpoint.api_url, endpoint.model, endpoint.api_key);
    let log_path = state.base_dir.join("logs").join(&name);
    let content = fs::read_to_string(&log_path).map_err(|e| e.to_string())?;

    let prompt = format!(
        "Summarize the following session log. Focus on the final results, key user requests, and critical data points. \
        Strip away internal thought processes, planning chatter, and redundant tool output details. \
        The goal is a concise but complete retrospective of the session. \
        Return ONLY the summarized content in Markdown format. \n\nCONTENT:\n{}",
        content
    );

    let msgs = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
        images_base64: None,
    }];

    match llm.chat(&msgs, 0.3).await {
        Ok(res) => {
            let summary = res.content.trim();
            fs::write(&log_path, summary).map_err(|e| e.to_string())?;
            Ok(summary.to_string())
        }
        Err(e) => Err(format!("LLM Summarization failed: {}", e)),
    }
}

#[tauri::command]
pub async fn ai_summarize_item(
    domain: String,
    name: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let config = AppConfig::load(&state.base_dir);
    let endpoint = config
        .endpoints
        .iter()
        .find(|e| e.is_enabled)
        .cloned()
        .ok_or_else(|| "No enabled LLM endpoints found.".to_string())?;

    let llm = LLMClient::new(endpoint.api_url, endpoint.model, endpoint.api_key);

    if domain == "brain" {
        let brain = crate::tools::brain::BrainTool::new(state.db.clone());
        let res = brain
            .execute_action(
                "brain".into(),
                "summarize".into(),
                serde_json::json!({"name": name}),
                Some(llm),
            )
            .await;
        if res.ok {
            Ok(res.output)
        } else {
            Err(res.output)
        }
    } else {
        let knowledge = crate::tools::knowledge::KnowledgeTool::new(state.db.clone());
        let res = knowledge
            .execute_action(
                "knowledge".into(),
                "summarize".into(),
                serde_json::json!({"domain": domain, "name": name}),
                Some(llm),
                None,
            )
            .await;
        if res.ok {
            Ok(res.output)
        } else {
            Err(res.output)
        }
    }
}

#[tauri::command]
pub fn get_next_execution_time(config: crate::agent::scheduler::ScheduleConfig) -> String {
    config.get_next_run_time(chrono::Utc::now())
}

#[tauri::command]
pub async fn list_extensions(
    state: State<'_, AgentState>,
) -> Result<Vec<crate::agent::multi_agent::ExternalToolMetadata>, String> {
    Ok(state.multi_agent.list_external_tools())
}

#[tauri::command]
pub async fn reload_extensions(state: State<'_, AgentState>) -> Result<usize, String> {
    state.multi_agent.refresh_external_tools()
}

#[tauri::command]
pub async fn set_extension_enabled(
    extension_name: String,
    enabled: bool,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    let mut config = AppConfig::load(&state.base_dir);
    if enabled {
        config.disabled_extensions.retain(|n| n != &extension_name);
    } else if !config
        .disabled_extensions
        .iter()
        .any(|n| n == &extension_name)
    {
        config.disabled_extensions.push(extension_name);
    }
    config.save(&state.base_dir)?;
    state.multi_agent.refresh_external_tools()?;
    Ok(())
}

#[tauri::command]
pub fn list_session_tree(
    session_id: String,
    state: State<'_, AgentState>,
) -> Result<crate::state::SessionTree, String> {
    let key = format!("session_tree:{}", session_id);
    let bytes = state
        .db
        .get("sessions", key.as_bytes())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Session tree not found".to_string())?;
    serde_json::from_slice::<crate::state::SessionTree>(&bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fork_session_branch(
    source_session_id: String,
    from_node_id: Option<String>,
    new_session_id: String,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    let source_key = format!("session_tree:{}", source_session_id);
    let bytes = state
        .db
        .get("sessions", source_key.as_bytes())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Source session tree not found".to_string())?;
    let source_tree =
        serde_json::from_slice::<crate::state::SessionTree>(&bytes).map_err(|e| e.to_string())?;

    let target_node = from_node_id.or_else(|| source_tree.active_node_id.clone());
    let mut keep = Vec::new();
    for node in &source_tree.nodes {
        keep.push(node.clone());
        if Some(node.id.clone()) == target_node {
            break;
        }
    }
    if keep.is_empty() {
        return Err("Cannot fork empty branch".to_string());
    }

    let mut parent: Option<String> = None;
    let mut new_nodes = Vec::new();
    for (idx, node) in keep.into_iter().enumerate() {
        let id = format!("{}-{}", new_session_id, idx + 1);
        let mut cloned = node.clone();
        cloned.id = id.clone();
        cloned.parent_id = parent.clone();
        parent = Some(id);
        new_nodes.push(cloned);
    }

    let forked = crate::state::SessionTree {
        session_id: new_session_id.clone(),
        forked_from_session_id: Some(source_session_id),
        active_node_id: parent,
        nodes: new_nodes,
    };

    let key = format!("session_tree:{}", new_session_id);
    let json = serde_json::to_vec(&forked).map_err(|e| e.to_string())?;
    state
        .db
        .insert("sessions", key.as_bytes(), &json)
        .map_err(|e| e.to_string())?;
    let _ = state.db.flush();
    Ok(new_session_id)
}

#[tauri::command]
pub fn resume_session_node(
    session_id: String,
    node_id: String,
    state: State<'_, AgentState>,
) -> Result<Vec<ChatMessage>, String> {
    let key = format!("session_tree:{}", session_id);
    let bytes = state
        .db
        .get("sessions", key.as_bytes())
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Session tree not found".to_string())?;
    let tree =
        serde_json::from_slice::<crate::state::SessionTree>(&bytes).map_err(|e| e.to_string())?;
    let mut index = None;
    for (idx, node) in tree.nodes.iter().enumerate() {
        if node.id == node_id {
            index = Some(idx);
            break;
        }
    }
    let idx = index.ok_or_else(|| "Session node not found".to_string())?;
    let mut out = Vec::new();
    for node in tree.nodes.into_iter().take(idx + 1) {
        out.push(ChatMessage {
            role: node.role,
            content: node.content,
            images_base64: None,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn get_search_metrics(
    limit: Option<usize>,
    state: State<'_, AgentState>,
) -> Result<Vec<serde_json::Value>, String> {
    let mut items = Vec::new();
    let max = limit.unwrap_or(200);
    let entries = state.db.scan("metrics").map_err(|e| e.to_string())?;
    for (k, v) in entries {
        let key = String::from_utf8_lossy(&k);
        if !key.starts_with("search_metric:") {
            continue;
        }
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&v) {
            items.push(json);
        }
    }
    items.sort_by(|a, b| {
        b.get("timestamp")
            .and_then(|t| t.as_str())
            .cmp(&a.get("timestamp").and_then(|t| t.as_str()))
    });
    items.truncate(max);
    Ok(items)
}

#[tauri::command]
pub fn get_provider_health(state: State<'_, AgentState>) -> Result<serde_json::Value, String> {
    let entries = state.db.scan("metrics").map_err(|e| e.to_string())?;
    let mut by_provider: std::collections::HashMap<String, (u32, u32, i64)> =
        std::collections::HashMap::new();
    for (k, v) in entries {
        let key = String::from_utf8_lossy(&k);
        if !key.starts_with("search_metric:") {
            continue;
        }
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&v) {
            let provider = json
                .get("provider")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string();
            let status = json.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let elapsed = json.get("elapsed_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            let entry = by_provider.entry(provider).or_insert((0, 0, 0));
            if status == "success" {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
            entry.2 += elapsed;
        }
    }

    let mut out = serde_json::Map::new();
    for (provider, (success, fail, elapsed_sum)) in by_provider {
        let total = (success + fail).max(1);
        let success_rate = success as f64 / total as f64;
        let avg_latency = elapsed_sum as f64 / total as f64;
        let score = (success_rate * 100.0) - (avg_latency / 1000.0).min(20.0);
        out.insert(
            provider,
            serde_json::json!({
                "success": success,
                "fail": fail,
                "avg_latency_ms": avg_latency,
                "health_score": score.max(0.0),
            }),
        );
    }
    Ok(serde_json::Value::Object(out))
}

#[tauri::command]
pub fn run_prompt_tool_lint(state: State<'_, AgentState>) -> Result<Vec<String>, String> {
    let config = AppConfig::load(&state.base_dir);
    let mut prompts = Vec::new();
    prompts.push(config.system_prompt);
    if let Some(v) = config.planner_prompt {
        prompts.push(v);
    }
    if let Some(v) = config.critic_prompt {
        prompts.push(v);
    }
    if let Some(v) = config.writer_prompt {
        prompts.push(v);
    }
    if let Some(v) = config.reflector_prompt {
        prompts.push(v);
    }
    prompts.push(crate::agent::prompts::get_fallback_reflector_prompt().to_string());
    Ok(state.multi_agent.lint_prompt_tool_alignment(&prompts))
}
