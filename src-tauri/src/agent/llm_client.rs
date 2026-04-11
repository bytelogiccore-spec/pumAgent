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
}

#[derive(Debug)]
pub struct LLMResult {
    pub content: String,
    pub raw: Value,
}

#[derive(Clone)]
pub struct LLMClient {
    client: Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl LLMClient {
    pub fn new(base_url: String, model: String, api_key: String) -> Self {
        LLMClient {
            client: Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap_or_default(),
            base_url,
            model,
            api_key,
        }
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
        };

        let mut req = self
            .client
            .post(&self.base_url)
            .header("Content-Type", "application/json; charset=utf-8");

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req.json(&payload).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();

            if text.contains("image input is not supported") || text.contains("mmproj") {
                return Err("오류: 현재 연결된 LLM 모델이 이미지(Vision) 분석을 지원하지 않거나, 멀티모달(mmproj) 프로젝터가 로드되지 않았습니다. 사진 첨부를 해제하거나 비전 지원 모델을 사용해주세요.".into());
            }

            return Err(format!("HTTP 오류: {} - {}", status, text).into());
        }

        let raw_json: Value = response.json().await?;

        let msg_obj = &raw_json["choices"][0]["message"];
        let mut content = msg_obj["content"].as_str().unwrap_or_default().to_string();

        if let Some(reasoning) = msg_obj.get("reasoning_content").and_then(|v| v.as_str()) {
            if !reasoning.is_empty()
                && !content.contains(&reasoning[..std::cmp::min(20, reasoning.len())])
            {
                content = format!("<think>\n{}\n</think>\n\n{}", reasoning, content);
            }
        }

        Ok(LLMResult {
            content,
            raw: raw_json,
        })
    }
}
