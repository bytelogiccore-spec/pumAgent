use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use teloxide::prelude::*;
use tokio::sync::mpsc;

use crate::agent::llm_client::{ChatMessage, LLMClient};
use crate::agent::multi_agent::MultiAgent;
use crate::agent::orchestrator::Orchestrator;
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
                "[시스템] 텔레그램 봇 연동 성공! (@{}) - 대기중...",
                me.user.username.unwrap_or_default()
            );
            let _ = app_handle.emit("tool_log", msg);
        }
        Err(e) => {
            let msg = format!(
                "[시스템 오류] 텔레그램 봇 연동 실패 (토큰 확인 필요): {}",
                e
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
                        "[시스템] 텔레그램 채팅방 ID 연동 완료. (Push 알림 가능)".to_string(),
                    );
                }

                let _ = bot
                    .send_message(msg.chat.id, format!("🤖 처리 시작: `{}`", text))
                    .await;

                // Prepare Orchestrator
                let local_llm = LLMClient::new(
                    config.api_url.clone(),
                    config.model.clone(),
                    config.llm_api_key.clone(),
                );
                let cloud_llm = if config.cloud_api_url.is_empty() {
                    local_llm.clone()
                } else {
                    LLMClient::new(
                        config.cloud_api_url.clone(),
                        config.cloud_model.clone(),
                        config.cloud_llm_api_key.clone(),
                    )
                };
                let routing_flags = super::orchestrator::CloudRoutingFlags {
                    planner: config.cloud_routing_planner,
                    critic: config.cloud_routing_critic,
                    writer: config.cloud_routing_writer,
                    worker: config.cloud_routing_worker,
                };
                let orchestrator = Orchestrator::new(
                    local_llm,
                    cloud_llm,
                    routing_flags,
                    multi_agent.clone(),
                    base_dir.clone(),
                    db.clone(),
                );

                let user_msgs = vec![ChatMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                    images_base64: None,
                }];

                // Channel for capturing internal logs and broadcasting them to Tauri GUI
                let (log_tx, mut log_rx) = mpsc::channel::<String>(100);
                let app_handle_clone = app_handle.clone();

                tokio::spawn(async move {
                    while let Some(log_msg) = log_rx.recv().await {
                        // Broadcast log to desktop GUI
                        let _ = app_handle_clone.emit("tool_log", log_msg.clone());
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
                        user_msgs,
                        &config.language,
                        config.max_loops,
                        config.use_multi_agent_workflow,
                        config.registry_prompt.as_deref(),
                        log_tx,
                        cancel_flag,
                    )
                    .await;

                match res {
                    Ok((final_output, _history)) => {
                        let _ = bot.send_message(msg.chat.id, final_output).await;
                    }
                    Err(e) => {
                        let _ = bot
                            .send_message(msg.chat.id, format!("❌ 오류 발생:\n{}", e))
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
