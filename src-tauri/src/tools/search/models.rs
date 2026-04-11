use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub link: String,
    pub snippet: String,
    pub recency_score: u32,
}

pub fn assign_recency_score(snippet: &str) -> u32 {
    let snippet_lower = snippet.to_lowercase();
    // Simple heuristic: higher score = more recent
    if snippet_lower.contains("분 전") || snippet_lower.contains("mins ago") {
        return 100;
    } else if snippet_lower.contains("시간 전") || snippet_lower.contains("hours ago") {
        return 80;
    } else if snippet_lower.contains("일 전")
        || snippet_lower.contains("days ago")
        || snippet_lower.contains("어제")
    {
        return 50;
    } else if snippet_lower.contains("주 전") || snippet_lower.contains("weeks ago") {
        return 30;
    } else if snippet_lower.contains("2026.") || snippet_lower.contains("2026년") {
        return 60; // Explicitly 2026 mentions are good contextually
    }
    0 // No date or old
}
