use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResultItem {
    pub title: String,
    pub link: String,
    pub snippet: String,
    pub recency_score: u32,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub query_used: String,
    #[serde(default)]
    pub reliability_score: u32,
    #[serde(default)]
    pub query_match_score: u32,
    #[serde(default)]
    pub cross_check_score: u32,
    #[serde(default)]
    pub final_score: u32,
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

pub fn canonicalize_link(link: &str) -> String {
    let no_hash = link.split('#').next().unwrap_or(link);
    let no_query = no_hash.split('?').next().unwrap_or(no_hash);
    no_query.trim_end_matches('/').to_string()
}

pub fn source_reliability_score(link: &str) -> u32 {
    let lower = link.to_lowercase();
    if lower.contains(".gov") || lower.contains(".edu") {
        90
    } else if lower.contains("wikipedia.org")
        || lower.contains("reuters.com")
        || lower.contains("apnews.com")
        || lower.contains("bbc.com")
    {
        80
    } else {
        50
    }
}

pub fn query_match_score(query: &str, text: &str) -> u32 {
    let q_tokens: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 2)
        .map(|s| s.to_string())
        .collect();
    if q_tokens.is_empty() {
        return 0;
    }
    let hay = text.to_lowercase();
    let mut hits = 0u32;
    for token in &q_tokens {
        if hay.contains(token) {
            hits += 1;
        }
    }
    ((hits as f32 / (q_tokens.len().max(1) as f32)) * 100.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_link_removes_query_and_fragment() {
        let url = "https://example.com/path/to?a=1&b=2#section";
        assert_eq!(canonicalize_link(url), "https://example.com/path/to");
    }

    #[test]
    fn source_reliability_prefers_gov_domains() {
        assert!(
            source_reliability_score("https://nasa.gov/news")
                > source_reliability_score("https://someblog.example/post")
        );
    }

    #[test]
    fn query_match_scores_higher_for_matching_text() {
        let hi = query_match_score(
            "rust async channels",
            "This post explains rust async channels and tokio patterns.",
        );
        let lo = query_match_score(
            "rust async channels",
            "Completely unrelated gardening content.",
        );
        assert!(hi > lo);
    }
}
