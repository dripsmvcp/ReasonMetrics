use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};
use std::collections::HashMap;
use whichlang::detect_language;

pub struct LanguageScorer {
    weight: f32,
    num_chunks: usize,
    min_words_per_chunk: usize,
}

impl LanguageScorer {
    pub fn new(weight: f32, num_chunks: usize, min_words_per_chunk: usize) -> Self {
        Self {
            weight,
            num_chunks: num_chunks.max(1),
            min_words_per_chunk: min_words_per_chunk.max(1),
        }
    }
}

fn strip_code_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut in_code_block = false;

    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

impl Scorer for LanguageScorer {
    fn name(&self) -> &str {
        "language_consistency"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let clean_text = strip_code_blocks(extracted_thinking);
        let chars: Vec<char> = clean_text.chars().collect();
        let total = chars.len();

        if total < 100 {
            return ScoreResult::with_diagnostics(
                100.0,
                vec![
                    ("detected_language".into(), "unknown_short".into()),
                    ("is_mixed".into(), "false".into()),
                ],
            );
        }

        let num_chunks = self.num_chunks.min(total).max(1);
        let chunk_size = total / num_chunks;

        let mut lang_counts: HashMap<String, usize> = HashMap::new();
        let mut valid_chunks = 0usize;

        for i in 0..num_chunks {
            let start = i * chunk_size;
            let end = if i + 1 == num_chunks {
                total
            } else {
                ((i + 1) * chunk_size).min(total)
            };
            let chunk: String = chars[start..end].iter().collect();
            if estimated_token_count(&chunk) < self.min_words_per_chunk {
                continue;
            }
            let lang = format!("{:?}", detect_language(&chunk));
            *lang_counts.entry(lang).or_insert(0) += 1;
            valid_chunks += 1;
        }

        if valid_chunks == 0 {
            return ScoreResult::with_diagnostics(
                100.0,
                vec![
                    ("detected_language".into(), "unknown_no_valid_chunks".into()),
                    ("is_mixed".into(), "false".into()),
                ],
            );
        }

        let (dominant_lang, dominant_count) = lang_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(lang, count)| (lang.clone(), *count))
            .unwrap_or(("unknown".into(), 0));

        let inconsistent = valid_chunks - dominant_count;
        let is_mixed = inconsistent > 1;

        let score = if inconsistent <= 1 {
            100.0
        } else {
            (100.0 - (inconsistent as f32 - 1.0) * 15.0).max(0.0)
        };

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("detected_language".into(), dominant_lang),
                ("inconsistent_chunks".into(), inconsistent.to_string()),
                ("is_mixed".into(), is_mixed.to_string()),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceRecord;
    use std::collections::HashMap;

    fn make_trace(thinking: &str) -> TraceRecord {
        TraceRecord {
            id: "test".into(),
            problem: "test".into(),
            thinking: thinking.into(),
            answer: "test".into(),
            domain: None,
            source: None,
            expected_answer: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_english_only_scores_high() {
        let scorer = LanguageScorer::new(0.12, 10, 20);
        let text = "Let me solve this problem step by step. First, I need to find the derivative of the function. The chain rule tells us that we should multiply the outer derivative by the inner derivative. Therefore, applying the chain rule gives us the result. Let me verify this by substituting back into the original equation. The computation confirms our answer is correct. This is a well-known result in calculus. ";
        let text = text.repeat(3); // Make it long enough for 10 chunks
        let trace = make_trace(&text);
        let result = scorer.score(&trace, &text);
        assert!(
            result.score >= 80.0,
            "English-only should score high, got {}",
            result.score
        );
    }

    #[test]
    fn test_short_text_defaults_to_100() {
        let scorer = LanguageScorer::new(0.12, 10, 20);
        let trace = make_trace("Short text.");
        let result = scorer.score(&trace, "Short text.");
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_strip_code_blocks() {
        let text = "Some text\n```python\ndef hello():\n    pass\n```\nMore text";
        let stripped = strip_code_blocks(text);
        assert!(!stripped.contains("def hello"));
        assert!(stripped.contains("Some text"));
        assert!(stripped.contains("More text"));
    }

    #[test]
    fn test_cjk_text_is_not_treated_as_empty() {
        let scorer = LanguageScorer::new(0.12, 4, 6);
        let text = "让我们逐步分析这个问题并验证最终答案是否正确。".repeat(12);
        let trace = make_trace(&text);
        let result = scorer.score(&trace, &text);
        let detected = result
            .diagnostics
            .iter()
            .find(|(k, _)| k == "detected_language")
            .map(|(_, v)| v.as_str())
            .unwrap_or("missing");
        assert_ne!(detected, "unknown_no_valid_chunks");
    }

    #[test]
    fn test_zero_chunk_config_is_sanitized() {
        let scorer = LanguageScorer::new(0.12, 0, 0);
        let text = "Let me solve this step by step and verify the result carefully. ".repeat(8);
        let trace = make_trace(&text);
        let result = scorer.score(&trace, &text);
        assert!(result.score >= 80.0);
    }
}
