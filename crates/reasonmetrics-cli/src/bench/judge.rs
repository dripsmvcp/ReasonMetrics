//! Optional tiered LLM judge.
//!
//! The heuristic composite is confident at the extremes but least reliable in
//! the middle band. Rather than pay an LLM to grade every trace, only the tasks
//! whose heuristic quality lands *inside a band* (default 40–70) are escalated
//! to a judge model. This keeps the judge's cost proportional to the genuinely
//! uncertain traces. The judge is advisory: its score is recorded alongside the
//! heuristic one, never blended into it.

use crate::bench::model::Model;
use crate::bench::score::{split_completion, TaskAttempts, TaskRow};

/// Inclusive heuristic-quality band that gets escalated to the judge.
#[derive(Debug, Clone, Copy)]
pub struct Band {
    pub lo: f32,
    pub hi: f32,
}

impl Default for Band {
    fn default() -> Self {
        Band { lo: 40.0, hi: 70.0 }
    }
}

impl Band {
    pub fn contains(&self, q: f32) -> bool {
        q >= self.lo && q <= self.hi
    }
}

/// The rubric sent to the judge for one trace. Asks for a single parseable line.
pub fn judge_prompt(problem: &str, thinking: &str, answer: &str) -> String {
    format!(
        "You are grading the QUALITY of a reasoning trace — how efficiently and \
         soundly it reasons, not merely whether the final answer is right.\n\n\
         Problem:\n{problem}\n\n\
         Reasoning:\n{thinking}\n\n\
         Final answer:\n{answer}\n\n\
         Rate the reasoning from 0 to 100, where 100 is concise, correct, \
         well-structured reasoning and 0 is rambling, repetitive, circular, or \
         incoherent. Consider redundant restarts and padding as negatives. \
         Reply with exactly one line:\nSCORE: <number>"
    )
}

/// Extract a 0–100 rating from a judge reply. Prefers a number following the
/// word "score"; otherwise the first in-range number anywhere in the text.
pub fn parse_judge_score(text: &str) -> Option<f32> {
    let lower = text.to_ascii_lowercase();
    if let Some(idx) = lower.find("score") {
        if let Some(n) = numbers(&text[idx + "score".len()..])
            .into_iter()
            .find(|n| (0.0..=100.0).contains(n))
        {
            return Some(n);
        }
    }
    numbers(text)
        .into_iter()
        .find(|n| (0.0..=100.0).contains(n))
}

/// Numeric tokens in a string, in order (digits and a single dot).
fn numbers(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || (c == '.' && !cur.contains('.')) {
            cur.push(c);
        } else {
            if let Ok(n) = cur.parse::<f32>() {
                out.push(n);
            }
            cur.clear();
        }
    }
    if let Ok(n) = cur.parse::<f32>() {
        out.push(n);
    }
    out
}

/// What a judging pass did, for the result artifact.
#[derive(Debug, Clone)]
pub struct JudgeReport {
    pub n_in_band: usize,
    pub n_scored: usize,
    pub mean_judge_score: Option<f32>,
}

/// Escalate every in-band, non-errored task to the judge and record its score on
/// the row. `rows` and `attempts` are in the same task order. A judge failure
/// leaves that row's `judge_score` as `None` and does not abort the pass.
pub fn run_judging(
    rows: &mut [TaskRow],
    attempts: &[TaskAttempts],
    judge: &dyn Model,
    band: Band,
) -> JudgeReport {
    let mut n_in_band = 0;
    let mut scored = Vec::new();

    for (row, ta) in rows.iter_mut().zip(attempts) {
        if row.error.is_some() || !band.contains(row.quality) {
            continue;
        }
        // Representative sample: the first successful completion for this task.
        let Some(c) = ta.samples.iter().flatten().next() else {
            continue;
        };
        n_in_band += 1;
        let (thinking, answer) = split_completion(&c.text);
        let prompt = judge_prompt(&ta.task.problem, &thinking, &answer);
        if let Ok(reply) = judge.complete(&prompt) {
            if let Some(js) = parse_judge_score(&reply.text) {
                row.judge_score = Some(js);
                scored.push(js);
            }
        }
    }

    let mean_judge_score = if scored.is_empty() {
        None
    } else {
        Some(scored.iter().sum::<f32>() / scored.len() as f32)
    };
    JudgeReport {
        n_in_band,
        n_scored: scored.len(),
        mean_judge_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::model::{Completion, MockModel};
    use crate::bench::score::TaskAttempts;
    use crate::bench::taskset::Task;

    #[test]
    fn band_is_inclusive() {
        let b = Band::default();
        assert!(b.contains(40.0));
        assert!(b.contains(70.0));
        assert!(b.contains(55.0));
        assert!(!b.contains(39.9));
        assert!(!b.contains(70.1));
    }

    #[test]
    fn parses_score_in_several_shapes() {
        assert_eq!(parse_judge_score("SCORE: 72"), Some(72.0));
        assert_eq!(parse_judge_score("score is 8"), Some(8.0));
        assert_eq!(
            parse_judge_score("I'd rate this SCORE:\n55.5 overall"),
            Some(55.5)
        );
        // No "score" keyword → first in-range number.
        assert_eq!(
            parse_judge_score("The rating is 63 out of 100."),
            Some(63.0)
        );
        assert_eq!(parse_judge_score("no numbers here"), None);
    }

    fn task(id: &str) -> Task {
        Task {
            id: id.into(),
            problem: "What is 2+2?".into(),
            expected_answer: "4".into(),
        }
    }

    fn attempt(id: &str, text: &str) -> TaskAttempts {
        TaskAttempts {
            task: task(id),
            samples: vec![Ok(Completion {
                text: text.into(),
                completion_tokens: Some(10),
            })],
        }
    }

    fn row(id: &str, quality: f32, error: Option<&str>) -> TaskRow {
        TaskRow {
            id: id.into(),
            correct: true,
            quality,
            tokens: 10,
            tokens_estimated: false,
            samples: 1,
            samples_correct: 1,
            judge_score: None,
            error: error.map(String::from),
        }
    }

    #[test]
    fn judges_only_in_band_rows() {
        let attempts = vec![
            attempt("low", "<think>x</think> 4"),
            attempt("mid", "<think>y</think> 4"),
            attempt("high", "<think>z</think> 4"),
        ];
        let mut rows = vec![
            row("low", 20.0, None),  // below band
            row("mid", 55.0, None),  // in band → judged
            row("high", 90.0, None), // above band
        ];
        // The judge returns the same canned reply for any prompt in this set.
        let pairs: Vec<(String, Completion)> = attempts
            .iter()
            .map(|a| {
                let (t, ans) = split_completion(&a.samples[0].as_ref().unwrap().text);
                (
                    judge_prompt(&a.task.problem, &t, &ans),
                    Completion {
                        text: "SCORE: 62".into(),
                        completion_tokens: Some(3),
                    },
                )
            })
            .collect();
        let judge = MockModel::new(pairs);

        let report = run_judging(&mut rows, &attempts, &judge, Band::default());
        assert_eq!(report.n_in_band, 1);
        assert_eq!(report.n_scored, 1);
        assert_eq!(rows[0].judge_score, None);
        assert_eq!(rows[1].judge_score, Some(62.0));
        assert_eq!(rows[2].judge_score, None);
        assert_eq!(report.mean_judge_score, Some(62.0));
    }

    #[test]
    fn errored_rows_are_never_judged() {
        let attempts = vec![attempt("e", "<think>x</think> 4")];
        let mut rows = vec![row("e", 55.0, Some("timeout"))];
        let judge = MockModel::new(vec![]);
        let report = run_judging(&mut rows, &attempts, &judge, Band::default());
        assert_eq!(report.n_in_band, 0);
        assert_eq!(rows[0].judge_score, None);
    }
}
