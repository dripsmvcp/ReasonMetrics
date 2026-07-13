use super::Scorer;
use crate::trace::{estimated_token_count, ScoreResult, TraceRecord};

pub struct StructureScorer {
    weight: f32,
}

impl StructureScorer {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }
}

impl Scorer for StructureScorer {
    fn name(&self) -> &str {
        "structural_clarity"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let lower = extracted_thinking.to_lowercase();
        let word_count = estimated_token_count(extracted_thinking);

        if word_count < 10 {
            return ScoreResult::new(50.0);
        }

        // Formal step markers — strongest structural signal
        let strong_markers = [
            "step 1", "step 2", "step 3", "step 4", "step 5", "first,", "second,", "third,",
            "finally,", "1. ", "2. ", "3. ", "4. ", "5. ", "1) ", "2) ", "3) ", "4) ",
        ];
        let strong_count: usize = strong_markers
            .iter()
            .map(|m| lower.matches(m).count())
            .sum();

        // Logical connectors — show reasoning flow between steps
        let medium_markers = [
            "therefore",
            "because",
            "thus",
            "since",
            "given that",
            "it follows",
            "we know that",
            "this means",
            "consequently",
            "as a result",
            "which implies",
            "from this",
            "building on",
        ];
        let medium_count: usize = medium_markers
            .iter()
            .map(|m| lower.matches(m).count())
            .sum();

        // Weak transition markers + conversational metacognitive markers.
        // "Let me think", "Wait", "Okay so" etc. show structured exploration
        // even in informal traces (common in medical/general reasoning).
        let weak_markers = [
            "then",
            "next",
            "so,",
            "also,",
            "now,",
            "finally",
            "let me think",
            "let me consider",
            "wait,",
            "okay,",
            "okay so",
            "hmm,",
            "oh,",
            "oh right",
            "actually,",
            "on the other hand",
            "alternatively,",
            "however,",
            "but wait",
            "so now",
            "moving on",
        ];
        let weak_count: usize = weak_markers.iter().map(|m| lower.matches(m).count()).sum();

        let total_marker_value = (strong_count * 3 + medium_count * 2 + weak_count) as f32;

        // Paragraph detection: count both \n\n (formal) and single \n (informal).
        // Double newlines count as full paragraphs; single newlines as half.
        let double_breaks = extracted_thinking.matches("\n\n").count();
        let single_breaks = extracted_thinking
            .matches('\n')
            .count()
            .saturating_sub(double_breaks * 2);
        let effective_paragraphs = double_breaks as f32 + single_breaks as f32 * 0.5;
        let paragraph_score = (effective_paragraphs * 8.0).min(40.0);

        let marker_density = total_marker_value / (word_count as f32 / 100.0).max(1.0);
        let mut score = (marker_density * 5.0 + paragraph_score).min(100.0);

        // Wall-of-text penalty: long text with no breaks at all
        let has_any_breaks = double_breaks > 0 || single_breaks > 0;
        if !has_any_breaks && word_count > 200 {
            score = score.min(40.0);
        }

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("strong_markers".into(), strong_count.to_string()),
                ("medium_markers".into(), medium_count.to_string()),
                ("weak_markers".into(), weak_count.to_string()),
                (
                    "effective_paragraphs".into(),
                    format!("{:.1}", effective_paragraphs),
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
    fn test_well_structured_trace() {
        let scorer = StructureScorer::new(0.10);
        let thinking = "Step 1: Identify the variables.\n\nStep 2: Set up the equation. Since x + y = 10, we know that y = 10 - x.\n\nStep 3: Substitute. Therefore, 2x + (10 - x) = 15.\n\nFinally, x = 5 and y = 5.";
        let trace = make_trace(thinking);
        let result = scorer.score(&trace, thinking);
        assert!(
            result.score >= 60.0,
            "Structured trace should score well, got {}",
            result.score
        );
    }

    #[test]
    fn test_wall_of_text_penalized() {
        let scorer = StructureScorer::new(0.10);
        // Must be >200 words with NO paragraph breaks (\n\n) to trigger the
        // wall-of-text penalty (score capped at 40). Use .repeat() to exceed 200 words.
        let thinking =
            "I think about this and compute something and get a number and try another thing. "
                .repeat(30);
        let trace = make_trace(&thinking);
        let result = scorer.score(&trace, &thinking);
        assert!(
            result.score <= 40.0,
            "Wall-of-text (>200 words, no paragraphs) should be penalized, got {}",
            result.score
        );
    }
}
