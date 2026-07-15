//! `reasonmetrics bench` — run a fixed task set against an OpenAI-compatible
//! endpoint and score the returned traces. Feature-gated (`bench`).

pub mod model;
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

pub fn run(_args: BenchArgs, _scoring: &ScoringConfig) -> anyhow::Result<()> {
    anyhow::bail!("reasonmetrics bench: not yet implemented")
}
