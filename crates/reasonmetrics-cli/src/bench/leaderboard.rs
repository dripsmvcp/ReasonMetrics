//! Assemble many committed bench result JSONs into one leaderboard.
//!
//! The reader is deliberately decoupled from [`super::result::BenchResult`]: it
//! declares only the fields a leaderboard needs and tolerates unknown ones, so
//! a result written by a newer tool version still loads. Cross-task-set rows are
//! never mixed — a leaderboard is grouped by (task set name, sha256), because
//! comparing accuracy across different problem sets is meaningless.

use std::cmp::Ordering;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::bench::LeaderboardFormat;

fn one() -> usize {
    1
}

/// One committed run, as read back from its result JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub schema_version: String,
    pub model: String,
    pub task_set: TaskSetRef,
    #[serde(default)]
    pub tool_version: String,
    #[serde(default)]
    pub generated_at: u64,
    pub sampling: SamplingRef,
    pub metrics: MetricsRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskSetRef {
    pub name: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub n: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SamplingRef {
    #[serde(default = "one")]
    pub samples: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsRef {
    #[serde(default)]
    pub n_scored: usize,
    #[serde(default)]
    pub accuracy: f32,
    #[serde(default)]
    pub mean_quality: f32,
    #[serde(default)]
    pub tokens_per_correct: Option<f32>,
    #[serde(default)]
    pub cost_per_1k_correct: Option<f32>,
}

/// One row of a rendered leaderboard.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub model: String,
    pub samples: usize,
    pub quality: f32,
    pub accuracy: f32,
    pub tokens_per_correct: Option<f32>,
    pub cost_per_1k_correct: Option<f32>,
    pub n_scored: usize,
    pub tool_version: String,
    pub generated_at: u64,
}

/// All rows sharing one frozen task set.
#[derive(Debug, Clone, Serialize)]
pub struct Group {
    pub task_set: String,
    pub sha256: String,
    pub n: usize,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Accuracy,
    Quality,
    Tokens,
    Cost,
}

impl FromStr for SortKey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "accuracy" => Ok(Self::Accuracy),
            "quality" => Ok(Self::Quality),
            "tokens" | "tokens_per_correct" => Ok(Self::Tokens),
            "cost" | "cost_per_1k_correct" => Ok(Self::Cost),
            other => Err(format!(
                "unknown sort `{other}` (accuracy|quality|tokens|cost)"
            )),
        }
    }
}

pub fn parse_entry(json: &str) -> anyhow::Result<Entry> {
    Ok(serde_json::from_str(json)?)
}

/// The schema version this build reads and writes.
pub const SCHEMA_VERSION: &str = "1";

/// Semantic checks on a parsed entry, beyond "it deserializes". Returns a list
/// of human-readable problems (empty = valid). The integrity check — a claimed
/// bundled task set must carry that set's frozen sha256 — is what keeps a
/// leaderboard entry honest: you cannot report against a modified problem set.
pub fn validate_entry(e: &Entry) -> Vec<String> {
    let mut v = Vec::new();
    if e.schema_version != SCHEMA_VERSION {
        v.push(format!(
            "schema_version {:?} unsupported (this build reads {:?})",
            e.schema_version, SCHEMA_VERSION
        ));
    }
    if e.model.trim().is_empty() {
        v.push("model is empty".into());
    }
    if e.task_set.name.trim().is_empty() {
        v.push("task_set.name is empty".into());
    }
    let sha_ok = e.task_set.sha256.len() == 64
        && e.task_set
            .sha256
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if !sha_ok {
        v.push("task_set.sha256 is not 64 lowercase hex chars".into());
    }
    if !(0.0..=1.0).contains(&e.metrics.accuracy) {
        v.push(format!("accuracy {} out of [0,1]", e.metrics.accuracy));
    }
    if !(0.0..=100.0).contains(&e.metrics.mean_quality) {
        v.push(format!(
            "mean_quality {} out of [0,100]",
            e.metrics.mean_quality
        ));
    }
    if e.sampling.samples < 1 {
        v.push("sampling.samples must be >= 1".into());
    }
    if e.task_set.n > 0 && e.metrics.n_scored > e.task_set.n {
        v.push(format!(
            "n_scored {} exceeds task_set.n {}",
            e.metrics.n_scored, e.task_set.n
        ));
    }
    // Integrity: if the set name is one we bundle, its sha must match ours.
    if sha_ok {
        if let Ok(ts) = crate::bench::taskset::load(&e.task_set.name) {
            if ts.sha256 != e.task_set.sha256 {
                v.push(format!(
                    "task_set.sha256 {}… does not match the frozen bundled `{}` ({}…) — \
                     results must be run against the committed task set",
                    &e.task_set.sha256[..8],
                    e.task_set.name,
                    &ts.sha256[..8],
                ));
            }
        }
    }
    v
}

/// Validate every `*.json` under `dir`: each must parse as a result and pass the
/// semantic checks. Returns all problems found (empty = everything valid). An
/// empty or missing directory is not an error — there is simply nothing to check.
pub fn validate_dir(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut problems = Vec::new();
    if !dir.exists() {
        return Ok(problems);
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();

    for p in paths {
        let raw = std::fs::read_to_string(&p)?;
        match parse_entry(&raw) {
            Err(e) => problems.push(format!(
                "{}: does not parse as a result JSON ({e})",
                p.display()
            )),
            Ok(entry) => {
                for msg in validate_entry(&entry) {
                    problems.push(format!("{}: {msg}", p.display()));
                }
            }
        }
    }
    Ok(problems)
}

/// Read every `*.json` under `dir` as a result entry. Files that do not parse
/// as a bench result are skipped with a warning rather than aborting the run.
pub fn load_dir(dir: &Path) -> anyhow::Result<Vec<Entry>> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("cannot read results dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();

    let mut entries = Vec::new();
    for p in paths {
        let raw = std::fs::read_to_string(&p)?;
        match parse_entry(&raw) {
            Ok(e) => entries.push(e),
            Err(err) => eprintln!("Skipping {}: not a bench result ({err})", p.display()),
        }
    }
    Ok(entries)
}

/// Ascending compare that always sorts `None` last (worst).
fn cmp_opt_asc(a: Option<f32>, b: Option<f32>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn sort_rows(rows: &mut [Row], sort: SortKey) {
    rows.sort_by(|a, b| {
        let primary = match sort {
            // Higher is better → descending.
            SortKey::Accuracy => b
                .accuracy
                .partial_cmp(&a.accuracy)
                .unwrap_or(Ordering::Equal)
                .then_with(|| cmp_opt_asc(a.tokens_per_correct, b.tokens_per_correct)),
            SortKey::Quality => b.quality.partial_cmp(&a.quality).unwrap_or(Ordering::Equal),
            // Lower is better → ascending, None last.
            SortKey::Tokens => cmp_opt_asc(a.tokens_per_correct, b.tokens_per_correct),
            SortKey::Cost => cmp_opt_asc(a.cost_per_1k_correct, b.cost_per_1k_correct),
        };
        // Stable, deterministic tie-break by model name.
        primary.then_with(|| a.model.cmp(&b.model))
    });
}

/// Dedup to the newest run per (task set, model, samples), group by task set,
/// and sort each group's rows. Output order is deterministic.
pub fn assemble(entries: Vec<Entry>, task_set_filter: Option<&str>, sort: SortKey) -> Vec<Group> {
    use std::collections::HashMap;

    let mut best: HashMap<(String, String, String, usize), Entry> = HashMap::new();
    for e in entries {
        if let Some(f) = task_set_filter {
            if e.task_set.name != f {
                continue;
            }
        }
        let key = (
            e.task_set.name.clone(),
            e.task_set.sha256.clone(),
            e.model.clone(),
            e.sampling.samples,
        );
        match best.get(&key) {
            Some(prev) if prev.generated_at >= e.generated_at => {}
            _ => {
                best.insert(key, e);
            }
        }
    }

    let mut groups: HashMap<(String, String), Group> = HashMap::new();
    for e in best.into_values() {
        let gk = (e.task_set.name.clone(), e.task_set.sha256.clone());
        let g = groups.entry(gk).or_insert_with(|| Group {
            task_set: e.task_set.name.clone(),
            sha256: e.task_set.sha256.clone(),
            n: e.task_set.n,
            rows: Vec::new(),
        });
        g.rows.push(Row {
            model: e.model,
            samples: e.sampling.samples,
            quality: e.metrics.mean_quality,
            accuracy: e.metrics.accuracy,
            tokens_per_correct: e.metrics.tokens_per_correct,
            cost_per_1k_correct: e.metrics.cost_per_1k_correct,
            n_scored: e.metrics.n_scored,
            tool_version: e.tool_version,
            generated_at: e.generated_at,
        });
    }

    let mut out: Vec<Group> = groups.into_values().collect();
    for g in &mut out {
        sort_rows(&mut g.rows, sort);
    }
    out.sort_by(|a, b| a.task_set.cmp(&b.task_set).then(a.sha256.cmp(&b.sha256)));
    out
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(6)]
}

fn fmt_opt(v: Option<f32>, decimals: usize) -> String {
    v.map(|x| format!("{x:.*}", decimals))
        .unwrap_or_else(|| "-".into())
}

pub fn render(groups: &[Group], format: LeaderboardFormat) -> String {
    match format {
        LeaderboardFormat::Json => serde_json::to_string_pretty(groups).unwrap_or_default(),
        LeaderboardFormat::Table => groups.iter().map(render_table).collect::<String>(),
        LeaderboardFormat::Md => groups.iter().map(render_md).collect::<Vec<_>>().join("\n"),
        LeaderboardFormat::Html => render_html(groups),
    }
}

fn render_table(g: &Group) -> String {
    let mut s = format!(
        "{} (sha {}, n={})\n{:<28} {:>7} {:>8} {:>9} {:>14} {:>10} {:>5} {:>8}\n{}\n",
        g.task_set,
        short_sha(&g.sha256),
        g.n,
        "model",
        "samples",
        "quality",
        "accuracy",
        "tokens/correct",
        "cost/1k",
        "n",
        "tool",
        "-".repeat(101),
    );
    for r in &g.rows {
        s.push_str(&format!(
            "{:<28} {:>7} {:>8.1} {:>8.1}% {:>14} {:>10} {:>5} {:>8}\n",
            r.model,
            r.samples,
            r.quality,
            r.accuracy * 100.0,
            fmt_opt(r.tokens_per_correct, 0),
            fmt_opt(r.cost_per_1k_correct, 2),
            r.n_scored,
            r.tool_version,
        ));
    }
    s.push('\n');
    s
}

fn render_md(g: &Group) -> String {
    let mut s = format!(
        "### {} (sha {}, n={})\n\n\
         | model | samples | quality | accuracy | tokens/correct | cost/1k | n | tool |\n\
         |---|---|---|---|---|---|---|---|\n",
        g.task_set,
        short_sha(&g.sha256),
        g.n,
    );
    for r in &g.rows {
        s.push_str(&format!(
            "| {} | {} | {:.1} | {:.1}% | {} | {} | {} | {} |\n",
            r.model,
            r.samples,
            r.quality,
            r.accuracy * 100.0,
            fmt_opt(r.tokens_per_correct, 0),
            fmt_opt(r.cost_per_1k_correct, 2),
            r.n_scored,
            r.tool_version,
        ));
    }
    s
}

fn render_html(groups: &[Group]) -> String {
    let mut s = String::new();
    for g in groups {
        s.push_str(&format!(
            "<h3>{} <small>(sha {}, n={})</small></h3>\n<table>\n\
             <tr><th>model</th><th>samples</th><th>quality</th><th>accuracy</th>\
             <th>tokens/correct</th><th>cost/1k</th><th>n</th><th>tool</th></tr>\n",
            g.task_set,
            short_sha(&g.sha256),
            g.n,
        ));
        for r in &g.rows {
            s.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{:.1}</td><td>{:.1}%</td>\
                 <td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                r.model,
                r.samples,
                r.quality,
                r.accuracy * 100.0,
                fmt_opt(r.tokens_per_correct, 0),
                fmt_opt(r.cost_per_1k_correct, 2),
                r.n_scored,
                r.tool_version,
            ));
        }
        s.push_str("</table>\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_json(model: &str, set: &str, sha: &str, acc: f32, gen: u64, samples: usize) -> String {
        format!(
            r#"{{"schema_version":"1","tool_version":"9.9.9","model":"{model}",
               "endpoint_host":"localhost","generated_at":{gen},
               "task_set":{{"name":"{set}","sha256":"{sha}","n":100}},
               "sampling":{{"temperature":0.7,"max_tokens":512,"samples":{samples}}},
               "metrics":{{"n_attempted":100,"n_scored":100,"n_errored":0,
                 "accuracy":{acc},"mean_quality":55.5,"tokens_per_correct":200.0,
                 "cost_per_1k_correct":null}},
               "results":[]}}"#
        )
    }

    #[test]
    fn parses_a_result_json_into_entry() {
        let e = parse_entry(&entry_json(
            "m",
            "overthinking-v2",
            "abcdef123",
            0.8,
            100,
            4,
        ))
        .unwrap();
        assert_eq!(e.model, "m");
        assert_eq!(e.task_set.name, "overthinking-v2");
        assert_eq!(e.sampling.samples, 4);
        assert!((e.metrics.accuracy - 0.8).abs() < 1e-6);
    }

    #[test]
    fn dedup_keeps_newest_run_per_model() {
        let entries = vec![
            parse_entry(&entry_json("m", "s", "sha1", 0.5, 100, 1)).unwrap(),
            parse_entry(&entry_json("m", "s", "sha1", 0.9, 200, 1)).unwrap(), // newer
        ];
        let groups = assemble(entries, None, SortKey::Accuracy);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].rows.len(), 1, "same model+set+samples deduped");
        assert!(
            (groups[0].rows[0].accuracy - 0.9).abs() < 1e-6,
            "newest wins"
        );
    }

    #[test]
    fn groups_by_task_set_and_sorts_by_accuracy() {
        let entries = vec![
            parse_entry(&entry_json("low", "s", "sha1", 0.30, 1, 1)).unwrap(),
            parse_entry(&entry_json("high", "s", "sha1", 0.90, 1, 1)).unwrap(),
            parse_entry(&entry_json("other", "t", "sha2", 0.50, 1, 1)).unwrap(),
        ];
        let groups = assemble(entries, None, SortKey::Accuracy);
        assert_eq!(groups.len(), 2, "two distinct task sets → two groups");
        let s = groups.iter().find(|g| g.task_set == "s").unwrap();
        assert_eq!(s.rows[0].model, "high", "highest accuracy first");
        assert_eq!(s.rows[1].model, "low");
    }

    #[test]
    fn task_set_filter_restricts_output() {
        let entries = vec![
            parse_entry(&entry_json("a", "s", "sha1", 0.5, 1, 1)).unwrap(),
            parse_entry(&entry_json("b", "t", "sha2", 0.5, 1, 1)).unwrap(),
        ];
        let groups = assemble(entries, Some("t"), SortKey::Accuracy);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].task_set, "t");
    }

    #[test]
    fn different_samples_are_separate_rows() {
        // Same model, same set, but pass@1 and pass@4 are distinct entries.
        let entries = vec![
            parse_entry(&entry_json("m", "s", "sha1", 0.5, 1, 1)).unwrap(),
            parse_entry(&entry_json("m", "s", "sha1", 0.8, 1, 4)).unwrap(),
        ];
        let groups = assemble(entries, None, SortKey::Accuracy);
        assert_eq!(groups[0].rows.len(), 2);
    }

    fn valid_entry() -> Entry {
        Entry {
            schema_version: "1".into(),
            model: "m".into(),
            task_set: TaskSetRef {
                name: "custom-set".into(),
                sha256: "a".repeat(64),
                n: 100,
            },
            tool_version: "0.2.0".into(),
            generated_at: 1,
            sampling: SamplingRef { samples: 1 },
            metrics: MetricsRef {
                n_scored: 10,
                accuracy: 0.5,
                mean_quality: 50.0,
                tokens_per_correct: Some(100.0),
                cost_per_1k_correct: None,
            },
        }
    }

    #[test]
    fn valid_entry_has_no_problems() {
        assert!(validate_entry(&valid_entry()).is_empty());
    }

    #[test]
    fn validate_flags_bad_fields() {
        let mut e = valid_entry();
        e.schema_version = "99".into();
        e.model = "  ".into();
        e.metrics.accuracy = 1.5;
        e.task_set.sha256 = "nothex".into();
        e.sampling.samples = 0;
        let problems = validate_entry(&e);
        assert!(problems.iter().any(|p| p.contains("schema_version")));
        assert!(problems.iter().any(|p| p.contains("model is empty")));
        assert!(problems.iter().any(|p| p.contains("accuracy")));
        assert!(problems.iter().any(|p| p.contains("sha256")));
        assert!(problems.iter().any(|p| p.contains("samples")));
    }

    #[test]
    fn bundled_set_must_carry_its_frozen_sha() {
        // Right name, wrong hash → integrity failure.
        let mut e = valid_entry();
        e.task_set.name = "overthinking-v2".into();
        e.task_set.sha256 = "a".repeat(64);
        e.task_set.n = 100;
        let problems = validate_entry(&e);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("does not match the frozen bundled")),
            "wrong sha for a bundled set must be rejected: {problems:?}"
        );

        // Right name, right hash → accepted.
        let ts = crate::bench::taskset::load("overthinking-v2").unwrap();
        e.task_set.sha256 = ts.sha256;
        assert!(
            validate_entry(&e).is_empty(),
            "matching frozen sha is valid"
        );
    }

    #[test]
    fn n_scored_cannot_exceed_task_set_size() {
        let mut e = valid_entry();
        e.metrics.n_scored = 101;
        assert!(validate_entry(&e)
            .iter()
            .any(|p| p.contains("exceeds task_set.n")));
    }

    #[test]
    fn render_table_has_header_and_rows() {
        let entries = vec![parse_entry(&entry_json("m", "s", "sha1", 0.5, 1, 1)).unwrap()];
        let groups = assemble(entries, None, SortKey::Accuracy);
        let out = render(&groups, LeaderboardFormat::Table);
        assert!(out.contains("model"));
        assert!(out.contains("accuracy"));
        assert!(out.contains("m"));
        assert!(out.contains("sha sha1"));
    }
}
