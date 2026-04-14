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
    let unquoted_keys_re = regex::Regex::new(r"([{,]\s*)([a-zA-Z0-9_]+)(\s*:)").ok();

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
            // Automatically add quotes to unquoted keys like `{action: "list", domain: "rules"}`
            if let Some(re) = &unquoted_keys_re {
                clean_json = re.replace_all(&clean_json, "$1\"$2\"$3").to_string();
            }
            clean_json = clean_json.replace("<|\"|>", "\"");

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
        let mut start_search_idx = 0;
        while let Some(start_mut_rel) = markdown[start_search_idx..].find('{') {
            let start_byte = start_search_idx + start_mut_rel;
            let mut balance = 0;
            let mut end_byte = start_byte;

            for (idx, ch) in markdown[start_byte..].char_indices() {
                if ch == '{' {
                    balance += 1;
                } else if ch == '}' {
                    balance -= 1;
                }
                if balance == 0 {
                    end_byte = start_byte + idx + ch.len_utf8();
                    break;
                }
            }

            if balance == 0 && end_byte > start_byte {
                let json_slice = &markdown[start_byte..end_byte];
                if json_slice.contains("\"tool\"") {
                    if let Ok(parsed) = serde_json::from_str::<Value>(json_slice) {
                        if parsed.get("tool").is_some() {
                            results.push(parsed);
                        }
                    }
                }
                start_search_idx = end_byte; // Skip to the end of this block
            } else {
                start_search_idx = start_byte + 1;
            }
        }
    }

    results
}

/// Aggressively strips thinking, thought, or reasoning blocks from a text string.
/// This is used to clean outputs for both the UI and notification tools.
pub fn strip_thinking_blocks(text: &str) -> String {
    // Aggressive pattern to catch variants like <think>, <think >, <think style="...">, <thinking>, etc.
    // We break it into a few broad patterns to ensure reliability.
    let patterns = vec![
        r"(?is)<[^>]*?(think|thought|thinking|thought_process|reasoning|details)\b[^>]*?>.*?(?:</[^>]*?(think|thought|thinking|thought_process|reasoning|details)\b[^>]*?>|$)",
    ];
    let mut clean_text = text.to_string();
    for p in patterns {
        if let Ok(re) = regex::Regex::new(p) {
            clean_text = re.replace_all(&clean_text, "").to_string();
        }
    }
    clean_text.trim().to_string()
}

/// Extracts follow-up suggestions in the format [SUGGESTION: Action Text]
pub fn extract_suggestions(text: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?i)\[SUGGESTION:\s*([^\]]+)\]").unwrap();
    let mut results = Vec::new();
    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            results.push(m.as_str().trim().to_string());
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

    #[test]
    fn test_gemma_quotes() {
        let text = r#"<|tool_call>call:knowledge{action:<|"|>list<|"|>,domain:<|"|>rules<|"|>}<tool_call|><|tool_response>"#;
        let blocks = extract_json_blocks(text);
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn test_gemma_unquoted_keys() {
        let text = r#"<|tool_call>call:knowledge{action: "list", args: {domain: "rules"}}<tool_call|><|tool_response>"#;
        let blocks = extract_json_blocks(text);
        assert_eq!(blocks.len(), 1);
        let action = blocks[0].get("action").unwrap().as_str().unwrap();
        assert_eq!(action, "list");
    }

    #[test]
    fn test_strip_thinking_blocks() {
        // Multi-tag cleaning
        let input = "<think>A</think><thinking>B</thinking><details>C</details>Hello";
        assert_eq!(strip_thinking_blocks(input), "Hello");

        // Case insensitivity
        let input = "<THINK>Hidden</THINK>Visible";
        assert_eq!(strip_thinking_blocks(input), "Visible");

        // Unclosed tag at end
        let input = "Start <thought_process>Rest is hidden";
        assert_eq!(strip_thinking_blocks(input), "Start");

        // Reasoning tag
        let input = "<reasoning>\nStep 1...\n</reasoning>Done";
        assert_eq!(strip_thinking_blocks(input), "Done");
    }
}
