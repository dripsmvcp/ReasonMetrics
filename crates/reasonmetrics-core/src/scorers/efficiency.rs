use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};
use std::sync::LazyLock;

//compiled patterns
pub(crate) static RESTART_REGEXES: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
    vec![
        r"(?i)\bwait,?\s+(let me|i need to|actually)",
        r"(?i)\blet me (reconsider|restart|start over|try again|redo|rethink)",
        r"(?i)\bactually,?\s+(no|wait|let me|that's wrong)",
        r"(?i)\b(hmm+|ugh),?\s+let me",
        r"(?i)\bi (made|see) an error",
        r"(?i)\bthat('s| is) (wrong|incorrect|not right),?\s+(let|i)",
        r"(?i)\bon second thought",
        r"(?i)\blet me approach this differently",
        r"(?i)\bi need to restart",
        r"(?i)\bno wait,",
        r"(?i)\bactually i was wrong",
        r"(?i)\blet me recalculate",
    ]
    .into_iter()
    .map(|p| regex::Regex::new(p).unwrap())
    .collect()
});

pub struct EfficiencyScorer {
    weight: f32,
    penalty_per_1k: f32,
}

impl EfficiencyScorer {
    pub fn new(weight: f32, penalty_per_1k: f32) -> Self {
        Self {
            weight,
            penalty_per_1k: penalty_per_1k.max(0.0),
        }
    }
}

impl Scorer for EfficiencyScorer {
    fn name(&self) -> &str {
        "efficiency"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let restart_count: usize = RESTART_REGEXES
            .iter()
            .map(|re| re.find_iter(extracted_thinking).count())
            .sum();
        let word_count = estimated_token_count(extracted_thinking);
        let restart_density = restart_count as f32 / (word_count as f32 / 1000.0).max(1.0);
        let score = (100.0 - restart_density * self.penalty_per_1k).max(0.0);

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("restart_count".into(), restart_count.to_string()),
                ("word_count".into(), word_count.to_string()),
                ("restart_density".into(), format!("{:.2}", restart_density)),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceRecord;
    use std::collections::HashMap;

    /// Helper to create a minimal TraceRecord for testing
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
    fn test_clean_trace_scores_high() {
        let scorer = EfficiencyScorer::new(0.20, 8.0);
        let trace = make_trace("First, let me compute the derivative. Then I apply the chain rule. Therefore the answer is 42.");
        let result = scorer.score(&trace, &trace.thinking);
        assert!(
            result.score >= 90.0,
            "Clean trace should score high, got {}",
            result.score
        );
    }

    #[test]
    fn test_restart_heavy_trace_scores_low() {
        let scorer = EfficiencyScorer::new(0.20, 8.0);
        let thinking = "Let me try this. Wait, let me reconsider. Actually, no that's wrong. Let me start over. Hmm, let me try again. Actually I was wrong. Let me recalculate.";
        let trace = make_trace(thinking);
        let result = scorer.score(&trace, &trace.thinking);
        assert!(
            result.score < 50.0,
            "Restart-heavy trace should score low, got {}",
            result.score
        );
    }

    #[test]
    fn test_long_trace_with_few_restarts_ok() {
        let scorer = EfficiencyScorer::new(0.20, 8.0);
        // 1 restart in ~100 words = acceptable
        let mut thinking = "Step one: we analyze the equation. ".repeat(25);
        thinking.push_str("Wait, let me reconsider this approach. ");
        thinking.push_str(&"Continuing with the corrected method. ".repeat(5));
        let trace = make_trace(&thinking);
        let result = scorer.score(&trace, &trace.thinking);
        assert!(
            result.score >= 70.0,
            "One restart in long trace should be OK, got {}",
            result.score
        );
    }

    #[test]
    fn test_negative_penalty_is_sanitized() {
        let scorer = EfficiencyScorer::new(0.20, -5.0);
        assert_eq!(scorer.penalty_per_1k, 0.0);
    }
}
