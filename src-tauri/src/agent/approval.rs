use rand::Rng;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::{oneshot, Mutex};

pub type PendingMap = Mutex<HashMap<String, oneshot::Sender<bool>>>;

pub fn pending_approvals() -> &'static PendingMap {
    static MAP: OnceLock<PendingMap> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn request_approval(
    telegram_tool: &crate::tools::telegram_tool::TelegramTool,
    command_preview: &str,
) -> bool {
    let id_str = {
        let mut rng = rand::thread_rng();
        let id: u16 = rng.gen_range(1000..9999);
        id.to_string()
    };

    let (tx, rx) = oneshot::channel();

    {
        let mut map = pending_approvals().lock().await;
        map.insert(id_str.clone(), tx);
    }

    let msg = format!(
        "🚨 *SECURITY ALERT* 🚨\nThe agent is attempting to execute a potentially dangerous system command:\n\n`{}`\n\nTo approve, reply with: `/approve {}`\nTo reject, reply with: `/reject {}`",
        command_preview, id_str, id_str
    );

    let status = telegram_tool.send_message(&msg).await;
    if status.contains("Failed") {
        // If telegram fails to send, we should automatically reject to be safe,
        // and remove it from the map.
        let mut map = pending_approvals().lock().await;
        map.remove(&id_str);
        return false;
    }

    rx.await.unwrap_or_default()
}
