use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::prelude::*;
use crate::agent::llm_client::ChatMessage;
use crate::agent::multi_agent::MultiAgent;
use crate::agent::orchestrator::Orchestrator;
use crate::agent::parser::extract_suggestions;
use crate::tools::telegram_tool::TelegramTool;
use crate::AppConfig;
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
                .branch(dptree::entry().filter_command::<Command>().endpoint(handle_commands))
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

        if data.starts_with("approve:") || data.starts_with("reject:") {
            let id = &data[8..];
            let is_approve = data.starts_with("approve:");

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
                    let _ = bot.edit_message_text(msg.chat().id, msg.id(), new_text).await;
                }
            } else {
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text("⚠️ Invalid or expired approval ID.")
                    .show_alert(true)
                    .await;
            }
        } else if data.starts_with("suggest:") {
            let suggestion = &data[8..];
            if let Some(msg) = q.message {
                // To simulate user typing the suggestion, we send it as a new message from the bot
                // but we should probably just notify the handler.
                // For simplicity, we send a message back to the chat.
                let _ = bot.send_message(msg.chat().id, suggestion).await;
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
            bot.send_message(msg.chat.id, "🔄 Conversation history has been reset.").await?;
        }
    }
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, state: TelegramState) -> ResponseResult<()> {
    let (config, base_dir, multi_agent, app_handle, db) = &*state;

    if let Some(text) = msg.text() {
        if text.starts_with("/") {
            return Ok(());
        }

        log::info!("Telegram Recv: {}", text);

        let history_key = format!("telegram_history:{}", msg.chat.id);

        let incoming_chat_id = msg.chat.id.to_string();
        if config.telegram_chat_id != incoming_chat_id {
            let mut mut_config = config.clone();
            mut_config.telegram_chat_id = incoming_chat_id;
            let _ = mut_config.save(base_dir);
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

        let mut session_history = match db.get("config", history_key.as_bytes()) {
            Ok(Some(bytes)) => {
                serde_json::from_slice::<Vec<ChatMessage>>(&bytes).unwrap_or_default()
            }
            _ => Vec::new(),
        };

        session_history.retain(|m| m.role != "system");
        session_history.push(ChatMessage {
            role: "user".to_string(),
            content: text.to_string(),
            images_base64: None,
        });

        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel::<String>(100);
        let app_handle_clone = app_handle.clone();
        let bot_clone = bot.clone();
        let chat_id_clone = msg.chat.id;
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
                let mut updated_history = history;
                if updated_history.len() > 20 {
                    let start = updated_history.len() - 20;
                    updated_history = updated_history[start..].to_vec();
                }
                if let Ok(serialized) = serde_json::to_vec(&updated_history) {
                    let _ = db.insert("config", history_key.as_bytes(), &serialized);
                    let _ = db.flush();
                }

                let (clean_text, _) = TelegramTool::clean_message_text(&final_output);
                
                // Extract suggestions
                let suggestions = extract_suggestions(&final_output);
                let mut markup = None;
                if !suggestions.is_empty() {
                    let mut buttons = Vec::new();
                    for sug in suggestions {
                        let callback_val = if sug.len() > 50 { format!("{}...", &sug[..50]) } else { sug.clone() };
                        buttons.push(vec![InlineKeyboardButton::callback(format!("➡️ {}", sug), format!("suggest:{}", callback_val))]);
                    }
                    markup = Some(InlineKeyboardMarkup::new(buttons));
                }

                let mut send_req = bot.send_message(msg.chat.id, clean_text);
                if let Some(m) = markup {
                    send_req = send_req.reply_markup(m);
                }
                let _ = send_req.await;
            }
            Err(e) => {
                let _ = bot
                    .send_message(msg.chat.id, format!("❌ Error occurred:\n{}", e))
                    .await;
            }
        }
    }
    Ok(())
}
