//! Turn model completions into scored, correctness-checked rows.

use serde::Serialize;

use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::accuracy_efficiency::answers_match;
use reasonmetrics_core::scoring::score_traces;
use reasonmetrics_core::trace::{estimated_token_count, extract_thinking, TraceRecord};

use crate::bench::model::Completion;
use crate::bench::taskset::Task;

/// One model attempt at one task: either a completion or an error message.
pub struct Attempt {
    pub task: Task,
    pub result: Result<Completion, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRow {
    pub id: String,
    pub correct: bool,
    pub quality: f32,
    pub tokens: usize,
    pub tokens_estimated: bool,
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

pub fn build_rows(attempts: &[Attempt], scoring: &ScoringConfig) -> Vec<TaskRow> {
    // Build trace records for the successful attempts, remembering their index
    // so results can be re-interleaved with the errored ones in task order.
    let mut records: Vec<TraceRecord> = Vec::new();
    let mut ok_index: Vec<usize> = Vec::new();
    let mut tokens: Vec<(usize, bool)> = Vec::new(); // (count, estimated)

    for (i, att) in attempts.iter().enumerate() {
        if let Ok(c) = &att.result {
            let (thinking, answer) = split_completion(&c.text);
            let (count, estimated) = match c.completion_tokens {
                Some(n) => (n, false),
                None => (
                    estimated_token_count(&thinking) + estimated_token_count(&answer),
                    true,
                ),
            };
            tokens.push((count, estimated));
            ok_index.push(i);
            records.push(TraceRecord {
                id: att.task.id.clone(),
                problem: att.task.problem.clone(),
                thinking,
                answer,
                domain: None,
                source: None,
                expected_answer: Some(att.task.expected_answer.clone()),
                extra: std::collections::HashMap::new(),
            });
        }
    }

    let scored = score_traces(&records, scoring);

    // Assemble rows back in original task order.
    let mut ok_rows: std::collections::HashMap<usize, TaskRow> = std::collections::HashMap::new();
    for (slot, &i) in ok_index.iter().enumerate() {
        let att = &attempts[i];
        let expected = &att.task.expected_answer;
        let (count, estimated) = tokens[slot];
        ok_rows.insert(
            i,
            TaskRow {
                id: att.task.id.clone(),
                correct: answers_match(&records[slot].answer, expected),
                quality: scored[slot].quality_score,
                tokens: count,
                tokens_estimated: estimated,
                error: None,
            },
        );
    }

    attempts
        .iter()
        .enumerate()
        .map(|(i, att)| match &att.result {
            Ok(_) => ok_rows
                .remove(&i)
                .expect("ok row built for every Ok attempt"),
            Err(msg) => TaskRow {
                id: att.task.id.clone(),
                correct: false,
                quality: 0.0,
                tokens: 0,
                tokens_estimated: false,
                error: Some(msg.clone()),
            },
        })
        .collect()
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

    #[test]
    fn build_rows_marks_correct_and_carries_errors() {
        let scoring = ScoringConfig::default();
        let attempts = vec![
            Attempt {
                task: task("a", "4"),
                result: Ok(Completion {
                    text: "<think>2+2=4. verify 4</think> 4".into(),
                    completion_tokens: Some(9),
                }),
            },
            Attempt {
                task: task("b", "4"),
                result: Ok(Completion {
                    text: "<think>hmm</think> 5".into(),
                    completion_tokens: None,
                }),
            },
            Attempt {
                task: task("c", "4"),
                result: Err("timeout".into()),
            },
        ];

        let rows = build_rows(&attempts, &scoring);
        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0].id, "a");
        assert!(rows[0].correct);
        assert_eq!(rows[0].tokens, 9);
        assert!(!rows[0].tokens_estimated);
        assert!(rows[0].error.is_none());

        assert_eq!(rows[1].id, "b");
        assert!(!rows[1].correct); // "5" != "4"
        assert!(rows[1].tokens_estimated); // no usage → estimated
        assert!(rows[1].error.is_none());

        assert_eq!(rows[2].id, "c");
        assert!(!rows[2].correct);
        assert_eq!(rows[2].tokens, 0);
        assert_eq!(rows[2].error.as_deref(), Some("timeout"));
    }
}
