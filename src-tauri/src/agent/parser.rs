use serde_json::Value;

/// Extracts all JSON code blocks or native tool_call tags from a text
pub fn extract_json_blocks(markdown: &str) -> Vec<Value> {
    let mut results = Vec::new();
    let mut start_idx = 0;

    let marker = "```json";
    let end_marker = "```";

    // 1. Standard markdown ```json ... ``` extraction
    while let Some(start) = markdown[start_idx..].find(marker) {
        let abs_start = start_idx + start + marker.len();

        if let Some(end) = markdown[abs_start..].find(end_marker) {
            let json_str = &markdown[abs_start..abs_start + end];

            if let Ok(parsed) = serde_json::from_str::<Value>(json_str.trim()) {
                results.push(parsed);
            }
            start_idx = abs_start + end + end_marker.len();
        } else if let Some(end) = markdown[abs_start..].rfind('}') {
            let json_str = &markdown[abs_start..abs_start + end + 1];
            if let Ok(parsed) = serde_json::from_str::<Value>(json_str.trim()) {
                results.push(parsed);
            }
            break;
        } else {
            break;
        }
    }

    // 2. Fallback for Gemma/Command R native tag formatting
    // E.g. `<|tool_call>call:search{action: "query", args: { "query": "..." }}<tool_call|>`
    let gemma_marker = "<|tool_call>call:";
    let mut g_idx = 0;
    while let Some(start) = markdown[g_idx..].find(gemma_marker) {
        let abs_start = g_idx + start + gemma_marker.len();

        // Find closing tag `<tool_call|>` or `<|tool_call>`
        let end_len;
        let end_pos;
        if let Some(pos) = markdown[abs_start..].find("<tool_call|>") {
            end_pos = pos;
            end_len = "<tool_call|>".len();
        } else if let Some(pos) = markdown[abs_start..].find("<|tool_call>") {
            end_pos = pos;
            end_len = "<|tool_call>".len();
        } else if let Some(pos) = markdown[abs_start..].rfind('}') {
            end_pos = pos + 1;
            end_len = 0;
            // Removed premature break
        } else {
            break;
        }

        let content = &markdown[abs_start..abs_start + end_pos].trim();
        // content is like: `search{"action": "query", "args": {"query": "..."}}`
        if let Some(brace_idx) = content.find('{') {
            let tool_name = content[..brace_idx]
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');
            let json_args_str = &content[brace_idx..];

            let mut clean_json = json_args_str.to_string();
            clean_json = clean_json.replace("action:", "\"action\":");
            clean_json = clean_json.replace("args:", "\"args\":");

            if let Ok(mut parsed) = serde_json::from_str::<Value>(&clean_json) {
                if let Some(obj) = parsed.as_object_mut() {
                    obj.insert("tool".to_string(), Value::String(tool_name.to_string()));
                }
                results.push(parsed);
            }
        } else {
            // Absolute chaos format: `search:query: "my query"` (no braces)
            let mut map = serde_json::Map::new();
            if content.starts_with("search") {
                map.insert("tool".to_string(), Value::String("search".to_string()));
                map.insert("action".to_string(), Value::String("query".to_string()));
            } else if content.starts_with("knowledge") {
                map.insert("tool".to_string(), Value::String("knowledge".to_string()));
            } else if content.starts_with("brain") {
                map.insert("tool".to_string(), Value::String("brain".to_string()));
            } else if content.starts_with("terminal") {
                map.insert("tool".to_string(), Value::String("terminal".to_string()));
            }

            if let Some(start_quote) = content.find('"') {
                if let Some(end_quote) = content[start_quote + 1..].rfind('"') {
                    let arg_val = &content[start_quote + 1..start_quote + 1 + end_quote];
                    map.insert("query".to_string(), Value::String(arg_val.to_string()));
                    map.insert("fact".to_string(), Value::String(arg_val.to_string()));
                    map.insert("command".to_string(), Value::String(arg_val.to_string()));
                }
            } else if let Some(last_colon) = content.rfind(':') {
                let arg_val = content[last_colon + 1..].trim();
                map.insert("query".to_string(), Value::String(arg_val.to_string()));
                map.insert("fact".to_string(), Value::String(arg_val.to_string()));
                map.insert("command".to_string(), Value::String(arg_val.to_string()));
            }
            results.push(Value::Object(map));
        }
        g_idx = abs_start + end_pos + end_len;
    }
    // 3. Ultimate Fallback: Scavenge for raw JSON objects containing `"tool"`
    if results.is_empty() {
        let chars: Vec<char> = markdown.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                let mut balance = 0;
                let mut end = i;
                for (j, &ch) in chars.iter().enumerate().skip(i) {
                    if ch == '{' {
                        balance += 1;
                    }
                    if ch == '}' {
                        balance -= 1;
                    }
                    if balance == 0 {
                        end = j;
                        break;
                    }
                }
                if balance == 0 && end > i {
                    let json_str: String = chars[i..=end].iter().collect();
                    if json_str.contains("\"tool\"") {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&json_str) {
                            if parsed.get("tool").is_some() {
                                results.push(parsed);
                            }
                        }
                    }
                    i = end; // Skip to the end of this block
                }
            }
            i += 1;
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_blocks() {
        let text = r#"<|tool_call>call:search { "action": "query", "args": { "query": "BTS 최신 정보", "time_range": "m" } }"#;
        let blocks = extract_json_blocks(text);
        assert_eq!(blocks.len(), 1);
    }
}
