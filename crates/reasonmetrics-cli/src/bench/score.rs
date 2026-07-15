//! Turn model completions into scored, correctness-checked rows.

use serde::Serialize;

use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::accuracy_efficiency::answers_match;
use reasonmetrics_core::scoring::score_traces;
use reasonmetrics_core::trace::{estimated_token_count, extract_thinking, TraceRecord};

use crate::bench::model::Completion;
use crate::bench::taskset::Task;

/// One task's `k` sampled attempts. Each sample is a completion or an error.
/// With `k = 1` this is a single attempt; with `k > 1` it drives pass@k.
pub struct TaskAttempts {
    pub task: Task,
    pub samples: Vec<Result<Completion, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    /// pass@k: true if *any* sample answered correctly.
    pub correct: bool,
    /// Mean quality over the task's successful samples.
    pub quality: f32,
    /// Total completion tokens across all of the task's samples — the real cost
    /// of arriving at the answer, so needing many draws counts against it.
    pub tokens: usize,
    pub tokens_estimated: bool,
    /// How many samples were drawn for this task.
    pub samples: usize,
    /// How many of those samples were correct.
    pub samples_correct: usize,
    /// Set only when *every* sample errored; the task is then excluded from scoring.
    pub error: Option<String>,
}

/// Split a raw completion into (thinking, answer). Thinking is the `<think>`…
/// content (via the core extractor); the answer is whatever follows the last
/// `</think>`, or the whole text when there are no tags.
pub fn split_completion(raw: &str) -> (String, String) {
    let thinking = if raw.contains("<think>") {
        extract_thinking(raw)
    } else {
        String::new()
    };
    let answer = match raw.rfind("</think>") {
        Some(idx) => raw[idx + "</think>".len()..].trim().to_string(),
        None => raw.trim().to_string(),
    };
    (thinking, answer)
}

pub fn build_rows(attempts: &[TaskAttempts], scoring: &ScoringConfig) -> Vec<TaskRow> {
    // Flatten every successful sample into a trace record, remembering which
    // task it belongs to so scored results can be regrouped per task.
    let mut records: Vec<TraceRecord> = Vec::new();
    let mut owner: Vec<usize> = Vec::new(); // task index for each record
    let mut rec_tokens: Vec<(usize, bool)> = Vec::new(); // (count, estimated) per record

    for (ti, ta) in attempts.iter().enumerate() {
        for c in ta.samples.iter().flatten() {
            let (thinking, answer) = split_completion(&c.text);
            let (count, estimated) = match c.completion_tokens {
                Some(n) => (n, false),
                None => (
                    estimated_token_count(&thinking) + estimated_token_count(&answer),
                    true,
                ),
            };
            rec_tokens.push((count, estimated));
            owner.push(ti);
            records.push(TraceRecord {
                id: ta.task.id.clone(),
                problem: ta.task.problem.clone(),
                thinking,
                answer,
                domain: None,
                source: None,
                expected_answer: Some(ta.task.expected_answer.clone()),
                extra: std::collections::HashMap::new(),
            });
        }
    }

    let scored = score_traces(&records, scoring);

    // Aggregate the samples of each task into one row, in task order.
    let mut rows = Vec::with_capacity(attempts.len());
    for (ti, ta) in attempts.iter().enumerate() {
        let slots: Vec<usize> = owner
            .iter()
            .enumerate()
            .filter_map(|(s, &o)| (o == ti).then_some(s))
            .collect();
        let total_samples = ta.samples.len();

        if slots.is_empty() {
            // Every sample errored; surface the first error and exclude the task.
            let err = ta
                .samples
                .iter()
                .find_map(|r| r.as_ref().err().cloned())
                .unwrap_or_else(|| "no samples".into());
            rows.push(TaskRow {
                id: ta.task.id.clone(),
                correct: false,
                quality: 0.0,
                tokens: 0,
                tokens_estimated: false,
                samples: total_samples,
                samples_correct: 0,
                error: Some(err),
            });
            continue;
        }

        let mut samples_correct = 0usize;
        let mut quality_sum = 0.0f32;
        let mut token_total = 0usize;
        let mut estimated = false;
        for &s in &slots {
            if answers_match(&records[s].answer, &ta.task.expected_answer) {
                samples_correct += 1;
            }
            quality_sum += scored[s].quality_score;
            token_total += rec_tokens[s].0;
            estimated |= rec_tokens[s].1;
        }
        rows.push(TaskRow {
            id: ta.task.id.clone(),
            correct: samples_correct > 0, // pass@k
            quality: quality_sum / slots.len() as f32,
            tokens: token_total,
            tokens_estimated: estimated,
            samples: total_samples,
            samples_correct,
            error: None,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::model::Completion;
    use crate::bench::taskset::Task;
    use reasonmetrics_core::config::ScoringConfig;

    fn task(id: &str, expected: &str) -> Task {
        Task {
            id: id.into(),
            problem: "What is 2+2?".into(),
            expected_answer: expected.into(),
        }
    }

    #[test]
    fn split_completion_separates_think_and_answer() {
        let (think, ans) = split_completion("<think>2+2=4, check: 4</think> The answer is 4");
        assert_eq!(think, "2+2=4, check: 4");
        assert_eq!(ans, "The answer is 4");
    }

    #[test]
    fn split_completion_without_tags_puts_all_in_answer() {
        let (think, ans) = split_completion("just 4");
        assert_eq!(think, "");
        assert_eq!(ans, "just 4");
    }

    fn single(task: Task, result: Result<Completion, String>) -> TaskAttempts {
        TaskAttempts {
            task,
            samples: vec![result],
        }
    }

    #[test]
    fn build_rows_marks_correct_and_carries_errors() {
        let scoring = ScoringConfig::default();
        let attempts = vec![
            single(
                task("a", "4"),
                Ok(Completion {
                    text: "<think>2+2=4. verify 4</think> 4".into(),
                    completion_tokens: Some(9),
                }),
            ),
            single(
                task("b", "4"),
                Ok(Completion {
                    text: "<think>hmm</think> 5".into(),
                    completion_tokens: None,
                }),
            ),
            single(task("c", "4"), Err("timeout".into())),
        ];

        let rows = build_rows(&attempts, &scoring);
        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0].id, "a");
        assert!(rows[0].correct);
        assert_eq!(rows[0].tokens, 9);
        assert!(!rows[0].tokens_estimated);
        assert_eq!(rows[0].samples, 1);
        assert_eq!(rows[0].samples_correct, 1);
        assert!(rows[0].error.is_none());

        assert_eq!(rows[1].id, "b");
        assert!(!rows[1].correct); // "5" != "4"
        assert!(rows[1].tokens_estimated); // no usage → estimated
        assert_eq!(rows[1].samples_correct, 0);
        assert!(rows[1].error.is_none());

        assert_eq!(rows[2].id, "c");
        assert!(!rows[2].correct);
        assert_eq!(rows[2].tokens, 0);
        assert_eq!(rows[2].samples, 1);
        assert_eq!(rows[2].error.as_deref(), Some("timeout"));
    }

    #[test]
    fn pass_at_k_solves_on_any_correct_sample_and_sums_tokens() {
        let scoring = ScoringConfig::default();
        // Three samples: wrong, right, wrong. pass@3 → solved.
        let attempts = vec![TaskAttempts {
            task: task("a", "4"),
            samples: vec![
                Ok(Completion {
                    text: "<think>maybe</think> 5".into(),
                    completion_tokens: Some(10),
                }),
                Ok(Completion {
                    text: "<think>2+2=4</think> 4".into(),
                    completion_tokens: Some(20),
                }),
                Ok(Completion {
                    text: "<think>no</think> 6".into(),
                    completion_tokens: Some(30),
                }),
            ],
        }];

        let rows = build_rows(&attempts, &scoring);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].correct, "any correct sample solves the task");
        assert_eq!(rows[0].samples, 3);
        assert_eq!(rows[0].samples_correct, 1);
        assert_eq!(rows[0].tokens, 60, "all sample tokens sum into cost");
    }

    #[test]
    fn all_samples_error_marks_task_errored() {
        let scoring = ScoringConfig::default();
        let attempts = vec![TaskAttempts {
            task: task("a", "4"),
            samples: vec![Err("timeout".into()), Err("500".into())],
        }];
        let rows = build_rows(&attempts, &scoring);
        assert_eq!(rows[0].samples, 2);
        assert_eq!(rows[0].error.as_deref(), Some("timeout"));
    }
}
