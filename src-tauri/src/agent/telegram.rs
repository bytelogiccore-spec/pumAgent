use crate::agent::llm_client::ChatMessage;
use crate::agent::multi_agent::MultiAgent;
use crate::agent::orchestrator::Orchestrator;
use crate::agent::parser::extract_suggestions;
use crate::tools::telegram_tool::TelegramTool;
use crate::AppConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Command {
    #[command(description = "Start the bot or get help.")]
    Help,
    #[command(description = "Reset current conversation history.")]
    Reset,
}

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

    let state: TelegramState = Arc::new((config, base_dir, multi_agent, app_handle, db));

    // Register command menu
    let _ = bot.set_my_commands(Command::bot_commands()).await;

    let handler = dptree::entry()
        .branch(Update::filter_callback_query().endpoint(handle_callback_query))
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<Command>()
                        .endpoint(handle_commands),
                )
                .branch(dptree::endpoint(handle_message)),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    state: TelegramState,
) -> ResponseResult<()> {
    if let Some(data) = q.data {
        let (_, _, _, _, _db) = &*state;

        let (id, is_approve) = if let Some(id) = data.strip_prefix("approve:") {
            (Some(id), true)
        } else if let Some(id) = data.strip_prefix("reject:") {
            (Some(id), false)
        } else {
            (None, false)
        };

        if let Some(id) = id {
            let mut map = crate::agent::approval::pending_approvals().lock().await;
            if let Some(tx) = map.remove(id) {
                let _ = tx.send(is_approve);
                let (icon, status) = if is_approve {
                    ("✅", "Approved")
                } else {
                    ("❌", "Rejected")
                };

                if let Some(msg) = q.message {
                    let new_text = format!("{} {}\nOriginal request processed.", icon, status);
                    let _ = bot
                        .edit_message_text(msg.chat().id, msg.id(), new_text)
                        .await;
                }
            } else {
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text("⚠️ Invalid or expired approval ID.")
                    .show_alert(true)
                    .await;
            }
        } else if let Some(suggestion) = data.strip_prefix("suggest:") {
            if let Some(msg) = q.message {
                // Trigger full reasoning loop for the suggestion
                process_telegram_request(
                    bot.clone(),
                    msg.chat().id,
                    suggestion.to_string(),
                    state.clone(),
                )
                .await?;
            }
        }
    }
    let _ = bot.answer_callback_query(q.id).await;
    Ok(())
}

async fn handle_commands(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: TelegramState,
) -> ResponseResult<()> {
    let (_, _, _, _, db) = &*state;
    match cmd {
        Command::Help => {
            let help_text = Command::descriptions().to_string();
            bot.send_message(msg.chat.id, help_text).await?;
        }
        Command::Reset => {
            let history_key = format!("telegram_history:{}", msg.chat.id);
            let _ = db.delete("config", history_key.as_bytes());
            bot.send_message(msg.chat.id, "🔄 Conversation history has been reset.")
                .await?;
        }
    }
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, state: TelegramState) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        if text.starts_with("/") {
            return Ok(());
        }
        process_telegram_request(bot, msg.chat.id, text.to_string(), state).await?;
    }
    Ok(())
}

async fn process_telegram_request(
    bot: Bot,
    chat_id: ChatId,
    text: String,
    state: TelegramState,
) -> ResponseResult<()> {
    let (config, base_dir, multi_agent, app_handle, db) = &*state;

    log::info!("Telegram Processing: chat_id={}, text={}", chat_id, text);

    let _ = bot
        .send_message(chat_id, format!("🤖 Processing started: `{}`", text))
        .await;

    let routing_flags = crate::agent::orchestrator::OrchestratorRouting {
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

    let history_key = format!("telegram_history:{}", chat_id);
    let mut session_history = match db.get("config", history_key.as_bytes()) {
        Ok(Some(bytes)) => serde_json::from_slice::<Vec<ChatMessage>>(&bytes).unwrap_or_default(),
        _ => Vec::new(),
    };

    session_history.retain(|m| m.role != "system");
    session_history.push(ChatMessage {
        role: "user".to_string(),
        content: text.clone(),
        images_base64: None,
    });

    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(100);
    let app_handle_clone = app_handle.clone();
    let bot_clone = bot.clone();
    let chat_id_clone = chat_id;
    let lang_clone = config.language.clone();
    let db_clone = db.clone();

    tokio::spawn(async move {
        while let Some(log_msg) = log_rx.recv().await {
            let _ = app_handle_clone.emit("tool_log", log_msg.clone());
            let resolved = Orchestrator::resolve_i18n(&log_msg, &lang_clone, &db_clone);
            let (clean_log, _) = TelegramTool::clean_message_text(&resolved);
            let _ = bot_clone
                .send_message(chat_id_clone, format!("📋 {}", clean_log))
                .await;
        }
    });

    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let res = orchestrator
        .run_loop(
            Some(format!("Telegram_{}", chat_id)),
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
            let mut updated_history = history;
            if updated_history.len() > 20 {
                let start = updated_history.len() - 20;
                updated_history = updated_history[start..].to_vec();
            }
            if let Ok(serialized) = serde_json::to_vec(&updated_history) {
                let _ = db.insert("config", history_key.as_bytes(), &serialized);
                let _ = db.flush();
            }

            let resolved_final = Orchestrator::resolve_i18n(&final_output, &config.language, db);
            let (mut clean_text, _) = TelegramTool::clean_message_text(&resolved_final);

            if clean_text.trim().is_empty() {
                clean_text = "✅ Task completed. (No text response)".to_string();
            }

            // Extract suggestions before stripping them in clean_message_text if they weren't matched?
            // Actually they were already stripped in clean_message_text, so we must extract from resolved_final
            let suggestions = extract_suggestions(&resolved_final);
            
            let mut markup = None;
            if !suggestions.is_empty() {
                let mut buttons = Vec::new();
                for sug in suggestions {
                    // Telegram callback_data limit is 64 BYTES. 
                    // 'suggest:' prefix is 8 bytes. We have ~56 bytes for content.
                    let prefix = "suggest:";
                    let max_bytes = 64 - prefix.len();
                    
                    let mut callback_val = sug.clone();
                    if callback_val.len() > max_bytes {
                        // Safely truncate to byte limit at char boundary
                        let mut end = max_bytes - 3; // reserve for "..."
                        while !callback_val.is_char_boundary(end) && end > 0 {
                            end -= 1;
                        }
                        callback_val = format!("{}...", &callback_val[..end]);
                    }

                    buttons.push(vec![InlineKeyboardButton::callback(
                        format!("➡️ {}", sug),
                        format!("{}{}", prefix, callback_val),
                    )]);
                }
                markup = Some(InlineKeyboardMarkup::new(buttons));
            }

            let mut send_req = bot.send_message(chat_id, clean_text);
            if let Some(m) = markup {
                send_req = send_req.reply_markup(m);
            }
            if let Err(e) = send_req.await {
                log::error!("Failed to send final telegram message: {}", e);
                let _ = bot.send_message(chat_id, format!("❌ Error sending message: {}", e)).await;
            }
        }
        Err(e) => {
            let _ = bot
                .send_message(chat_id, format!("❌ Error occurred:\n{}", e))
                .await;
        }
    }

    Ok(())
}
