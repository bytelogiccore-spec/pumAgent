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

    pub async fn execute_action(&self, tool: String, action: String, args: serde_json::Value) -> crate::agent::multi_agent::ToolResult {
        if action == "send_message" {
            let message = args.get("message").and_then(|m| m.as_str()).unwrap_or("");
            let result = self.send_message(message).await;
            crate::agent::multi_agent::ToolResult { tool_name: tool, action, ok: result.contains("successfully"), output: result }
        } else {
            crate::agent::multi_agent::ToolResult { tool_name: tool, action: action.clone(), ok: false, output: format!("Unknown telegram action: {}", action) }
        }
    }
}
