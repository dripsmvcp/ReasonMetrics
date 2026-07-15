//! `reasonmetrics bench` — run a fixed task set against an OpenAI-compatible
//! endpoint and score the returned traces. Feature-gated (`bench`).

pub mod aggregate;
pub mod judge;
pub mod leaderboard;
pub mod model;
pub mod result;
pub mod runner;
pub mod score;
pub mod site;
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
    /// Draws per task. >1 turns accuracy into pass@k (any correct sample solves).
    pub samples: usize,
    /// Opt-in tiered judge: endpoint + model. Both required to enable it.
    pub judge_endpoint: Option<String>,
    pub judge_model: Option<String>,
    /// Heuristic-quality band escalated to the judge (inclusive).
    pub judge_band: (f32, f32),
    pub judge_api_key_env: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LeaderboardArgs {
    pub results: PathBuf,
    pub task_set: Option<String>,
    pub sort: leaderboard::SortKey,
    pub format: LeaderboardFormat,
    pub out: Option<PathBuf>,
    /// If set, write a complete standalone `index.html` here instead of a table.
    pub site: Option<PathBuf>,
    /// Validate every result JSON and exit non-zero on any problem (for CI).
    pub strict: bool,
}

/// Combine every committed result JSON under `results/` into one leaderboard.
pub fn run_leaderboard(args: LeaderboardArgs) -> anyhow::Result<()> {
    // Validation mode: check every result and exit, rendering nothing. An empty
    // results dir is valid (nothing to check), so this is CI-safe from day one.
    if args.strict {
        let problems = leaderboard::validate_dir(&args.results)?;
        if problems.is_empty() {
            eprintln!("All result JSONs in {} are valid.", args.results.display());
            return Ok(());
        }
        for p in &problems {
            eprintln!("  ✗ {p}");
        }
        anyhow::bail!("{} result validation problem(s)", problems.len());
    }

    let entries = leaderboard::load_dir(&args.results)?;
    eprintln!(
        "Loaded {} result file(s) from {}",
        entries.len(),
        args.results.display()
    );
    let groups = leaderboard::assemble(entries, args.task_set.as_deref(), args.sort);

    // A `--site` dir always gets a page — an empty results dir yields the honest
    // "No results yet" placeholder, which CI needs to render before any run lands.
    if let Some(dir) = &args.site {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("index.html");
        std::fs::write(&path, site::render(&groups))?;
        eprintln!("Leaderboard site written to {}", path.display());
        return Ok(());
    }

    // A table over nothing is not useful; guide the user instead.
    if groups.is_empty() {
        anyhow::bail!(
            "no bench result JSONs found in {} (run `reasonmetrics bench` first)",
            args.results.display()
        );
    }

    let rendered = leaderboard::render(&groups, args.format);
    match &args.out {
        Some(p) => {
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(p, &rendered)?;
            eprintln!("Leaderboard written to {}", p.display());
        }
        None => println!("{rendered}"),
    }
    Ok(())
}

/// Build the optional judge model from `--judge-*` args. Returns `None` when no
/// judge endpoint was requested; errors if only half the pair was given.
#[allow(clippy::type_complexity)]
fn build_judge(
    args: &BenchArgs,
) -> anyhow::Result<Option<(Box<dyn model::Model>, String, judge::Band)>> {
    match (&args.judge_endpoint, &args.judge_model) {
        (None, None) => Ok(None),
        (Some(endpoint), Some(jmodel)) => {
            let key = match &args.judge_api_key_env {
                Some(var) => Some(std::env::var(var).map_err(|_| {
                    anyhow::anyhow!("env var `{var}` (from --judge-api-key-env) is not set")
                })?),
                None => None,
            };
            // Judge only needs a short reply; temperature 0 for a stable verdict.
            let m = model::HttpModel::new(endpoint, jmodel, key, 0.0, 64);
            let band = judge::Band {
                lo: args.judge_band.0,
                hi: args.judge_band.1,
            };
            Ok(Some((Box::new(m), model::host_of(endpoint), band)))
        }
        _ => anyhow::bail!("--judge-endpoint and --judge-model must be given together"),
    }
}

pub fn run(args: BenchArgs, scoring: &ScoringConfig) -> anyhow::Result<()> {
    let task_set = taskset::load(&args.task_set)?;
    eprintln!(
        "Loaded task set `{}` ({} tasks, sha256 {}…)",
        task_set.name,
        task_set.tasks.len(),
        &task_set.sha256[..task_set.sha256.len().min(8)]
    );

    let samples = args.samples.max(1);
    if samples > 1 && args.temperature == 0.0 {
        eprintln!(
            "Warning: --samples {samples} with --temperature 0 draws {samples} identical \
             completions; pass@k is only meaningful above temperature 0."
        );
    }

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

    let attempts = runner::run_tasks(
        &http,
        &task_set.tasks,
        args.concurrency,
        args.retries,
        samples,
    );
    let mut rows = score::build_rows(&attempts, scoring);

    // Optional tiered judge: escalate only the uncertain middle band.
    let judge_meta = build_judge(&args)?.map(|(judge, host, band)| {
        let report = judge::run_judging(&mut rows, &attempts, judge.as_ref(), band);
        eprintln!(
            "Judge scored {} of {} in-band traces (quality {:.0}–{:.0}).",
            report.n_scored, report.n_in_band, band.lo, band.hi
        );
        result::JudgeMeta {
            model: args.judge_model.clone().unwrap_or_default(),
            endpoint_host: host,
            band: [band.lo, band.hi],
            n_in_band: report.n_in_band,
            n_scored: report.n_scored,
            mean_judge_score: report.mean_judge_score,
        }
    });

    let metrics = aggregate::aggregate(&rows, args.cost_per_mtok);
    let any_estimated = rows.iter().any(|r| r.tokens_estimated);

    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let mut result = result::BenchResult::new(
        command,
        (
            task_set.name.clone(),
            task_set.sha256.clone(),
            task_set.tasks.len(),
        ),
        args.model.clone(),
        model::host_of(&args.endpoint),
        (args.temperature, args.max_tokens, samples),
        any_estimated,
        metrics,
        rows,
    );
    result.judge = judge_meta;

    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| result.default_out_path());
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
