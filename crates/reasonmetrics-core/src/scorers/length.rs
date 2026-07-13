use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};

pub struct LengthScorer {
    weight: f32,
    sweet_spot_min: usize,
    sweet_spot_max: usize,
}

impl LengthScorer {
    pub fn new(weight: f32, sweet_spot_min: usize, sweet_spot_max: usize) -> Self {
        let (sweet_spot_min, sweet_spot_max) = if sweet_spot_min <= sweet_spot_max {
            (sweet_spot_min, sweet_spot_max)
        } else {
            (sweet_spot_max, sweet_spot_min)
        };
        Self {
            weight,
            sweet_spot_min,
            sweet_spot_max,
        }
    }
}

impl Scorer for LengthScorer {
    fn name(&self) -> &str {
        "length_calibration"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let words = estimated_token_count(extracted_thinking);

        let score = if words <= 30 {
            5.0 // Almost empty — definitely bad
        } else if words <= 100 {
            30.0 // Very brief
        } else if words < self.sweet_spot_min {
            60.0 // Below sweet spot
        } else if words <= self.sweet_spot_max {
            100.0 // Sweet spot — configurable via [scoring.length]
        } else if words <= self.sweet_spot_max.saturating_add(2000) {
            90.0 // Acceptable for complex problems
        } else if words <= 10000 {
            70.0 // Getting long
        } else if words <= 20000 {
            40.0 // Very long — likely overthinking
        } else {
            15.0 // Extreme — almost certainly bad
        };

        ScoreResult::with_diagnostics(score, vec![("word_count".into(), words.to_string())])
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
    fn test_sweet_spot_length() {
        let scorer = LengthScorer::new(0.07, 200, 3000);
        let thinking = "word ".repeat(1000); // 1000 words — in sweet spot
        let result = scorer.score(&make_trace(&thinking), &thinking);
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_too_short() {
        let scorer = LengthScorer::new(0.07, 200, 3000);
        let result = scorer.score(&make_trace("yes"), "yes");
        assert!(result.score <= 30.0);
    }

    #[test]
    fn test_too_long() {
        let scorer = LengthScorer::new(0.07, 200, 3000);
        let thinking = "word ".repeat(25000);
        let result = scorer.score(&make_trace(&thinking), &thinking);
        assert!(result.score <= 20.0);
    }

    #[test]
    fn test_inverted_bounds_are_normalized() {
        let scorer = LengthScorer::new(0.07, 3000, 200);
        let thinking = "word ".repeat(1000);
        let result = scorer.score(&make_trace(&thinking), &thinking);
        assert_eq!(result.score, 100.0);
    }
}
