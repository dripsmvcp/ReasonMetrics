use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};
use std::collections::{HashMap, HashSet};

pub struct RepetitionScorer {
    weight: f32,
}

impl RepetitionScorer {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);

        let is_sentence_end = ch == '.'
            || ch == '!'
            || ch == '?'
            || ch == '\u{3002}'
            || ch == '\u{FF01}'
            || ch == '\u{FF1F}';
        if is_sentence_end && current.chars().count() > 10 {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current = String::new();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && estimated_token_count(&trimmed) >= 3 {
        sentences.push(trimmed);
    }
    sentences
}

impl Scorer for RepetitionScorer {
    fn name(&self) -> &str {
        "repetition"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let paragraphs: Vec<String> = extracted_thinking
            .split("\n\n")
            .map(normalize)
            .filter(|p| estimated_token_count(p) >= 5)
            .collect();

        let para_unique_ratio = if paragraphs.is_empty() {
            1.0
        } else {
            let unique: HashSet<_> = paragraphs.iter().collect();
            unique.len() as f32 / paragraphs.len() as f32
        };

        let sentences = split_sentences(extracted_thinking);
        let mut sentence_freq: HashMap<String, usize> = HashMap::new();
        for s in &sentences {
            *sentence_freq.entry(normalize(s)).or_insert(0) += 1;
        }
        let repeated_sentences = sentence_freq.values().filter(|&&count| count >= 3).count();
        let sentence_rep_ratio = if sentences.is_empty() {
            0.0
        } else {
            repeated_sentences as f32 / sentences.len() as f32
        };

        let repetition_ratio = (1.0 - para_unique_ratio).max(sentence_rep_ratio);
        let score = 100.0 * (1.0 - (repetition_ratio * 3.0).min(1.0));

        ScoreResult::with_diagnostics(
            score,
            vec![
                (
                    "paragraph_unique_ratio".into(),
                    format!("{:.2}", para_unique_ratio),
                ),
                (
                    "repeated_sentences_3plus".into(),
                    repeated_sentences.to_string(),
                ),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_no_repetition_scores_high() {
        let scorer = RepetitionScorer::new(0.15);
        let thinking = "First, I analyze the problem.\n\nSecond, I set up equations.\n\nThird, I solve for x.\n\nFinally, I verify the result.";
        let result = scorer.score(&make_trace(thinking), thinking);
        assert!(
            result.score >= 90.0,
            "No repetition should score high, got {}",
            result.score
        );
    }

    #[test]
    fn test_repeated_paragraphs_penalized() {
        let scorer = RepetitionScorer::new(0.15);
        let para = "Let me think about this problem carefully and consider all angles.";
        let thinking = format!(
            "{}\n\n{}\n\n{}\n\nNow the actual solution.",
            para, para, para
        );
        let result = scorer.score(&make_trace(&thinking), &thinking);
        assert!(
            result.score < 60.0,
            "Repeated paragraphs should be penalized, got {}",
            result.score
        );
    }

    #[test]
    fn test_repeated_cjk_paragraphs_penalized() {
        let scorer = RepetitionScorer::new(0.15);
        let para = "逐步分析这个问题并验证答案。";
        let thinking = format!("{}\n\n{}\n\n{}\n\n现在给出最终答案。", para, para, para);
        let result = scorer.score(&make_trace(&thinking), &thinking);
        assert!(
            result.score < 80.0,
            "Repeated CJK paragraphs should still be detected, got {}",
            result.score
        );
    }
}
