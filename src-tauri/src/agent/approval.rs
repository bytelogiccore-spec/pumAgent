use crate::tools::telegram_tool::TelegramTool;
use rand::RngCore;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::OnceLock;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use tokio::sync::{oneshot, Mutex};

pub struct PendingApproval {
    pub tx: oneshot::Sender<bool>,
    pub request_hash: String,
    pub expires_at: Instant,
}

pub type PendingMap = Mutex<HashMap<String, PendingApproval>>;

pub fn pending_approvals() -> &'static PendingMap {
    static MAP: OnceLock<PendingMap> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn request_approval(telegram_tool: &TelegramTool, command_preview: &str) -> bool {
    let id_str = random_token_hex(16);
    let request_hash = short_request_hash(command_preview);

    let (tx, rx) = oneshot::channel();

    {
        let mut map = pending_approvals().lock().await;
        map.retain(|_, approval| approval.expires_at > Instant::now());
        map.insert(
            id_str.clone(),
            PendingApproval {
                tx,
                request_hash: request_hash.clone(),
                expires_at: Instant::now() + Duration::from_secs(300),
            },
        );
    }

    let msg = format!(
        "🚨 *SECURITY ALERT* 🚨\nThe agent is attempting to execute a potentially dangerous system command:\n\n`{}`\n\nPlease select an action below:",
        command_preview
    );

    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(
            "✅ Approve",
            format!("approve:{}:{}", id_str, request_hash),
        ),
        InlineKeyboardButton::callback("❌ Reject", format!("reject:{}:{}", id_str, request_hash)),
    ]]);

    let status = telegram_tool.send_message(&msg, Some(keyboard)).await;
    if status.contains("Failed") {
        // If telegram fails to send, we should automatically reject to be safe,
        // and remove it from the map.
        let mut map = pending_approvals().lock().await;
        map.remove(&id_str);
        return false;
    }

    rx.await.unwrap_or_default()
}

pub fn build_approval_payload(tool: &str, action: &str, content: &str) -> String {
    let hash = short_request_hash(content);
    format!(
        "Tool: {}\nAction: {}\nFingerprint: {}\nPayload: {}",
        tool, action, hash, content
    )
}

fn random_token_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{:02x}", b)).collect::<String>()
}

fn short_request_hash(content: &str) -> String {
    let hash = content
        .bytes()
        .fold(1469598103934665603u64, |acc, b| acc ^ (b as u64))
        .wrapping_mul(1099511628211);
    format!("{:016x}", hash)
}
