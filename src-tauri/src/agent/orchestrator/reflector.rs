use crate::agent::llm_client::{ChatMessage, LLMResult};
use crate::agent::parser::extract_json_blocks;
use tokio::sync::mpsc;

impl super::Orchestrator {
    pub async fn run_reflector_pipeline(
        &self,
        reflector_prompt: &str,
        mut history: Vec<ChatMessage>,
        log_tx: mpsc::Sender<String>,
    ) {
        let _ = log_tx
            .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_start", "args": {}})))
            .await;

        let system_msg = ChatMessage {
            role: "system".to_string(),
            content: reflector_prompt.to_string(),
            images_base64: None,
        };

        // Insert the system prompt at the beginning of the history so the LLM knows it is the Reflector.
        history.insert(0, system_msg);

        // Append a prompt asking it to execute tools now if needed
        history.push(ChatMessage {
            role: "user".to_string(),
            content: "You have reviewed the conversation. If you need to store anything in the brain or schedule a task, output the appropriate JSON tool blocks now. If no memory or scheduling is needed, reply exactly 'NO_MEMORY_NEEDED'.".to_string(), images_base64: None });

        let mut reflector_res_result = self.get_llm_for_reflector().chat(&history, 0.7).await;
        if reflector_res_result.is_err() {
            for fallback_ep in self.get_fallback_endpoints(&self.routing.reflector_id) {
                reflector_res_result = crate::agent::llm_client::LLMClient::new(
                    fallback_ep.api_url.clone(),
                    fallback_ep.model.clone(),
                    fallback_ep.api_key.clone(),
                )
                .chat(&history, 0.7)
                .await;
                if reflector_res_result.is_ok() {
                    break;
                }
            }
        }

        let reflector_res = reflector_res_result.unwrap_or_else(|_| LLMResult {
            content: "NO_MEMORY_NEEDED".to_string(),
            raw: serde_json::Value::Null,
        });

        let ai_text = reflector_res.content.clone();
        if ai_text.trim() == "NO_MEMORY_NEEDED" {
            let _ = log_tx
                .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_no_memory", "args": {}})))
                .await;
            return;
        }

        let tool_calls = extract_json_blocks(&ai_text);
        if tool_calls.is_empty() {
            let _ = log_tx
                .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_no_tools", "args": {}})))
                .await;
            return;
        }

        let _ = log_tx
            .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_executing", "args": {"count": tool_calls.len()}})))
            .await;

        let results = self
            .multi_agent
            .execute_tools(tool_calls, Some(self.get_llm_for_worker()), None)
            .await;

        for r in results {
            if r.ok
                && (r.action == "write" || r.action == "write_artifact")
                && (r.tool_name == "brain" || r.tool_name == "knowledge")
            {
                let _ = log_tx
                    .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_db_saved", "args": {"tool": r.tool_name}})))
                    .await;
            }
        }

        // Optionally, log results
        let _ = log_tx
            .send(format!("i18n:{}", serde_json::json!({"key": "log.reflector_done", "args": {}})))
            .await;
    }
}
