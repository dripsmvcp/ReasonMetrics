use super::Scorer;
use crate::trace::{floor_char_boundary, ScoreResult, TraceRecord};

pub struct VerificationScorer {
    weight: f32,
}

impl VerificationScorer {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }
}

// Explicit verification: model deliberately checks its own work.
// Weighted higher because it shows intentional self-correction behavior.
// (Per LIMO: verification phrases correlate with correct answers.)
pub(crate) const EXPLICIT_VERIFICATION: &[&str] = &[
    "let me verify",
    "let me check",
    "let me confirm",
    "let me double-check",
    "let me validate",
    "checking:",
    "to verify",
    "we can check",
    "substituting back",
    "plugging back in",
    "does this satisfy",
    "sanity check",
    "cross-checking",
    "let's verify",
    "let's check",
    "to confirm this",
    "we can verify",
    "i should check",
    "i should verify",
    // CJK — explicit
    "验证一下",
    "检查一下",
    "代回去",
    "重新检查",
    "確認すると",
    "検算",
    "代入して確かめる",
    "확인해보면",
    "검산",
    "대입해보면",
];

// Implicit verification: model shows its work is consistent without
// explicitly saying "let me verify." Common in medical/general reasoning.
// Weighted lower but still valuable — shows the model cross-references.
// (Per the Evaluating Step-by-step Reasoning survey: coherence checks
//  between steps are a form of implicit verification.)
const IMPLICIT_VERIFICATION: &[&str] = &[
    "indeed",
    "as expected",
    "this confirms",
    "which confirms",
    "consistent with",
    "this is consistent",
    "this matches",
    "this aligns with",
    "this agrees with",
    "this supports",
    "as we expected",
    "which makes sense",
    "this makes sense",
    "that checks out",
    "everything fits",
    "fits together",
    "this explains",
    "which explains",
    // Medical/clinical implicit verification
    "the findings support",
    "the symptoms are consistent",
    "this is supported by",
    "the clinical picture",
    "the presentation is consistent",
    "the evidence suggests",
    "the data confirms",
    // Code implicit verification
    "output matches",
    "test passes",
    "returns the expected",
    "gives the correct",
    "produces the right",
    "running this gives",
    // CJK — implicit
    "满足条件",
    "確かめると",
    "조건을 만족",
    "果然",
    "正好",
    "确实",
    "的确",
    "やはり",
    "確かに",
    "역시",
    "맞다",
];

impl Scorer for VerificationScorer {
    fn name(&self) -> &str {
        "self_verification"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, _trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let lower = extracted_thinking.to_lowercase();
        let char_count = lower.chars().count();
        let len = lower.len();

        if char_count < 50 {
            let explicit: usize = EXPLICIT_VERIFICATION
                .iter()
                .map(|p| lower.matches(p).count())
                .sum();
            let implicit: usize = IMPLICIT_VERIFICATION
                .iter()
                .map(|p| lower.matches(p).count())
                .sum();
            let has_verification = explicit > 0 || implicit > 0;
            let score = if explicit > 0 {
                80.0
            } else if implicit > 0 {
                70.0
            } else {
                50.0
            };

            return ScoreResult::with_diagnostics(
                score,
                vec![
                    ("late_explicit".into(), explicit.to_string()),
                    ("late_implicit".into(), implicit.to_string()),
                    ("has_verification".into(), has_verification.to_string()),
                ],
            );
        }

        let last_40_start = floor_char_boundary(&lower, len.saturating_sub(len * 40 / 100));
        let last_40 = &lower[last_40_start..];

        let mid_start = floor_char_boundary(&lower, len / 4);
        let mid_end = floor_char_boundary(&lower, (len * 3) / 4);
        let middle_50 = &lower[mid_start..mid_end];

        let late_explicit: usize = EXPLICIT_VERIFICATION
            .iter()
            .map(|p| last_40.matches(p).count())
            .sum();
        let late_implicit: usize = IMPLICIT_VERIFICATION
            .iter()
            .map(|p| last_40.matches(p).count())
            .sum();
        let mid_explicit: usize = EXPLICIT_VERIFICATION
            .iter()
            .map(|p| middle_50.matches(p).count())
            .sum();
        let mid_implicit: usize = IMPLICIT_VERIFICATION
            .iter()
            .map(|p| middle_50.matches(p).count())
            .sum();

        let has_verification =
            late_explicit > 0 || late_implicit > 0 || mid_explicit > 0 || mid_implicit > 0;

        // Scoring: explicit verification is strongest signal, implicit still valuable.
        // Late verification (last 40%) is weighted higher than mid-trace.
        let score = if late_explicit >= 2 {
            100.0
        } else if late_explicit == 1 && late_implicit >= 1 {
            95.0
        } else if late_explicit == 1 {
            85.0
        } else if late_implicit >= 2 {
            80.0
        } else if late_implicit == 1 {
            70.0
        } else if mid_explicit >= 1 {
            65.0
        } else if mid_implicit >= 1 {
            55.0
        } else {
            30.0
        };

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("late_explicit".into(), late_explicit.to_string()),
                ("late_implicit".into(), late_implicit.to_string()),
                ("mid_explicit".into(), mid_explicit.to_string()),
                ("mid_implicit".into(), mid_implicit.to_string()),
                ("has_verification".into(), has_verification.to_string()),
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
    fn test_verification_at_end_scores_high() {
        let scorer = VerificationScorer::new(0.08);
        let thinking = "First, I solve the equation. x = 5. Let me verify by substituting back. Indeed, 2(5) + 3 = 13. This confirms our answer.";
        let result = scorer.score(&make_trace(thinking), thinking);
        assert!(result.score >= 80.0);
    }

    #[test]
    fn test_no_verification_scores_low() {
        let scorer = VerificationScorer::new(0.08);
        let thinking = "I compute the derivative. The derivative is 2x. So the answer is 2x. That seems right to me based on my calculations above.";
        let result = scorer.score(&make_trace(thinking), thinking);
        assert!(result.score <= 40.0);
    }

    #[test]
    fn test_short_trace_with_verification_gets_credit() {
        let scorer = VerificationScorer::new(0.08);
        let thinking = "x = 5. Let me check: 2(5) + 3 = 13.";
        let result = scorer.score(&make_trace(thinking), thinking);
        assert!(result.score >= 80.0);
    }

    #[test]
    fn test_medical_implicit_verification() {
        let scorer = VerificationScorer::new(0.08);
        let thinking = "The patient presents with sudden weakness and DVT after long travel. This is consistent with paradoxical embolism through a PFO. The clinical picture fits together — the findings support a right-to-left shunt allowing venous clot to reach the brain. This explains the neurological symptoms.";
        let result = scorer.score(&make_trace(thinking), thinking);
        assert!(
            result.score >= 60.0,
            "Medical trace with implicit verification should score well, got {}",
            result.score
        );
    }
}
