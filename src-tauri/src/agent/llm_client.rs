use rquest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub images_base64: Option<Vec<String>>,
}

#[derive(Serialize)]
struct ChatPayload<'a> {
    model: &'a str,
    temperature: f32,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug)]
pub struct LLMResult {
    pub content: String,
    pub raw: Value,
    pub native_tool_calls: Vec<Value>,
}

#[derive(Clone)]
pub struct LLMClient {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
    tools: Option<Vec<serde_json::Value>>,
}

impl LLMClient {
    pub fn new(base_url: String, model: String, api_key: String) -> Self {
        LLMClient {
            client: Client::builder()
                .timeout(Duration::from_secs(300)) // 5 minutes (sufficient for heavy local prompts without endless hanging)
                .build()
                .unwrap_or_default(),
            base_url,
            model,
            api_key,
            tools: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        temperature: f32,
    ) -> Result<LLMResult, Box<dyn Error + Send + Sync>> {
        let json_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                if let Some(images) = &m.images_base64 {
                    if !images.is_empty() {
                        let mut content_arr = vec![serde_json::json!({
                            "type": "text",
                            "text": m.content
                        })];
                        for b64 in images {
                            content_arr.push(serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:image/jpeg;base64,{}", b64)
                                }
                            }));
                        }
                        return serde_json::json!({
                            "role": m.role.clone(),
                            "content": content_arr
                        });
                    }
                }
                // Standard text message
                serde_json::json!({
                    "role": m.role.clone(),
                    "content": m.content.clone()
                })
            })
            .collect();

        let payload = ChatPayload {
            model: &self.model,
            temperature,
            messages: json_messages,
            tools: self.tools.clone(),
        };

        let mut retries = 0;
        let max_retries = 3;
        let mut base_delay = Duration::from_secs(2);

        let response = loop {
            let mut req = self
                .client
                .post(&self.base_url)
                .header("Content-Type", "application/json; charset=utf-8")
                .header(
                    "HTTP-Referer",
                    "https://github.com/bytelogiccore-spec/pumAgent",
                )
                .header("X-Title", "PumAgent");

            if !self.api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", self.api_key));
            }

            let response = req.json(&payload).send().await?;

            if response.status().is_success() {
                break response;
            } else if response.status().as_u16() == 429 || response.status().is_server_error() {
                if retries >= max_retries {
                    return Err(format!(
                        "i18n:{}",
                        serde_json::json!({
                            "key": "err.http_max_retries",
                            "args": {
                                "max_retries": max_retries,
                                "status": response.status().as_u16()
                            }
                        })
                    ).into());
                }
                retries += 1;
                log::warn!(
                    "LLM API rate limited (429) or Server Error. Retrying {}/{} in {}s...",
                    retries,
                    max_retries,
                    base_delay.as_secs()
                );
                tokio::time::sleep(base_delay).await;
                base_delay *= 2; // Exponential backoff
                continue;
            } else {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();

                if text.contains("image input is not supported") || text.contains("mmproj") {
                    return Err(format!(
                        "i18n:{}",
                        serde_json::json!({
                            "key": "err.vision_not_supported",
                            "args": {}
                        })
                    ).into());
                }

                return Err(format!(
                    "i18n:{}",
                    serde_json::json!({
                        "key": "err.http_error",
                        "args": {
                            "status": status.as_u16(),
                            "text": text
                        }
                    })
                ).into());
            }
        };

        let raw_json: Value = response.json().await?;

        let msg_obj = &raw_json["choices"][0]["message"];
        let mut content = msg_obj["content"].as_str().unwrap_or_default().to_string();

        if let Some(reasoning) = msg_obj.get("reasoning_content").and_then(|v| v.as_str()) {
            let prefix: String = reasoning.chars().take(20).collect();
            if !reasoning.is_empty() && !content.contains(&prefix) {
                content = format!("<think>\n{}\n</think>\n\n{}", reasoning, content);
            }
        }

        let mut native_tool_calls = vec![];
        if let Some(tool_calls) = msg_obj.get("tool_calls").and_then(|t| t.as_array()) {
            for call in tool_calls {
                if let Some(func) = call.get("function") {
                    let tool_name = func.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                    let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                    if let Ok(mut parsed_args) = serde_json::from_str::<serde_json::Value>(args_str) {
                        if let Some(obj) = parsed_args.as_object_mut() {
                            obj.insert("tool".to_string(), serde_json::json!(tool_name));
                            native_tool_calls.push(parsed_args);
                        }
                    }
                }
            }
        }

        Ok(LLMResult {
            content,
            raw: raw_json,
            native_tool_calls,
        })
    }
}
