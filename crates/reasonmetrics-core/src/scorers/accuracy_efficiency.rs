// Port of the LLMThinkBench "Overthinking Score" (arXiv 2507.04023):
// harmonic mean of answer accuracy and token efficiency. Requires a
// ground-truth `expected_answer` on the trace; without one the scorer is
// neutral (score 100, default weight 0.0) so unlabeled datasets are unaffected.

use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};

// Correct answers never drop below the floor: correctness dominates verbosity.
const EFFICIENCY_FLOOR: f32 = 0.2;

pub struct AccuracyEfficiencyScorer {
    weight: f32,
    token_min: usize,
    token_max: usize,
}

impl AccuracyEfficiencyScorer {
    pub fn new(weight: f32, token_min: usize, token_max: usize) -> Self {
        Self {
            weight,
            token_min,
            token_max: token_max.max(token_min + 1),
        }
    }
}

pub fn normalize_answer(s: &str) -> String {
    s.trim().trim_end_matches(['.', ',']).trim().to_lowercase()
}

pub fn answers_match(answer: &str, expected: &str) -> bool {
    let a = normalize_answer(answer);
    let b = normalize_answer(expected);
    if a == b {
        return true;
    }
    if let (Ok(x), Ok(y)) = (a.parse::<f64>(), b.parse::<f64>()) {
        return (x - y).abs() < 1e-9;
    }
    false
}

impl Scorer for AccuracyEfficiencyScorer {
    fn name(&self) -> &str {
        "accuracy_efficiency"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let Some(expected) = &trace.expected_answer else {
            return ScoreResult::with_diagnostics(
                100.0,
                vec![("skipped".into(), "no_ground_truth".into())],
            );
        };

        let accurate = answers_match(&trace.answer, expected);
        let tokens = estimated_token_count(extracted_thinking);
        let span = (self.token_max - self.token_min) as f32;
        let over = tokens.saturating_sub(self.token_min) as f32 / span;
        let efficiency = (1.0 - over).clamp(EFFICIENCY_FLOOR, 1.0);

        // Harmonic mean of accuracy ∈ {0,1} and efficiency ∈ [floor,1]:
        // wrong answers score 0 outright, correct ones scale with concision.
        let score = if accurate {
            100.0 * (2.0 * efficiency) / (1.0 + efficiency)
        } else {
            0.0
        };

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("skipped".into(), "false".into()),
                ("accurate".into(), accurate.to_string()),
                ("token_efficiency".into(), format!("{efficiency:.3}")),
                ("thinking_tokens".into(), tokens.to_string()),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_trace(thinking: &str, answer: &str, expected: Option<&str>) -> TraceRecord {
        TraceRecord {
            id: "t".into(),
            problem: "What is 17 + 26?".into(),
            thinking: thinking.into(),
            answer: answer.into(),
            domain: None,
            source: None,
            expected_answer: expected.map(String::from),
            extra: HashMap::new(),
        }
    }

    fn scorer() -> AccuracyEfficiencyScorer {
        AccuracyEfficiencyScorer::new(0.0, 50, 5000)
    }

    #[test]
    fn correct_and_short_scores_high() {
        let t = make_trace("17 + 26 = 43.", "43", Some("43"));
        let r = scorer().score(&t, &t.thinking);
        assert!(r.score >= 95.0, "got {}", r.score);
    }

    #[test]
    fn correct_but_bloated_scores_mid() {
        let bloat = "Consider the addition carefully once more. ".repeat(1200);
        let t = make_trace(&bloat, "43", Some("43"));
        let r = scorer().score(&t, &t.thinking);
        assert!(
            r.score > 20.0 && r.score < 60.0,
            "bloated-but-correct should be mid, got {}",
            r.score
        );
    }

    #[test]
    fn wrong_answer_scores_zero_regardless_of_length() {
        let t = make_trace("17 + 26 = 44.", "44", Some("43"));
        let r = scorer().score(&t, &t.thinking);
        assert_eq!(r.score, 0.0);
    }

    #[test]
    fn missing_ground_truth_is_skipped_and_neutral() {
        let t = make_trace("17 + 26 = 43.", "43", None);
        let r = scorer().score(&t, &t.thinking);
        assert_eq!(r.score, 100.0);
        assert!(r
            .diagnostics
            .iter()
            .any(|(k, v)| k == "skipped" && v == "no_ground_truth"));
    }

    #[test]
    fn numeric_answers_normalize() {
        for answer in ["4", "4.0", " 4 ", "4."] {
            let t = make_trace("2+2=4", answer, Some("4"));
            let r = scorer().score(&t, &t.thinking);
            assert!(r.score > 0.0, "answer {answer:?} should match");
        }
    }

    #[test]
    fn text_answers_normalize_case_and_punctuation() {
        let t = make_trace("thinking here", "X = ±2.", Some("x = ±2"));
        let r = scorer().score(&t, &t.thinking);
        assert!(r.score > 0.0);
    }

    #[test]
    fn answers_match_is_public_and_normalizes() {
        // Reachable as a public item, and applies numeric + punctuation/casing rules.
        assert!(super::answers_match("43.", "43"));
        assert!(super::answers_match("4.0", "4"));
        assert!(!super::answers_match("44", "43"));
        assert_eq!(super::normalize_answer(" X = 2. "), "x = 2");
    }
}
