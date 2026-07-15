//! `reasonmetrics bench` — run a fixed task set against an OpenAI-compatible
//! endpoint and score the returned traces. Feature-gated (`bench`).

pub mod aggregate;
pub mod model;
pub mod result;
pub mod runner;
pub mod score;
pub mod taskset;

use std::path::PathBuf;
use std::str::FromStr;

use reasonmetrics_core::config::ScoringConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderboardFormat {
    Table,
    Md,
    Html,
    Json,
}

impl FromStr for LeaderboardFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "md" | "markdown" => Ok(Self::Md),
            "html" => Ok(Self::Html),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown format `{other}` (use table|md|html|json)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BenchArgs {
    pub endpoint: String,
    pub model: String,
    pub task_set: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub concurrency: usize,
    pub cost_per_mtok: Option<f32>,
    pub api_key_env: Option<String>,
    pub out: Option<PathBuf>,
    pub format: LeaderboardFormat,
    pub retries: usize,
}

pub fn run(args: BenchArgs, scoring: &ScoringConfig) -> anyhow::Result<()> {
    let task_set = taskset::load(&args.task_set)?;
    eprintln!(
        "Loaded task set `{}` ({} tasks, sha256 {}…)",
        task_set.name,
        task_set.tasks.len(),
        &task_set.sha256[..task_set.sha256.len().min(8)]
    );

    let api_key = match &args.api_key_env {
        Some(var) => Some(
            std::env::var(var)
                .map_err(|_| anyhow::anyhow!("env var `{var}` (from --api-key-env) is not set"))?,
        ),
        None => None,
    };

    let http = model::HttpModel::new(
        &args.endpoint,
        &args.model,
        api_key,
        args.temperature,
        args.max_tokens,
    );

    let attempts = runner::run_tasks(&http, &task_set.tasks, args.concurrency, args.retries);
    let rows = score::build_rows(&attempts, scoring);
    let metrics = aggregate::aggregate(&rows, args.cost_per_mtok);
    let any_estimated = rows.iter().any(|r| r.tokens_estimated);

    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let result = result::BenchResult::new(
        command,
        (
            task_set.name.clone(),
            task_set.sha256.clone(),
            task_set.tasks.len(),
        ),
        args.model.clone(),
        model::host_of(&args.endpoint),
        (args.temperature, args.max_tokens, 1),
        any_estimated,
        metrics,
        rows,
    );

    let out_path = args.out.clone().unwrap_or_else(|| result.default_out_path());
    result.write_json(&out_path)?;
    eprintln!("Result written to {}", out_path.display());

    println!("{}", result.render(args.format));
    if result.metrics.n_errored > 0 {
        eprintln!(
            "Warning: {} of {} tasks errored and were excluded from accuracy.",
            result.metrics.n_errored, result.metrics.n_attempted
        );
    }
    Ok(())
}
