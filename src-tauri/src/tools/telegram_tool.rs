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

        let re = regex::Regex::new(r"```mermaid(?:\r?\n)([\s\S]*?)```").unwrap();
        let mut diagrams = Vec::new();
        for cap in re.captures_iter(message) {
            if let Some(code) = cap.get(1) {
                diagrams.push(code.as_str().to_string());
            }
        }
        let clean_text = re
            .replace_all(message, "🎨 [Mermaid Diagram Attached]")
            .to_string();

        let mut final_status = "Telegram message sent successfully.".to_string();

        match bot.send_message(chat_id, &clean_text).await {
            Ok(_) => {}
            Err(e) => {
                final_status = format!("Failed to send Telegram message: {}", e);
            }
        }

        if !diagrams.is_empty() {
            use base64::{engine::general_purpose, Engine as _};
            use flate2::write::ZlibEncoder;
            use flate2::Compression;
            use std::io::Write;
            use teloxide::types::InputFile;

            let client = rquest::Client::new();
            for diagram in diagrams {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                if encoder.write_all(diagram.as_bytes()).is_ok() {
                    if let Ok(compressed) = encoder.finish() {
                        let encoded = general_purpose::URL_SAFE.encode(&compressed);
                        let kroki_url = format!("https://kroki.io/mermaid/png/{}", encoded);

                        // We download the bytes manually into memory to avoid Url type incompatibilities
                        if let Ok(resp) = client.get(&kroki_url).send().await {
                            if let Ok(bytes) = resp.bytes().await {
                                let _ = bot
                                    .send_photo(
                                        chat_id,
                                        InputFile::memory(bytes.to_vec())
                                            .file_name("diagram.png".to_string()),
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        final_status
    }

    pub async fn execute_action(
        &self,
        tool: String,
        action: String,
        args: serde_json::Value,
    ) -> crate::agent::multi_agent::ToolResult {
        if action == "send_message" {
            let message = args.get("message").and_then(|m| m.as_str()).unwrap_or("");
            let result = self.send_message(message).await;
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action,
                ok: result.contains("successfully"),
                output: result,
            }
        } else {
            crate::agent::multi_agent::ToolResult {
                tool_name: tool,
                action: action.clone(),
                ok: false,
                output: format!("Unknown telegram action: {}", action),
            }
        }
    }
}
