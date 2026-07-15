//! Reduce per-task rows into leaderboard metrics.

use serde::Serialize;

use crate::bench::score::TaskRow;

#[derive(Debug, Clone, Serialize)]
pub struct BenchMetrics {
    pub n_attempted: usize,
    pub n_scored: usize,
    pub n_errored: usize,
    pub accuracy: f32,
    pub mean_quality: f32,
    pub tokens_per_correct: Option<f32>,
    pub cost_per_1k_correct: Option<f32>,
}

pub fn aggregate(rows: &[TaskRow], cost_per_mtok: Option<f32>) -> BenchMetrics {
    let n_attempted = rows.len();
    let scored: Vec<&TaskRow> = rows.iter().filter(|r| r.error.is_none()).collect();
    let n_scored = scored.len();
    let n_errored = n_attempted - n_scored;

    let n_correct = scored.iter().filter(|r| r.correct).count();
    let accuracy = if n_scored > 0 {
        n_correct as f32 / n_scored as f32
    } else {
        0.0
    };
    let mean_quality = if n_scored > 0 {
        scored.iter().map(|r| r.quality).sum::<f32>() / n_scored as f32
    } else {
        0.0
    };

    let total_tokens: usize = scored.iter().map(|r| r.tokens).sum();
    let tokens_per_correct = if n_correct > 0 {
        Some(total_tokens as f32 / n_correct as f32)
    } else {
        None
    };
    let cost_per_1k_correct = match (cost_per_mtok, n_correct > 0) {
        (Some(cost), true) => {
            let total_cost = total_tokens as f32 / 1_000_000.0 * cost;
            Some(total_cost / n_correct as f32 * 1000.0)
        }
        _ => None,
    };

    BenchMetrics {
        n_attempted,
        n_scored,
        n_errored,
        accuracy,
        mean_quality,
        tokens_per_correct,
        cost_per_1k_correct,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::score::TaskRow;

    fn row(id: &str, correct: bool, quality: f32, tokens: usize, err: Option<&str>) -> TaskRow {
        TaskRow {
            id: id.into(),
            correct,
            quality,
            tokens,
            tokens_estimated: false,
            error: err.map(String::from),
        }
    }

    #[test]
    fn aggregates_counts_accuracy_and_costs() {
        let rows = vec![
            row("a", true, 80.0, 100, None),
            row("b", false, 40.0, 300, None),
            row("c", true, 60.0, 200, None),
            row("d", false, 0.0, 0, Some("timeout")),
        ];
        let m = aggregate(&rows, Some(0.50));

        assert_eq!(m.n_attempted, 4);
        assert_eq!(m.n_scored, 3);
        assert_eq!(m.n_errored, 1);
        assert!((m.accuracy - 2.0 / 3.0).abs() < 1e-6);
        assert!((m.mean_quality - 60.0).abs() < 1e-6); // (80+40+60)/3
        // total scored tokens = 600, correct = 2 → 300 tokens/correct
        assert!((m.tokens_per_correct.unwrap() - 300.0).abs() < 1e-6);
        // cost = 600/1e6 * 0.50 = 0.0003 ; per correct = 0.00015 ; per 1k = 0.15
        assert!((m.cost_per_1k_correct.unwrap() - 0.15).abs() < 1e-6);
    }

    #[test]
    fn zero_correct_yields_none_ratios() {
        let rows = vec![row("a", false, 10.0, 100, None)];
        let m = aggregate(&rows, Some(0.50));
        assert_eq!(m.accuracy, 0.0);
        assert!(m.tokens_per_correct.is_none());
        assert!(m.cost_per_1k_correct.is_none());
    }

    #[test]
    fn no_cost_flag_yields_none_cost() {
        let rows = vec![row("a", true, 90.0, 100, None)];
        let m = aggregate(&rows, None);
        assert!(m.tokens_per_correct.is_some());
        assert!(m.cost_per_1k_correct.is_none());
    }
}
