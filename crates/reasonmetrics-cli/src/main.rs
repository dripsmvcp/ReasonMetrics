use clap::{Parser, Subcommand};

#[cfg(feature = "bench")]
mod bench;
mod output;
mod parser;
mod pipeline;
use std::path::{Path, PathBuf};

use crate::parser::read_jsonl;
use crate::pipeline::score_traces;
use reasonmetrics_core::config::{Config, OutputFormat};
use reasonmetrics_core::trace::ScoredTrace;

#[derive(Parser)]
#[command(name = "reasonmetrics", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, global = true)]
    threads: Option<usize>,
    #[arg(long, global = true, default_value = "reasonmetrics.toml")]
    config: PathBuf,
    #[arg(short, long, global = true)]
    verbose: bool,
}
#[derive(Subcommand)]
enum Commands {
    Score {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Filter {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long, default_value = "clean.jsonl")]
        output: PathBuf,
        /// Keep traces scoring at least N, where the score is a percentile
        /// against a reference corpus of real traces: 70 = "better than 70% of
        /// real reasoning traces". Absolute, so it works on a single trace.
        #[arg(long)]
        min_score: Option<f32>,
        /// Keep the best N% of THIS input file (e.g. 30 keeps the top 30%).
        /// Unlike --min-score, guarantees exactly how many traces survive —
        /// use it when the output size has to be fixed. Overrides --min-score.
        #[arg(long, value_parser = parse_percent)]
        top_percent: Option<f32>,
        #[arg(long)]
        min_efficiency: Option<f32>,
        #[arg(long)]
        min_language: Option<f32>,
    },
    Report {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long, default_value = "reasonmetrics_report.html")]
        output: PathBuf,
    },

    Stats {
        #[arg(short, long)]
        input: PathBuf,
    },
    Explain,
    InitConfig,
    /// List the model families in the embedded registry
    Models,
    /// Benchmark a model's reasoning over a fixed task set (feature: bench)
    #[cfg(feature = "bench")]
    Bench {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value = "overthinking-v1")]
        task_set: String,
        #[arg(long, default_value_t = 0.0)]
        temperature: f32,
        #[arg(long, default_value_t = 8192)]
        max_tokens: usize,
        #[arg(long, default_value_t = 8)]
        concurrency: usize,
        #[arg(long)]
        cost_per_mtok: Option<f32>,
        #[arg(long)]
        api_key_env: Option<String>,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        #[arg(long, default_value = "table")]
        format: String,
        #[arg(long, default_value_t = 2)]
        retries: usize,
        /// Draws per task; >1 reports pass@k (needs temperature > 0)
        #[arg(long, default_value_t = 1)]
        samples: usize,
        /// Opt-in tiered judge: OpenAI-compatible endpoint for the judge model
        #[arg(long)]
        judge_endpoint: Option<String>,
        /// Judge model name (required with --judge-endpoint)
        #[arg(long)]
        judge_model: Option<String>,
        /// Heuristic-quality band to escalate to the judge, "lo,hi"
        #[arg(long, default_value = "40,70")]
        judge_band: String,
        /// Env var holding the judge endpoint's API key
        #[arg(long)]
        judge_api_key_env: Option<String>,
    },
    /// Combine committed bench result JSONs into one leaderboard (feature: bench)
    #[cfg(feature = "bench")]
    Leaderboard {
        #[arg(long, default_value = "results")]
        results: std::path::PathBuf,
        /// Restrict to a single task set (default: one table per set found)
        #[arg(long)]
        task_set: Option<String>,
        /// accuracy|quality|tokens|cost
        #[arg(long, default_value = "accuracy")]
        sort: String,
        #[arg(long, default_value = "table")]
        format: String,
        /// Write the rendered leaderboard here instead of stdout
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Generate a standalone static site (index.html) in this directory
        #[arg(long)]
        site: Option<std::path::PathBuf>,
        /// Validate result JSONs and exit non-zero on any problem (for CI)
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    if let Some(threads) = cli.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .map_err(|e| anyhow::anyhow!("Failed to set thread count: {}", e))?;
    }

    match cli.command {
        Commands::Score { input, output } => {
            let config = Config::load(&cli.config)?;
            cmd_score(&input, output.as_deref(), &config)?
        }
        Commands::Filter {
            input,
            output,
            min_score,
            top_percent,
            min_efficiency,
            min_language,
        } => {
            let config = Config::load(&cli.config)?;
            cmd_filter(
                &input,
                &output,
                &config,
                min_score,
                top_percent,
                min_efficiency,
                min_language,
            )?
        }
        Commands::Report { input, output } => {
            let config = Config::load(&cli.config)?;
            cmd_report(&input, &output, &config)?
        }
        Commands::Stats { input } => {
            let config = Config::load(&cli.config)?;
            cmd_stats(&input, &config)?
        }
        Commands::Explain => cmd_explain(),
        Commands::InitConfig => cmd_init_config(),
        Commands::Models => cmd_models(),
        #[cfg(feature = "bench")]
        Commands::Bench {
            endpoint,
            model,
            task_set,
            temperature,
            max_tokens,
            concurrency,
            cost_per_mtok,
            api_key_env,
            out,
            format,
            retries,
            samples,
            judge_endpoint,
            judge_model,
            judge_band,
            judge_api_key_env,
        } => {
            let config = Config::load(&cli.config)?;
            let format = format
                .parse::<bench::LeaderboardFormat>()
                .map_err(|e| anyhow::anyhow!(e))?;
            let judge_band = parse_band(&judge_band)?;
            let args = bench::BenchArgs {
                endpoint,
                model,
                task_set,
                temperature,
                max_tokens,
                concurrency,
                cost_per_mtok,
                api_key_env,
                out,
                format,
                retries,
                samples,
                judge_endpoint,
                judge_model,
                judge_band,
                judge_api_key_env,
            };
            bench::run(args, &config.scoring)?
        }
        #[cfg(feature = "bench")]
        Commands::Leaderboard {
            results,
            task_set,
            sort,
            format,
            out,
            site,
            strict,
        } => {
            let sort = sort
                .parse::<bench::leaderboard::SortKey>()
                .map_err(|e| anyhow::anyhow!(e))?;
            let format = format
                .parse::<bench::LeaderboardFormat>()
                .map_err(|e| anyhow::anyhow!(e))?;
            bench::run_leaderboard(bench::LeaderboardArgs {
                results,
                task_set,
                sort,
                format,
                out,
                site,
                strict,
            })?
        }
    }

    Ok(())
}

/// Parse a `"lo,hi"` judge band into an inclusive (lo, hi) pair.
#[cfg(feature = "bench")]
fn parse_band(s: &str) -> anyhow::Result<(f32, f32)> {
    let (lo, hi) = s
        .split_once(',')
        .ok_or_else(|| anyhow::anyhow!("--judge-band must be \"lo,hi\", got `{s}`"))?;
    let lo: f32 = lo.trim().parse()?;
    let hi: f32 = hi.trim().parse()?;
    if lo > hi {
        anyhow::bail!("--judge-band lo ({lo}) must be <= hi ({hi})");
    }
    Ok((lo, hi))
}

fn cmd_score(input: &Path, output: Option<&Path>, config: &Config) -> anyhow::Result<()> {
    let traces = read_jsonl(input, config.input.strict)?;
    eprintln!("Loaded {} traces from {}", traces.len(), input.display());

    let scored = score_traces(&traces, &config.scoring);

    let (output_path, format) = match output {
        Some(path) => {
            let format = match path.extension().and_then(|e| e.to_str()) {
                Some(ext) if ext.eq_ignore_ascii_case("parquet") => OutputFormat::Parquet,
                Some(ext) if ext.eq_ignore_ascii_case("jsonl") => OutputFormat::Jsonl,
                Some(other) => anyhow::bail!(
                    "Unsupported output format: {}. Use .parquet or .jsonl",
                    other
                ),
                None => config.output.format,
            };
            (path.to_path_buf(), format)
        }
        None => match config.output.format {
            OutputFormat::Parquet => (PathBuf::from("scored.parquet"), OutputFormat::Parquet),
            OutputFormat::Jsonl => (PathBuf::from("scored.jsonl"), OutputFormat::Jsonl),
        },
    };

    match format {
        OutputFormat::Parquet => crate::output::parquet::write_parquet(&scored, &output_path)?,
        OutputFormat::Jsonl => crate::output::jsonl::write_jsonl(&scored, &output_path)?,
    }

    eprintln!("\nResults saved to {}", output_path.display());
    print_summary(&scored);
    Ok(())
}

/// `--top-percent` takes a percentage: 0 keeps nothing, 100 keeps everything.
fn parse_percent(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| format!("`{s}` is not a number"))?;
    if !(0.0..=100.0).contains(&v) {
        return Err(format!("must be between 0 and 100, got {v}"));
    }
    Ok(v)
}

fn cmd_filter(
    input: &Path,
    output: &Path,
    config: &Config,
    min_score: Option<f32>,
    top_percent: Option<f32>,
    min_efficiency: Option<f32>,
    min_language: Option<f32>,
) -> anyhow::Result<()> {
    let traces = read_jsonl(input, config.input.strict)?;
    eprintln!("Loaded {} traces", traces.len());

    let scored = score_traces(&traces, &config.scoring);

    // The dimension gates apply in both modes; only the quality cut differs.
    let passes_gates = |s: &reasonmetrics_core::trace::ScoredTrace| {
        min_efficiency.map_or(true, |m| s.efficiency_score >= m)
            && min_language.map_or(true, |m| s.language_score >= m)
    };

    let filtered: Vec<_> = match top_percent {
        // Rank-relative: keep the best N% OF THIS FILE. Exact output size, which
        // an absolute threshold cannot promise.
        Some(pct) => {
            let mut ranked: Vec<_> = scored
                .iter()
                .zip(traces.iter())
                .filter(|(s, _)| passes_gates(s))
                .collect();
            // Descending by score; ties broken by id so the output is deterministic.
            ranked.sort_by(|a, b| {
                b.0.quality_score
                    .partial_cmp(&a.0.quality_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.id.cmp(&b.0.id))
            });
            let keep = ((ranked.len() as f32) * pct / 100.0).round() as usize;
            eprintln!(
                "Keeping the top {pct}% of {} traces by score ({keep} traces)",
                ranked.len()
            );
            ranked
                .into_iter()
                .take(keep)
                .map(|(_, trace)| trace.clone())
                .collect()
        }
        // Absolute: "better than N% of real reference traces".
        None => {
            let min_score = min_score.unwrap_or(config.output.min_score);
            scored
                .iter()
                .zip(traces.iter())
                .filter(|(s, _)| s.quality_score >= min_score && passes_gates(s))
                .map(|(_, trace)| trace.clone())
                .collect()
        }
    };

    let removed = traces.len() - filtered.len();
    eprintln!(
        "Filtered: kept {} / {} ({} removed, {:.1}%)",
        filtered.len(),
        traces.len(),
        removed,
        removed as f32 / traces.len() as f32 * 100.0
    );

    crate::output::jsonl::write_jsonl(&filtered, output)?;
    eprintln!("Clean traces saved to {}", output.display());
    Ok(())
}

fn cmd_report(input: &Path, output: &Path, config: &Config) -> anyhow::Result<()> {
    let traces = read_jsonl(input, config.input.strict)?;
    eprintln!("Loaded {} traces, scoring...", traces.len());

    let scored = score_traces(&traces, &config.scoring);
    crate::output::report::generate_report(&scored, output)?;

    eprintln!("Report saved to {}", output.display());
    Ok(())
}

fn cmd_stats(input: &Path, config: &Config) -> anyhow::Result<()> {
    let traces = read_jsonl(input, config.input.strict)?;
    eprintln!("Loaded {} traces, scoring...\n", traces.len());

    let scored = score_traces(&traces, &config.scoring);
    print_detailed_stats(&scored);
    Ok(())
}

fn cmd_explain() {
    println!(
        r#"
reasonmetrics quality dimensions
==============================

1. EFFICIENCY (weight: 20%)
   Measures: Ratio of productive reasoning to total trace length.
   Low score means: Too many "wait, let me restart" / "actually, no" phrases.
   Based on: "Think Deep, Not Just Long" (2602.13517)

2. LANGUAGE CONSISTENCY (weight: 12%)
   Measures: Whether the trace stays in one language throughout.
   Low score means: Language mixing (e.g., English/Chinese switching mid-trace).
   Based on: DeepSeek R1 paper (2501.12948)

3. ANSWER ALIGNMENT (weight: 18%)
   Measures: Whether the answer appears at the END of the trace.
   Low score means: Answer is buried in the middle, or no clear conclusion.
   Based on: Sky-T1 report (code answers in middle of QwQ traces)

4. STRUCTURAL CLARITY (weight: 10%)
   Measures: Logical step markers, paragraph breaks, structured reasoning.
   Low score means: Wall-of-text with no clear progression.
   Based on: LIMO paper — "Optimal Structural Organization"

5. REPETITION (weight: 15%)
   Measures: Repeated paragraphs and sentences within the trace.
   Low score means: Same content restated 3+ times.
   Based on: "Between Underthinking and Overthinking" (2505.00127)

6. OVERTHINKING (weight: 10%)
   Measures: Whether trace length is proportionate to problem complexity.
   Low score means: Simple problem with extremely long trace.
   Based on: "Do NOT Think That Much for 2+3=?" (2412.21187)

7. SELF-VERIFICATION (weight: 8%)
   Measures: Whether the model checks its own work before concluding.
   Low score means: No verification, substitution, or sanity checks.
   Based on: LIMO + DeepSeek R1 papers

8. LENGTH CALIBRATION (weight: 7%)
   Measures: Whether trace falls in the empirically good length range.
   Low score means: Either too short (<100 words) or too long (>10000 words).
   Based on: OpenThoughts (2506.04178) — response length as quality signal

9. ACCURACY-EFFICIENCY (weight: 0% by default)
   Measures: Harmonic mean of answer correctness and token efficiency.
   Requires: expected_answer (aliases: ground_truth, label, target) on the trace.
   Based on: LLMThinkBench Overthinking Score (2507.04023, ACL 2026 Findings)

RAW SCORE      = weighted sum of all 9 dimensions (0-100).
QUALITY SCORE  = that raw score's PERCENTILE against a reference corpus of 2,517
                 real reasoning traces — "better than N% of real traces".

  Both are reported. Filter and rank on quality_score: the raw scale is crushed
  (99.9% of real traces score above 70), so an absolute cut on it keeps almost
  everything. The mapping is monotone, so it never reorders traces.

  `filter --min-score 70`   keeps traces better than 70% of the reference corpus.
  `filter --top-percent 30` keeps the best 30% of YOUR file, whatever it contains.

  Because it is a percentile, a very short trace scores near zero — correctly: it
  contains little reasoning. See docs/CALIBRATION.md.
"#
    );
}

fn cmd_init_config() {
    print!("{}", Config::default_toml());
}

fn cmd_models() {
    let entries = reasonmetrics_core::registry::entries();
    println!("{} model families in the registry:\n", entries.len());
    for e in entries {
        println!("{}  — {}", e.id, e.display_name);
        if !e.extraction.think_tags.is_empty() {
            let tags: Vec<&str> = e
                .extraction
                .think_tags
                .iter()
                .map(|(start, _)| start.as_str())
                .collect();
            println!("  tags:   {}", tags.join(", "));
        }
        if !e.extraction.reasoning_fields.is_empty() {
            println!("  fields: {}", e.extraction.reasoning_fields.join(", "));
        }
        if let Some(c) = &e.cost {
            println!(
                "  cost:   ${}/M in, ${}/M out  ({})",
                c.input_per_mtok, c.output_per_mtok, c.source
            );
        }
        println!();
    }
    println!("Add a model family: one TOML + one fixture — see CONTRIBUTING.md.");
}

fn print_summary(scored: &[ScoredTrace]) {
    let n = scored.len() as f32;
    let avg = scored.iter().map(|s| s.quality_score).sum::<f32>() / n;
    let high = scored.iter().filter(|s| s.quality_score >= 70.0).count();
    let mixed = scored.iter().filter(|s| s.is_language_mixed).count();

    eprintln!("\n=== Summary ===");
    eprintln!("Total traces:     {}", scored.len());
    eprintln!("Avg quality:      {:.1}/100", avg);
    eprintln!(
        "High quality:     {} ({:.1}%)",
        high,
        high as f32 / n * 100.0
    );
    eprintln!(
        "Language mixed:   {} ({:.1}%)",
        mixed,
        mixed as f32 / n * 100.0
    );
}

type DimExtractor<'a> = Vec<(&'a str, Box<dyn Fn(&ScoredTrace) -> f32>)>;

fn print_detailed_stats(scored: &[ScoredTrace]) {
    let n = scored.len() as f32;

    println!("=== Dataset Statistics ===\n");
    println!("Total traces: {}\n", scored.len());

    let dims: DimExtractor = vec![
        (
            "Quality (composite)",
            Box::new(|s: &ScoredTrace| s.quality_score),
        ),
        ("Efficiency", Box::new(|s: &ScoredTrace| s.efficiency_score)),
        (
            "Language Consistency",
            Box::new(|s: &ScoredTrace| s.language_score),
        ),
        (
            "Answer Alignment",
            Box::new(|s: &ScoredTrace| s.answer_alignment_score),
        ),
        (
            "Structural Clarity",
            Box::new(|s: &ScoredTrace| s.structural_score),
        ),
        ("Repetition", Box::new(|s: &ScoredTrace| s.repetition_score)),
        (
            "Overthinking",
            Box::new(|s: &ScoredTrace| s.overthinking_score),
        ),
        (
            "Self-Verification",
            Box::new(|s: &ScoredTrace| s.verification_score),
        ),
        (
            "Length Calibration",
            Box::new(|s: &ScoredTrace| s.length_score),
        ),
    ];

    println!(
        "{:<25} {:>8} {:>8} {:>8} {:>10}",
        "Dimension", "Avg", "Min", "Max", "% High"
    );
    println!("{}", "-".repeat(65));

    for (name, getter) in &dims {
        let values: Vec<f32> = scored.iter().map(getter).collect();
        let avg = values.iter().sum::<f32>() / n;
        let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let high = values.iter().filter(|&&v| v >= 80.0).count();
        println!(
            "{:<25} {:>7.1} {:>7.1} {:>7.1} {:>8.1}%",
            name,
            avg,
            min,
            max,
            high as f32 / n * 100.0
        );
    }

    println!("\n=== Issues Detected ===\n");
    let language_mixed = scored.iter().filter(|s| s.is_language_mixed).count();
    let no_verify = scored.iter().filter(|s| !s.has_self_verification).count();
    let overthinking = scored
        .iter()
        .filter(|s| s.overthinking_score < 50.0)
        .count();
    let answer_misaligned = scored
        .iter()
        .filter(|s| s.answer_alignment_score < 50.0)
        .count();

    println!(
        "Language mixed:       {} ({:.1}%)",
        language_mixed,
        language_mixed as f32 / n * 100.0
    );
    println!(
        "No self-verification: {} ({:.1}%)",
        no_verify,
        no_verify as f32 / n * 100.0
    );
    println!(
        "Severe overthinking:  {} ({:.1}%)",
        overthinking,
        overthinking as f32 / n * 100.0
    );
    println!(
        "Answer misaligned:    {} ({:.1}%)",
        answer_misaligned,
        answer_misaligned as f32 / n * 100.0
    );

    let removable = scored.iter().filter(|s| s.quality_score < 70.0).count();
    println!("\n=== Recommendation ===\n");
    println!(
        "Remove {} traces (quality < 70) → {} clean traces",
        removable,
        scored.len() - removable
    );
}
