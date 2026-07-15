//! Drive the task set against a model under a bounded rayon pool.

use std::time::Duration;

use rayon::prelude::*;

use crate::bench::model::{Completion, Model};
use crate::bench::score::TaskAttempts;
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

/// Run every task `samples` times against the model. Work is parallelized
/// across all (task, sample) pairs so concurrency is used even when there are
/// few tasks; results are regrouped per task in stable sample order.
pub fn run_tasks(
    model: &dyn Model,
    tasks: &[Task],
    concurrency: usize,
    retries: usize,
    samples: usize,
) -> Vec<TaskAttempts> {
    let samples = samples.max(1);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency.max(1))
        .build()
        .expect("failed to build bench thread pool");

    let work: Vec<(usize, usize)> = (0..tasks.len())
        .flat_map(|ti| (0..samples).map(move |si| (ti, si)))
        .collect();

    let mut outcomes: Vec<(usize, usize, Result<Completion, String>)> = pool.install(|| {
        work.par_iter()
            .map(|&(ti, si)| {
                (
                    ti,
                    si,
                    complete_with_retries(model, &tasks[ti].problem, retries),
                )
            })
            .collect()
    });

    outcomes.sort_by_key(|&(ti, si, _)| (ti, si));
    let mut grouped: Vec<Vec<Result<Completion, String>>> = (0..tasks.len())
        .map(|_| Vec::with_capacity(samples))
        .collect();
    for (ti, _si, res) in outcomes {
        grouped[ti].push(res);
    }

    tasks
        .iter()
        .zip(grouped)
        .map(|(task, samples)| TaskAttempts {
            task: task.clone(),
            samples,
        })
        .collect()
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

        let attempts = run_tasks(&mock, &tasks, 2, 0, 1);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].task.id, "a");
        assert_eq!(attempts[0].samples.len(), 1);
        assert!(attempts[0].samples[0].is_ok());
        assert_eq!(attempts[1].task.id, "b");
        assert!(attempts[1].samples[0].is_err());
    }

    #[test]
    fn run_tasks_draws_k_samples_per_task() {
        let tasks = vec![Task {
            id: "a".into(),
            problem: "P-A".into(),
            expected_answer: "1".into(),
        }];
        let mock = MockModel::new(vec![(
            "P-A".to_string(),
            Completion {
                text: "<think>..</think> 1".into(),
                completion_tokens: Some(3),
            },
        )]);

        let attempts = run_tasks(&mock, &tasks, 4, 0, 3);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].samples.len(), 3, "three draws per task");
        assert!(attempts[0].samples.iter().all(|r| r.is_ok()));
    }
}
