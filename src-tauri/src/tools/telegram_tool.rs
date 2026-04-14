use crate::AppConfig;
use std::path::PathBuf;
use teloxide::prelude::*;
use teloxide::types::InlineKeyboardMarkup;

pub struct TelegramTool {
    base_dir: PathBuf,
}

impl TelegramTool {
    pub fn new(base_dir: PathBuf) -> Self {
        TelegramTool { base_dir }
    }

    pub fn clean_message_text(message: &str) -> (String, Vec<String>) {
        // Aggressively strip thinking/reasoning blocks using the common parser utility
        let text = crate::agent::parser::strip_thinking_blocks(message);

        let re = regex::Regex::new(r"```mermaid(?:\r?\n)([\s\S]*?)```").unwrap();
        let mut diagrams = Vec::new();
        for cap in re.captures_iter(&text) {
            if let Some(code) = cap.get(1) {
                diagrams.push(code.as_str().to_string());
            }
        }
        let clean_text = re
            .replace_all(&text, "🎨 [Mermaid Diagram Attached]")
            .to_string();

        (clean_text.trim().to_string(), diagrams)
    }

    pub async fn send_message(
        &self,
        message: &str,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> String {
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

        println!("[TelegramTool] --- SENDING MESSAGE ---");
        println!("[TelegramTool] Raw message length: {}", message.len());

        // DEBUG: Write raw message to file for inspection
        let debug_file = self.base_dir.join("debug_telegram_raw.txt");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_file)
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = std::io::Write::write_all(
                &mut file,
                format!("\n--- [{}] RAW MESSAGE ---\n{}\n---------------------------\n", timestamp, message)
                    .as_bytes(),
            );
        }

        let (clean_text, diagrams) = Self::clean_message_text(message);

        // VALIDATION: Check if think tags still exist
        let lower_clean = clean_text.to_lowercase();
        if lower_clean.contains("<think") || lower_clean.contains("<thought") || lower_clean.contains("<reasoning") {
            eprintln!("\n[TelegramTool] [CRITICAL WARNING] Think tags detected in CLEANED message!");
            eprintln!("[TelegramTool] Content: \"{}\"\n", clean_text);
        }

        println!("[TelegramTool] Cleaned message length: {}", clean_text.len());
        let preview: String = clean_text.chars().take(100).collect();
        println!("[TelegramTool] Final payload preview: \"{}...\"", preview.replace("\n", " "));

        let mut final_status = "Telegram message sent successfully.".to_string();

        let mut send_req = bot.send_message(chat_id, &clean_text);
        if let Some(markup) = reply_markup {
            send_req = send_req.reply_markup(markup);
        }

        match send_req.await {
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
            let result = self.send_message(message, None).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_message_text() {
        // Case 1: Simple message
        let (clean, _) = TelegramTool::clean_message_text("안녕");
        assert_eq!(clean, "안녕");

        // Case 2: Message with think tags
        let (clean, _) = TelegramTool::clean_message_text("<think>비밀생각</think>안녕");
        assert_eq!(clean, "안녕");

        // Case 3: Message with unclosed think tags
        let (clean, _) = TelegramTool::clean_message_text("<think>끝까지 생각중... 안녕");
        assert_eq!(clean, "");

        // Case 4: Message with multiple tags
        let (clean, _) = TelegramTool::clean_message_text("<think>생각1</think>중간<thought>생각2</thought>끝");
        assert_eq!(clean, "중간끝");

        // Case 5: Case insensitive and whitespace in tags
        let (clean, _) = TelegramTool::clean_message_text("<THINK style=\"hidden\">대문자</THINK>안녕");
        assert_eq!(clean, "안녕");
    }
}
