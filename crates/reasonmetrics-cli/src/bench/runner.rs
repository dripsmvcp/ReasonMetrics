//! Drive the task set against a model under a bounded rayon pool.

use std::time::Duration;

use rayon::prelude::*;

use crate::bench::model::Model;
use crate::bench::score::Attempt;
use crate::bench::taskset::Task;

fn complete_with_retries(
    model: &dyn Model,
    prompt: &str,
    retries: usize,
) -> Result<crate::bench::model::Completion, String> {
    let mut last = String::new();
    for attempt in 0..=retries {
        match model.complete(prompt) {
            Ok(c) => return Ok(c),
            Err(e) => {
                last = e.to_string();
                if attempt < retries {
                    // Linear backoff; keep it short so a run doesn't stall.
                    std::thread::sleep(Duration::from_millis(250 * (attempt as u64 + 1)));
                }
            }
        }
    }
    Err(last)
}

pub fn run_tasks(
    model: &dyn Model,
    tasks: &[Task],
    concurrency: usize,
    retries: usize,
) -> Vec<Attempt> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency.max(1))
        .build()
        .expect("failed to build bench thread pool");

    pool.install(|| {
        tasks
            .par_iter()
            .map(|task| Attempt {
                task: task.clone(),
                result: complete_with_retries(model, &task.problem, retries),
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::model::{Completion, MockModel};
    use crate::bench::taskset::Task;

    #[test]
    fn run_tasks_preserves_order_and_records_errors() {
        let tasks = vec![
            Task {
                id: "a".into(),
                problem: "P-A".into(),
                expected_answer: "1".into(),
            },
            Task {
                id: "b".into(),
                problem: "P-B".into(),
                expected_answer: "2".into(),
            },
        ];
        // Only P-A has a canned response; P-B will error out.
        let mock = MockModel::new(vec![(
            "P-A".to_string(),
            Completion {
                text: "<think>..</think> 1".into(),
                completion_tokens: Some(3),
            },
        )]);

        let attempts = run_tasks(&mock, &tasks, 2, 0);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].task.id, "a");
        assert!(attempts[0].result.is_ok());
        assert_eq!(attempts[1].task.id, "b");
        assert!(attempts[1].result.is_err());
    }
}
