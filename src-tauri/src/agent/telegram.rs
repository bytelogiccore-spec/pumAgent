use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::prelude::*;
use tokio::sync::mpsc;

use crate::agent::llm_client::ChatMessage;
use crate::agent::multi_agent::MultiAgent;
use crate::agent::orchestrator::Orchestrator;
use crate::tools::telegram_tool::TelegramTool;
use crate::AppConfig;

type TelegramState = Arc<(
    AppConfig,
    PathBuf,
    Arc<MultiAgent>,
    AppHandle,
    Arc<dbx_core::Database>,
)>;

pub async fn start_telegram_bot(
    config: AppConfig,
    base_dir: PathBuf,
    multi_agent: Arc<MultiAgent>,
    app_handle: AppHandle,
    db: Arc<dbx_core::Database>,
) {
    log::info!("Telegram Bot Listener starting...");
    let bot = Bot::new(config.telegram_bot_token.clone());

    // Validate token and announce in GUI
    match bot.get_me().await {
        Ok(me) => {
            let msg = format!(
                "i18n:{}",
                serde_json::json!({"key": "log.sys_telegram_success", "args": {"username": me.user.username.unwrap_or_default()}})
            );
            let _ = app_handle.emit("tool_log", msg);
        }
        Err(e) => {
            let msg = format!(
                "i18n:{}",
                serde_json::json!({"key": "log.sys_telegram_failed", "args": {"err": e.to_string()}})
            );
            let _ = app_handle.emit("tool_log", msg);
            return; // Stop if invalid token
        }
    }

    // We use a shared state tuple for the Handler
    let state: TelegramState = Arc::new((config, base_dir, multi_agent, app_handle, db));

    let handler = Update::filter_message().endpoint(
        |bot: Bot, msg: Message, state: TelegramState| async move {
            let (config, base_dir, multi_agent, app_handle, db) = &*state;

            if let Some(text) = msg.text() {
                log::info!("Telegram Recv: {}", text);

                if text.starts_with("/approve ") || text.starts_with("/reject ") {
                    let parts: Vec<&str> = text.split_whitespace().collect();
                    if parts.len() == 2 {
                        let id = parts[1];
                        let is_approve = text.starts_with("/approve");
                        let mut map = crate::agent::approval::pending_approvals().lock().await;
                        if let Some(tx) = map.remove(id) {
                            let _ = tx.send(is_approve);
                            let resp_msg = if is_approve {
                                "✅ Execution Approved."
                            } else {
                                "❌ Execution Rejected."
                            };
                            let _ = bot.send_message(msg.chat.id, resp_msg).await;
                        } else {
                            let _ = bot
                                .send_message(msg.chat.id, "⚠️ Invalid or expired approval ID.")
                                .await;
                        }
                        return respond(());
                    }
                }

                let history_key = format!("telegram_history:{}", msg.chat.id);

                if text == "/reset" {
                    let _ = db.delete("config", history_key.as_bytes());
                    let _ = bot
                        .send_message(msg.chat.id, "🔄 Conversation history has been reset.")
                        .await;
                    return respond(());
                }

                let incoming_chat_id = msg.chat.id.to_string();
                if config.telegram_chat_id != incoming_chat_id {
                    log::info!("Updating telegram_chat_id to {}", incoming_chat_id);
                    let mut mut_config = config.clone();
                    mut_config.telegram_chat_id = incoming_chat_id;
                    if let Err(e) = mut_config.save(base_dir) {
                        log::error!("Failed to save new telegram_chat_id: {}", e);
                    }
                    let _ = app_handle.emit(
                        "tool_log",
                        format!(
                            "i18n:{}",
                            serde_json::json!({"key": "log.sys_telegram_linked", "args": {}})
                        ),
                    );
                }

                let _ = bot
                    .send_message(msg.chat.id, format!("🤖 Processing started: `{}`", text))
                    .await;

                let routing_flags = super::orchestrator::OrchestratorRouting {
                    endpoints: config.endpoints.clone(),
                    planner_id: config.planner_endpoint_id.clone(),
                    critic_id: config.critic_endpoint_id.clone(),
                    writer_id: config.writer_endpoint_id.clone(),
                    worker_id: config.worker_endpoint_id.clone(),
                    reflector_id: config.reflector_endpoint_id.clone(),
                    registry_id: config.registry_endpoint_id.clone(),
                };
                let orchestrator = Orchestrator::new(
                    routing_flags,
                    multi_agent.clone(),
                    base_dir.clone(),
                    db.clone(),
                );

                // Load session history from DB
                let mut session_history = match db.get("config", history_key.as_bytes()) {
                    Ok(Some(bytes)) => {
                        serde_json::from_slice::<Vec<ChatMessage>>(&bytes).unwrap_or_default()
                    }
                    _ => Vec::new(),
                };

                // Filter out system messages from persisted history to avoid duplication
                session_history.retain(|m| m.role != "system");

                session_history.push(ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                    images_base64: None,
                });

                // Channel for capturing internal logs and broadcasting them to Tauri GUI
                let (log_tx, mut log_rx) = mpsc::channel::<String>(100);
                let app_handle_clone = app_handle.clone();
                let bot_clone = bot.clone();
                let chat_id_clone = msg.chat.id;
                let lang_clone = config.language.clone();
                let db_clone = db.clone();

                tokio::spawn(async move {
                    while let Some(log_msg) = log_rx.recv().await {
                        // Broadcast log to desktop GUI
                        let _ = app_handle_clone.emit("tool_log", log_msg.clone());

                        // Also forward translated log to Telegram for "Call Info" reporting
                        let resolved = Orchestrator::resolve_i18n(&log_msg, &lang_clone, &db_clone);
                        let (clean_log, _) = TelegramTool::clean_message_text(&resolved);
                        let _ = bot_clone
                            .send_message(chat_id_clone, format!("📋 {}", clean_log))
                            .await;
                    }
                });

                let cancel_flag = Arc::new(AtomicBool::new(false));

                // Execute the agent pipeline wrapper
                let res = orchestrator
                    .run_loop(
                        Some(format!("Telegram_{}", msg.chat.id)),
                        &config.system_prompt,
                        config.planner_prompt.as_deref(),
                        config.critic_prompt.as_deref(),
                        config.writer_prompt.as_deref(),
                        session_history,
                        &config.language,
                        config.max_loops,
                        config.use_multi_agent_workflow,
                        config.registry_prompt.as_deref(),
                        log_tx,
                        cancel_flag,
                    )
                    .await;

                match res {
                    Ok((final_output, history)) => {
                        // Persist the updated history (limiting to last 20 messages for context)
                        let mut updated_history = history;
                        if updated_history.len() > 20 {
                            let start = updated_history.len() - 20;
                            updated_history = updated_history[start..].to_vec();
                        }
                        if let Ok(serialized) = serde_json::to_vec(&updated_history) {
                            let _ = db.insert("config", history_key.as_bytes(), &serialized);
                            let _ = db.flush();
                        }

                        // Aggressively clean final output to strip thinking blocks and mermaid diagrams
                        let (clean_text, _diagrams) =
                            TelegramTool::clean_message_text(&final_output);
                        let _ = bot.send_message(msg.chat.id, clean_text).await;
                    }
                    Err(e) => {
                        let _ = bot
                            .send_message(msg.chat.id, format!("❌ Error occurred:\n{}", e))
                            .await;
                    }
                }
            }
            respond(())
        },
    );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
