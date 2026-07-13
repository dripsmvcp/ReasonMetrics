use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};

pub struct OverthinkingScorer {
    weight: f32,
}

impl OverthinkingScorer {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }
}

fn estimate_complexity(problem: &str) -> f32 {
    let word_count = estimated_token_count(problem);
    let problem_lower = problem.to_lowercase();

    let math_symbols = ['∫', '∑', '∏', '√', '∞', '≤', '≥', '≠'];
    let math_keywords = [
        "\\frac", "\\int", "\\sum", "\\lim", "\\sqrt", "\\binom", "^{", "_{",
    ];
    let math_count: usize = math_symbols
        .iter()
        .map(|&s| problem.chars().filter(|&c| c == s).count())
        .sum::<usize>()
        + math_keywords
            .iter()
            .map(|k| problem_lower.matches(k).count())
            .sum::<usize>();

    let code_keywords = [
        "def ",
        "function ",
        "class ",
        "import ",
        "fn ",
        "#include",
        "void ",
    ];
    let code_count: usize = code_keywords
        .iter()
        .map(|k| problem_lower.matches(k).count())
        .sum();

    let word_component = (word_count as f32 / 150.0).min(0.4);
    let math_component = (math_count as f32 * 0.1).min(0.3);
    let code_component = (code_count as f32 * 0.15).min(0.3);

    (word_component + math_component + code_component).min(1.0)
}

impl Scorer for OverthinkingScorer {
    fn name(&self) -> &str {
        "overthinking"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let complexity = estimate_complexity(&trace.problem);
        let trace_words = estimated_token_count(extracted_thinking);

        let expected_max = 500.0 + complexity * 4000.0;
        let ratio = trace_words as f32 / expected_max;

        let score = if ratio <= 1.0 {
            100.0
        } else if ratio <= 1.5 {
            90.0
        } else if ratio <= 2.5 {
            70.0
        } else if ratio <= 4.0 {
            40.0
        } else {
            10.0
        };

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("complexity".into(), format!("{:.2}", complexity)),
                ("trace_words".into(), trace_words.to_string()),
                ("expected_max".into(), format!("{:.0}", expected_max)),
                ("ratio".into(), format!("{:.2}", ratio)),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_trace(problem: &str, thinking: &str) -> TraceRecord {
        TraceRecord {
            id: "test".into(),
            problem: problem.into(),
            thinking: thinking.into(),
            answer: "test".into(),
            domain: None,
            source: None,
            expected_answer: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_simple_problem_short_trace_ok() {
        let scorer = OverthinkingScorer::new(0.10);
        let trace = make_trace("What is 2+3?", "2 plus 3 equals 5.");
        let result = scorer.score(&trace, &trace.thinking);
        assert!(result.score >= 90.0);
    }

    #[test]
    fn test_simple_problem_long_trace_penalized() {
        let scorer = OverthinkingScorer::new(0.10);
        // "Let me consider this carefully. " = 5 words per repeat.
        // 600 repeats = 3000 words. For "What is 2+3?": complexity ≈ 0.03,
        // expected_max ≈ 608, ratio ≈ 4.9 → score = 10. Must be < 50.
        let long = "Let me consider this carefully. ".repeat(600);
        let trace = make_trace("What is 2+3?", &long);
        let result = scorer.score(&trace, &trace.thinking);
        assert!(
            result.score < 50.0,
            "Overthinking simple problem should penalize, got {}",
            result.score
        );
    }

    #[test]
    fn test_complex_problem_long_trace_ok() {
        let scorer = OverthinkingScorer::new(0.10);
        let problem = "Prove that for all positive integers n, \\sum_{k=1}^{n} k^2 = \\frac{n(n+1)(2n+1)}{6}. Use mathematical induction and verify the base case and inductive step.";
        let long = "Let me work through the induction. Base case: n=1, we get 1 = 1(2)(3)/6 = 1. Good. Now assume true for n=k. We need to show it holds for n=k+1. Adding (k+1)^2 to both sides of the inductive hypothesis gives us the sum up to k+1. Expanding the right side and simplifying... ".repeat(10);
        let trace = make_trace(problem, &long);
        let result = scorer.score(&trace, &trace.thinking);
        assert!(
            result.score >= 70.0,
            "Complex problem with long trace should be OK, got {}",
            result.score
        );
    }
}
