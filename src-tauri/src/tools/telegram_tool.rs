use crate::AppConfig;
use std::path::PathBuf;
use teloxide::prelude::*;

pub struct TelegramTool {
    base_dir: PathBuf,
}

impl TelegramTool {
    pub fn new(base_dir: PathBuf) -> Self {
        TelegramTool { base_dir }
    }

    pub async fn send_message(&self, message: &str) -> String {
        // We load the config fresh every time to ensure we have the latest token and chat_id
        let config = AppConfig::load(&self.base_dir);

        if !config.telegram_enabled {
            return "Failed to send message: Telegram integration is disabled in settings."
                .to_string();
        }

        if config.telegram_bot_token.trim().is_empty() {
            return "Failed to send message: Telegram bot token is missing.".to_string();
        }

        if config.telegram_chat_id.trim().is_empty() {
            return "Failed to send message: Telegram chat id is unknown. The user must send a message to the bot first to register their chat ID.".to_string();
        }

        let bot = Bot::new(config.telegram_bot_token.clone());
        let chat_id = ChatId(config.telegram_chat_id.parse::<i64>().unwrap_or_default());

        match bot.send_message(chat_id, message).await {
            Ok(_) => "Telegram message sent successfully.".to_string(),
            Err(e) => format!("Failed to send Telegram message: {}", e),
        }
    }
}
