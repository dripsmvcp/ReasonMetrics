use super::Scorer;
use crate::trace::{floor_char_boundary, ScoreResult, TraceRecord};

/// Extract the tail portion of a lowercased trace for analysis.
/// Uses adaptive window: full text for short traces, last 40% for medium,
/// last 30% for long traces.  Based on structural analysis of LIMO &
/// DeepSeek-R1 traces where answer + verification occupy the final ~30-40%.
fn tail_slice(lower: &str) -> &str {
    let len = lower.len();
    let window = if len < 200 {
        len
    } else if len < 800 {
        // Medium traces: generous 55% tail — reasoning often starts converging
        // past the midpoint, especially in conversational style.
        (len * 55) / 100
    } else if len < 2000 {
        (len * 40) / 100
    } else {
        (len * 30) / 100
    };
    let start = floor_char_boundary(lower, len.saturating_sub(window));
    &lower[start..]
}

/// Extract the very last ~12% of text for anti-divergence checks.
fn final_slice(lower: &str) -> &str {
    let len = lower.len();
    let window = if len < 200 { len } else { (len * 12) / 100 };
    let start = floor_char_boundary(lower, len.saturating_sub(window));
    &lower[start..]
}

pub struct AlignmentScorer {
    weight: f32,
}

impl AlignmentScorer {
    pub fn new(weight: f32) -> Self {
        Self { weight }
    }
}

// --- Signal 1: Convergence language in the tail (0-35) ---
// The trace ends with language that narrows to a definitive conclusion.
// This is the PRIMARY signal — works across all domains and languages.
const CONCLUSION_PHRASES: &[&str] = &[
    // English — formal
    "therefore",
    "thus",
    "hence",
    "in conclusion",
    "we conclude",
    "to summarize",
    "in summary",
    // English — answer-presenting
    "the answer is",
    "the result is",
    "the solution is",
    "final answer",
    "so the answer",
    "this gives us",
    "which gives",
    "we get",
    "we find that",
    "we obtain",
    // English — conversational convergence (medical, general)
    "so overall",
    "putting it all together",
    "all things considered",
    "all in all",
    "this means that",
    "this tells us",
    "this indicates",
    "this suggests",
    "this confirms",
    "this points to",
    "i would conclude",
    "i'd conclude",
    "the diagnosis is",
    "i'd bet",
    "i think that makes sense",
    "makes sense given",
    "makes sense because",
    "fits the bill",
    "fits perfectly",
    "everything fits",
    "clicks into place",
    "it seems like",
    "it sure seems",
    "it looks like",
    "most likely",
    "i believe",
    "i think the answer",
    "yup,",
    "yeah,",
    "so the key",
    "the key takeaway",
    "the bottom line",
    "in short",
    "given all this",
    "with this in mind",
    "considering all",
    // Chinese
    "因此",
    "所以",
    "答案是",
    "最终答案",
    "由此可得",
    "综上所述",
    "总结来说",
    "可以得出",
    "我们得到",
    "由此得出",
    // Japanese
    "したがって",
    "よって",
    "答えは",
    "最終的な答え",
    "以上から",
    "まとめると",
    "結論として",
    // Korean
    "따라서",
    "정답은",
    "최종 답",
    "결론적으로",
    "종합하면",
];

// --- Signal 2: Answer echo — bonus when answer text appears in tail (0-25) ---
// Only fires for matchable answers (≥3 chars, or short numeric/symbolic).

// --- Signal 3: Formatted / delimited answer (0-20) ---
const DELIMITER_PHRASES: &[&str] = &[
    "answer:",
    "solution:",
    "result:",
    "output:",
    "answer =",
    "\\boxed{",
    "```",
    "答案:",
    "答案：",
    "结论:",
    "结论：",
    "答え:",
    "答え：",
    "정답:",
    "정답：",
];

// --- Signal 4: Anti-divergence — penalize ending with uncertainty (0-20) ---
// Good traces converge; bad traces end mid-exploration.
const UNCERTAINTY_PHRASES: &[&str] = &[
    "i'm not sure",
    "i am not sure",
    "let me try another",
    "let me reconsider",
    "wait, maybe",
    "hmm, actually",
    "this doesn't seem right",
    "that can't be right",
    "i need to rethink",
    "let me start over",
    "back to square one",
    "i'm confused",
    "不确定",
    "让我重新考虑",
    "ちょっと分からない",
    "잘 모르겠",
];

impl Scorer for AlignmentScorer {
    fn name(&self) -> &str {
        "answer_alignment"
    }
    fn weight(&self) -> f32 {
        self.weight
    }

    fn score(&self, trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult {
        let lower = extracted_thinking.to_lowercase();
        let tail = tail_slice(&lower);
        let final_part = final_slice(&lower);

        // Signal 1: Convergence language (0-35)
        // Two sub-signals:
        //   a) Phrase matching — specific conclusion/convergence phrases
        //   b) Structural — the trace ends with a declarative statement,
        //      not a question or mid-thought (robust across all domains)
        let conclusion_count = CONCLUSION_PHRASES
            .iter()
            .filter(|p| tail.contains(*p))
            .count();
        // Structural: check if final text ends assertively
        let trimmed_end = final_part.trim_end();
        let ends_declarative = trimmed_end.ends_with('.')
            || trimmed_end.ends_with('!')
            || trimmed_end.ends_with('。')
            || trimmed_end.ends_with('！');
        let ends_with_question = trimmed_end.ends_with('?') || trimmed_end.ends_with('？');
        let structural_bonus: u32 = if ends_declarative && !ends_with_question {
            10
        } else {
            0
        };
        // Phrase: 1 phrase = 15, 2 = 22, 3+ = 25  (diminishing returns)
        let phrase_pts: u32 = match conclusion_count {
            0 => 0,
            1 => 15,
            2 => 22,
            _ => 25,
        };
        let convergence_pts = (phrase_pts + structural_bonus).min(35);

        // Signal 2: Answer echo (0-25)
        let answer_lower = trace.answer.to_lowercase();
        let answer_snippet: String = answer_lower.chars().take(60).collect();
        let answer_char_count = answer_snippet.chars().count();

        let short_numeric = (1..=2).contains(&answer_char_count)
            && answer_snippet.chars().any(|c| c.is_numeric())
            && answer_snippet.chars().all(|c| {
                c.is_numeric()
                    || matches!(c, '.' | '-' | '+' | '/' | '=' | '%' | '±' | '×' | '−' | '÷')
            });
        let short_non_ascii = answer_char_count == 1 && !answer_snippet.is_ascii();
        let can_match = answer_char_count >= 3 || short_numeric || short_non_ascii;
        let echo_pts = if !answer_snippet.is_empty() && can_match && tail.contains(&answer_snippet)
        {
            25u32
        } else {
            0
        };

        // Signal 3: Formatted answer / delimiters (0-20)
        let has_delimiter = DELIMITER_PHRASES.iter().any(|p| tail.contains(p));
        let delimiter_pts = if has_delimiter { 20u32 } else { 0 };

        // Signal 4: Anti-divergence (0-20)
        // Award points when the trace does NOT end with uncertainty.
        let ends_uncertain = UNCERTAINTY_PHRASES.iter().any(|p| final_part.contains(p));
        let antidiv_pts = if ends_uncertain { 0u32 } else { 20 };

        let total = (convergence_pts + echo_pts + delimiter_pts + antidiv_pts).min(100);
        let score = total as f32;
        let answer_in_end = score >= 50.0;

        ScoreResult::with_diagnostics(
            score,
            vec![
                ("answer_in_trace_end".into(), answer_in_end.to_string()),
                ("convergence".into(), convergence_pts.to_string()),
                ("answer_echo".into(), echo_pts.to_string()),
                ("delimiters".into(), delimiter_pts.to_string()),
                ("anti_divergence".into(), antidiv_pts.to_string()),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_trace(thinking: &str, answer: &str) -> TraceRecord {
        TraceRecord {
            id: "test".into(),
            problem: "test".into(),
            thinking: thinking.into(),
            answer: answer.into(),
            domain: None,
            source: None,
            expected_answer: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_math_trace_with_boxed_answer() {
        let scorer = AlignmentScorer::new(0.18);
        let thinking = "Let me work through this. First I set up the equation. Then I solve for x. Therefore, the answer is x = 42. \\boxed{42}";
        let trace = make_trace(thinking, "42");
        let result = scorer.score(&trace, thinking);
        // convergence(therefore)=20 + echo(42)=25 + delimiter(\\boxed)=20 + antidiv=20 = 85
        assert!(
            result.score >= 80.0,
            "Math trace with boxed answer should score very high, got {}",
            result.score
        );
    }

    #[test]
    fn test_medical_conversational_convergence() {
        let scorer = AlignmentScorer::new(0.18);
        // Real-world medical reasoning pattern: no formal answer, conversational conclusion.
        // Medical traces often use phrases like "this points to", "fits the bill",
        // "everything fits", which are convergence signals even without formal answers.
        let thinking = "So we have sudden weakness and a swollen leg after long travel. \
            A clot in the leg could cause stroke symptoms through paradoxical embolism. \
            This points to a PFO. The clinical picture fits the bill perfectly — \
            a right-to-left shunt letting a venous clot reach the brain. \
            Everything fits together. In conclusion, PFO is the diagnosis.";
        let trace = make_trace(thinking, "Patent foramen ovale (PFO)");
        let result = scorer.score(&trace, thinking);
        // convergence(this points to + fits the bill + everything fits + in conclusion)=35 + antidiv=20 = 55
        assert!(
            result.score >= 50.0,
            "Medical conversational trace should score well, got {}",
            result.score
        );
    }

    #[test]
    fn test_divergent_trace_scores_low() {
        let scorer = AlignmentScorer::new(0.18);
        // Trace ends with uncertainty — no convergence, ends confused
        let long_padding = "Let me think about this problem more. ".repeat(20);
        let thinking = format!("{long_padding}I'm not sure about this. Let me try another approach. Wait, maybe I should reconsider the whole thing.");
        let trace = make_trace(&thinking, "42");
        let result = scorer.score(&trace, &thinking);
        // convergence=0 + echo=0 + delimiter=0 + antidiv=0 = 0
        assert!(
            result.score < 20.0,
            "Divergent/uncertain trace should score low, got {}",
            result.score
        );
    }

    #[test]
    fn test_short_numeric_answer_at_end() {
        let scorer = AlignmentScorer::new(0.18);
        let thinking = "I simplify the expression carefully. Therefore, the answer is 42.";
        let trace = make_trace(thinking, "42");
        let result = scorer.score(&trace, thinking);
        // convergence(therefore + the answer is)=28 + echo(42)=25 + antidiv=20 = 73
        assert!(
            result.score >= 50.0,
            "Short numeric answer at end should get credit, got {}",
            result.score
        );
    }

    #[test]
    fn test_no_answer_but_converges() {
        let scorer = AlignmentScorer::new(0.18);
        // Trace converges clearly but answer field doesn't match anything in thinking
        let thinking = "After analyzing all the evidence, we conclude that the mechanism involves quantum tunneling. In summary, the barrier transmission coefficient is non-zero.";
        let trace = make_trace(thinking, "The transmission coefficient T = e^{-2κL}");
        let result = scorer.score(&trace, thinking);
        // convergence(we conclude + in summary)=28 + echo=0 + delimiter=0 + antidiv=20 = 48
        assert!(
            result.score >= 40.0,
            "Converging trace without answer echo should still score reasonably, got {}",
            result.score
        );
    }
}
