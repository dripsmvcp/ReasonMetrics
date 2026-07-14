// Core data structures for reasonmetrics

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

//we convert the input id to a string, we accept string or number for id
fn deserialize_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected string or number for id, got {other}"
        ))),
    }
}

// TraceRecord - Raw input from JSONL

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    #[serde(
        deserialize_with = "deserialize_id",
        alias = "idx",
        alias = "index",
        alias = "uuid"
    )]
    pub id: String,

    #[serde(alias = "question", alias = "prompt", alias = "query", alias = "input")]
    pub problem: String,

    #[serde(
        alias = "reasoning",
        alias = "chain_of_thought",
        alias = "cot",
        alias = "thought"
    )]
    pub thinking: String,

    #[serde(
        alias = "solution",
        alias = "response",
        alias = "output",
        alias = "result"
    )]
    pub answer: String,

    /// Optional domain (math, code, science)
    #[serde(default)]
    pub domain: Option<String>,

    /// Optional source dataset name
    #[serde(default)]
    pub source: Option<String>,

    /// Optional ground-truth answer; enables the accuracy_efficiency scorer
    #[serde(default, alias = "ground_truth", alias = "label", alias = "target")]
    pub expected_answer: Option<String>,

    /// Catch-all for extra JSON fields we didn't explicitly define
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

//Think-tag extraction

pub fn extract_thinking(raw: &str) -> String {
    // Try <think>...</think> (DeepSeek R1, QwQ)
    let open_tag = "<think>";
    let close_tag = "</think>";
    if let Some(start) = raw.find(open_tag) {
        let content_start = start + open_tag.len();
        if let Some(relative_end) = raw[content_start..].find(close_tag) {
            let end = content_start + relative_end;
            return raw[content_start..end].trim().to_string();
        }
    }

    //Try <reasoning>...</reasoning>

    let open_tag = "<reasoning>";
    let close_tag = "</reasoning>";
    if let Some(start) = raw.find(open_tag) {
        let content_start = start + open_tag.len();
        if let Some(relative_end) = raw[content_start..].find(close_tag) {
            let end = content_start + relative_end;
            return raw[content_start..end].trim().to_string();
        }
    }

    //Try <thought>...</thought>

    let open_tag = "<thought>";
    let close_tag = "</thought>";
    if let Some(start) = raw.find(open_tag) {
        let content_start = start + open_tag.len();
        if let Some(relative_end) = raw[content_start..].find(close_tag) {
            let end = content_start + relative_end;
            return raw[content_start..end].trim().to_string();
        }
    }

    // No tags — return raw text trimmed

    raw.trim().to_string()
}

/// Find the largest valid char boundary at or before `index`.
/// Used by scorers that slice into specific percentages of text.
pub fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut i = index.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

//detect common CJK scripts that often omit spaces between words

fn has_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '\u{3400}'..='\u{4DBF}'  // CJK Extension A
            | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs
            | '\u{3040}'..='\u{30FF}' // Hiragana + Katakana
            | '\u{AC00}'..='\u{D7AF}' // Hangul
        )
    })
}

pub fn estimated_token_count(text: &str) -> usize {
    if text.trim().is_empty() {
        return 0;
    }

    let whitespace_words = text.split_whitespace().count();
    if whitespace_words >= 2 || !has_cjk(text) {
        return whitespace_words.max(1);
    }

    let non_whitespace_chars = text.chars().filter(|c| !c.is_whitespace()).count();
    (non_whitespace_chars / 2).max(1)
}

//ScoreResult - What each scorer returns

#[derive(Debug, Clone)]
pub struct ScoreResult {
    /// 0.0 to 100.0 (clamped automatically)
    pub score: f32,
    /// Key-value pairs explaining the score
    pub diagnostics: Vec<(String, String)>,
}

impl ScoreResult {
    ///Create with just a score (no diagnostics)
    pub fn new(score: f32) -> Self {
        Self {
            score: score.clamp(0.0, 100.0),
            diagnostics: Vec::new(),
        }
    }
    ///Create with score + diagnostics
    pub fn with_diagnostics(score: f32, diagnostics: Vec<(String, String)>) -> Self {
        Self {
            score: score.clamp(0.0, 100.0),
            diagnostics,
        }
    }
}

//ScoredTrace - Output data (trace + all scores)
// A trace with all 8 quality scores computed. Written to Parquet/JSONL.

#[derive(Debug, Clone, Serialize)]
pub struct ScoredTrace {
    pub id: String,
    pub problem: String,
    pub thinking: String,
    pub answer: String,

    /// Percentile against a reference corpus of real reasoning traces (0-100):
    /// "better than N% of real traces". This is the number to filter and rank on.
    ///
    /// It is `raw_score` mapped through [`crate::calibration::calibrate`]. The
    /// mapping is monotone, so ranking is identical to the raw composite — but
    /// the raw scale was crushed (99.9% of real traces above 70), which made
    /// `--min-score` useless and the scorecard misleading. See issue #30.
    pub quality_score: f32,

    /// The underlying weighted average of all 8 dimension scores (0-100), before
    /// calibration. Kept for transparency and for refitting the curve.
    pub raw_score: f32,

    //Individual scores (0-100 each). NOT calibrated: these are diagnostics, and
    //most are saturated (language_score is exactly 100 for 98.1% of real traces),
    //so a percentile of them would be meaningless.
    pub efficiency_score: f32,
    pub language_score: f32,
    pub answer_alignment_score: f32,
    pub structural_score: f32,
    pub repetition_score: f32,
    pub overthinking_score: f32,
    pub verification_score: f32,
    pub length_score: f32,

    //Diagnostics
    pub thinking_word_count: u32, // Estimated word/token count
    pub restart_count: u32,
    pub detected_language: String,
    pub has_self_verification: bool,
    pub is_language_mixed: bool,
    pub answer_in_trace_end: bool,
}

//Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_think_tags() {
        assert_eq!(
            extract_thinking("<think>Step by step.</think>"),
            "Step by step."
        );
    }

    #[test]
    fn test_extract_reasoning_tags() {
        assert_eq!(
            extract_thinking("<reasoning>Base case first.</reasoning>"),
            "Base case first."
        );
    }

    #[test]
    fn test_extract_plain_text() {
        assert_eq!(extract_thinking("Just text."), "Just text.");
    }

    #[test]
    fn test_extract_trims_whitespace() {
        assert_eq!(extract_thinking("  \n  hello  \n  "), "hello");
    }

    #[test]
    fn test_deserialization_with_aliases() {
        let json = r#"{"id":"1","question":"Q","reasoning":"R","solution":"A"}"#;
        let t: TraceRecord = serde_json::from_str(json).unwrap();
        assert_eq!(t.problem, "Q");
        assert_eq!(t.thinking, "R");
        assert_eq!(t.answer, "A");
    }

    #[test]
    fn test_score_result_clamps() {
        assert_eq!(ScoreResult::new(150.0).score, 100.0);
        assert_eq!(ScoreResult::new(-50.0).score, 0.0);
    }

    #[test]
    fn test_extract_malformed_tag_order() {
        // Edge case: closing tag before opening tag should not panic.
        // The code searches for </think> only AFTER the <think> position,
        // so the stray </think> at the start is ignored.
        let result = extract_thinking("</think>some text<think>content</think>");
        assert_eq!(result, "content");
    }

    #[test]
    fn test_extract_unclosed_tag() {
        // Edge case: opening tag without closing tag falls through to plain text
        let result = extract_thinking("<think>no closing tag here");
        assert_eq!(result, "<think>no closing tag here");
    }

    #[test]
    fn test_estimated_token_count_english() {
        assert_eq!(estimated_token_count("one two three"), 3);
    }

    #[test]
    fn test_estimated_token_count_cjk() {
        // No spaces, but still clearly not a 1-word trace.
        assert!(estimated_token_count("逐步分析并验证答案是否正确。") >= 4);
    }

    #[test]
    fn test_estimated_token_count_blank() {
        assert_eq!(estimated_token_count("   \n  "), 0);
    }

    #[test]
    fn test_deserialization_with_numeric_id() {
        let json = r#"{"idx":123,"problem":"Q","thinking":"R","answer":"A"}"#;
        let t: TraceRecord = serde_json::from_str(json).unwrap();
        assert_eq!(t.id, "123");
    }
}
