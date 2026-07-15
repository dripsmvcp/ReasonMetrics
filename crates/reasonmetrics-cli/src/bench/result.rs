//! The committed result artifact and its leaderboard renderings.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::bench::aggregate::BenchMetrics;
use crate::bench::score::TaskRow;
use crate::bench::LeaderboardFormat;

#[derive(Debug, Clone, Serialize)]
pub struct TaskSetMeta {
    pub name: String,
    pub sha256: String,
    pub n: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sampling {
    pub temperature: f32,
    pub max_tokens: usize,
    pub samples: usize,
}

/// Provenance for an optional tiered-judge pass.
#[derive(Debug, Clone, Serialize)]
pub struct JudgeMeta {
    pub model: String,
    pub endpoint_host: String,
    pub band: [f32; 2],
    pub n_in_band: usize,
    pub n_scored: usize,
    pub mean_judge_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub schema_version: &'static str,
    pub tool_version: &'static str,
    pub generated_at: u64,
    pub command: String,
    pub task_set: TaskSetMeta,
    pub model: String,
    pub endpoint_host: String,
    pub sampling: Sampling,
    pub tokens_estimated: bool,
    pub metrics: BenchMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judge: Option<JudgeMeta>,
    pub results: Vec<TaskRow>,
}

impl BenchResult {
    /// `task_set` is (name, sha256, n); `sampling` is (temperature, max_tokens, samples).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: String,
        task_set: (String, String, usize),
        model: String,
        endpoint_host: String,
        sampling: (f32, usize, usize),
        tokens_estimated: bool,
        metrics: BenchMetrics,
        results: Vec<TaskRow>,
    ) -> Self {
        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            schema_version: "1",
            tool_version: env!("CARGO_PKG_VERSION"),
            generated_at,
            command,
            task_set: TaskSetMeta {
                name: task_set.0,
                sha256: task_set.1,
                n: task_set.2,
            },
            model,
            endpoint_host,
            sampling: Sampling {
                temperature: sampling.0,
                max_tokens: sampling.1,
                samples: sampling.2,
            },
            tokens_estimated,
            metrics,
            judge: None,
            results,
        }
    }

    pub fn default_out_path(&self) -> PathBuf {
        let model_slug: String = self
            .model
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let short = &self.task_set.sha256[..self.task_set.sha256.len().min(6)];
        PathBuf::from(format!(
            "results/{}-{}-{}.json",
            self.task_set.name, model_slug, short
        ))
    }

    pub fn write_json(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn render(&self, format: LeaderboardFormat) -> String {
        match format {
            LeaderboardFormat::Json => serde_json::to_string_pretty(self).unwrap_or_default(),
            LeaderboardFormat::Table => self.render_table(),
            LeaderboardFormat::Md => self.render_md(),
            LeaderboardFormat::Html => self.render_html(),
        }
    }

    fn cells(&self) -> (String, String, String, String, String) {
        let m = &self.metrics;
        let quality = format!("{:.1}", m.mean_quality);
        let accuracy = format!("{:.1}%", m.accuracy * 100.0);
        let tpc = m
            .tokens_per_correct
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "-".into());
        let cost = m
            .cost_per_1k_correct
            .map(|v| format!("{v:.2}"))
            .unwrap_or_else(|| "-".into());
        let n = format!("{}", m.n_scored);
        (quality, accuracy, tpc, cost, n)
    }

    fn render_table(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "{:<28} {:>8} {:>9} {:>13} {:>12} {:>5}\n{}\n{:<28} {:>8} {:>9} {:>13} {:>12} {:>5}\n",
            "model",
            "quality",
            "accuracy",
            "tokens/correct",
            "cost/1k",
            "n",
            "-".repeat(78),
            self.model,
            quality,
            accuracy,
            tpc,
            cost,
            n,
        )
    }

    fn render_md(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "| model | quality | accuracy | tokens/correct | cost/1k | n |\n\
             |---|---|---|---|---|---|\n\
             | {} | {} | {} | {} | {} | {} |\n",
            self.model, quality, accuracy, tpc, cost, n,
        )
    }

    fn render_html(&self) -> String {
        let (quality, accuracy, tpc, cost, n) = self.cells();
        format!(
            "<table>\n<tr><th>model</th><th>quality</th><th>accuracy</th>\
             <th>tokens/correct</th><th>cost/1k</th><th>n</th></tr>\n\
             <tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n</table>\n",
            self.model, quality, accuracy, tpc, cost, n,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::aggregate::BenchMetrics;
    use crate::bench::score::TaskRow;
    use crate::bench::LeaderboardFormat;

    fn sample() -> BenchResult {
        let metrics = BenchMetrics {
            n_attempted: 2,
            n_scored: 2,
            n_errored: 0,
            accuracy: 0.5,
            mean_quality: 70.0,
            tokens_per_correct: Some(300.0),
            cost_per_1k_correct: None,
        };
        let rows = vec![
            TaskRow {
                id: "a".into(),
                correct: true,
                quality: 80.0,
                tokens: 100,
                tokens_estimated: false,
                samples: 1,
                samples_correct: 1,
                judge_score: None,
                error: None,
            },
            TaskRow {
                id: "b".into(),
                correct: false,
                quality: 60.0,
                tokens: 200,
                tokens_estimated: false,
                samples: 1,
                samples_correct: 0,
                judge_score: None,
                error: None,
            },
        ];
        BenchResult::new(
            "reasonmetrics bench --model m".into(),
            ("overthinking-v1".into(), "abc123".into(), 2),
            "m".into(),
            "localhost:8000".into(),
            (0.0, 8192, 1),
            false,
            metrics,
            rows,
        )
    }

    #[test]
    fn default_out_path_uses_taskset_model_and_shorthash() {
        let r = sample();
        let p = r.default_out_path();
        assert_eq!(
            p,
            std::path::PathBuf::from("results/overthinking-v1-m-abc123.json")
        );
    }

    #[test]
    fn json_roundtrips_and_omits_secrets() {
        let r = sample();
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"schema_version\":\"1\""));
        assert!(json.contains("\"endpoint_host\":\"localhost:8000\""));
        assert!(json.contains("\"task_set\""));
        assert!(!json.to_lowercase().contains("authorization"));
    }

    #[test]
    fn table_render_has_header_and_a_row() {
        let out = sample().render(LeaderboardFormat::Table);
        assert!(out.contains("model"));
        assert!(out.contains("quality"));
        assert!(out.contains("50.0%") || out.contains("0.50")); // accuracy shown
        assert!(out.contains("m"));
    }
}
