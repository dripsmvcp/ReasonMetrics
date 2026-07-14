#![allow(deprecated)] // assert_cmd::cargo_bin deprecation — cosmetic only

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::TempDir;

/// Helper: create a temp dir with a JSONL test file
fn setup_test_data() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let jsonl_path = dir.path().join("test_traces.jsonl");
    let mut file = std::fs::File::create(&jsonl_path).unwrap();

    writeln!(file, r#"{{"id":"1","problem":"What is 2+2?","thinking":"Simple: 2+2=4. Therefore the answer is 4.","answer":"4"}}"#).unwrap();
    writeln!(file, r#"{{"id":"2","problem":"Solve x^2=4","thinking":"Step 1: Take square root of both sides.\n\nStep 2: x = ±2.\n\nLet me verify: 2^2=4 and (-2)^2=4. This confirms both solutions.","answer":"x = ±2"}}"#).unwrap();

    (dir, jsonl_path)
}

#[test]
fn test_score_command_parquet() {
    let (dir, input) = setup_test_data();
    let output = dir.path().join("output.parquet");

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "score",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Parquet file should exist and be non-empty
    assert!(output.exists());
    assert!(output.metadata().unwrap().len() > 0);
}

#[test]
fn test_filter_command() {
    let (dir, input) = setup_test_data();
    let output = dir.path().join("clean.jsonl");

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "filter",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--min-score",
            "0",
        ])
        .assert()
        .success();

    assert!(output.exists());
    let content = std::fs::read_to_string(&output).unwrap();
    // With min-score 0, ALL traces should pass (we have 2 in setup_test_data)
    assert_eq!(
        content.lines().count(),
        2,
        "Expected 2 traces but got {}",
        content.lines().count()
    );
}

/// Build a corpus of `n` traces that score differently, so a rank-based filter
/// has something to rank. Varying the number of verification phrases moves
/// several dimensions at once (verification up, but repetition down once the
/// phrase recurs), so the resulting order is deliberately NOT assumed here —
/// the tests below read the real scores instead of trusting an intuition.
fn setup_ranked_corpus(n: usize) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ranked.jsonl");
    let mut file = std::fs::File::create(&path).unwrap();

    for i in 0..n {
        let verifications = "Let me verify this. Checking the result confirms it. ".repeat(i);
        let thinking = format!(
            "Step 1: set up the equation.\n\nStep 2: solve it.\n\n{verifications}So x = {i}."
        );
        writeln!(
            file,
            r#"{{"id":"t{i}","problem":"Solve for x","thinking":{},"answer":"x = {i}"}}"#,
            serde_json::to_string(&thinking).unwrap()
        )
        .unwrap();
    }
    (dir, path)
}

/// Score `input` and return (id, quality_score) for every trace.
fn score_corpus(dir: &TempDir, input: &std::path::Path) -> Vec<(String, f64)> {
    let scored = dir.path().join("scored.jsonl");
    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "score",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            scored.to_str().unwrap(),
        ])
        .assert()
        .success();

    std::fs::read_to_string(&scored)
        .unwrap()
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            (
                v["id"].as_str().unwrap().to_string(),
                v["quality_score"].as_f64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn test_filter_top_percent_keeps_exactly_that_share() {
    let (dir, input) = setup_ranked_corpus(20);
    let output = dir.path().join("top.jsonl");

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "filter",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--top-percent",
            "30",
        ])
        .assert()
        .success();

    let content = std::fs::read_to_string(&output).unwrap();
    // The point of --top-percent over --min-score: the output size is exact.
    assert_eq!(
        content.lines().count(),
        6,
        "top 30% of 20 traces must be exactly 6, got {}",
        content.lines().count()
    );
}

#[test]
fn test_filter_top_percent_keeps_the_best_traces() {
    let (dir, input) = setup_ranked_corpus(10);
    let output = dir.path().join("top.jsonl");

    let mut scores = score_corpus(&dir, &input);
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "filter",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--top-percent",
            "20",
        ])
        .assert()
        .success();

    let kept: Vec<String> = std::fs::read_to_string(&output)
        .unwrap()
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();

    // The contract of a rank filter: it keeps the highest-scoring traces — not
    // merely the right NUMBER of them. Asserted against the scores the scorer
    // actually produced, so this stays true if the scorer's ordering changes.
    let mut expected: Vec<String> = scores.iter().take(2).map(|(id, _)| id.clone()).collect();
    let mut got = kept.clone();
    expected.sort();
    got.sort();
    assert_eq!(
        got, expected,
        "--top-percent 20 must keep the 2 highest-scoring traces of 10"
    );

    // And nothing it dropped may outrank anything it kept.
    let worst_kept = scores
        .iter()
        .filter(|(id, _)| kept.contains(id))
        .map(|(_, s)| *s)
        .fold(f64::INFINITY, f64::min);
    let best_dropped = scores
        .iter()
        .filter(|(id, _)| !kept.contains(id))
        .map(|(_, s)| *s)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        worst_kept >= best_dropped,
        "a dropped trace ({best_dropped}) outranks a kept one ({worst_kept})"
    );
}

#[test]
fn test_filter_rejects_out_of_range_percent() {
    let (dir, input) = setup_test_data();
    let output = dir.path().join("clean.jsonl");

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "filter",
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--top-percent",
            "150",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("between 0 and 100"));
}

#[test]
fn test_stats_command() {
    let (dir, input) = setup_test_data();

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "stats",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dataset Statistics"));
}

#[test]
fn test_explain_command() {
    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .arg("explain")
        .assert()
        .success()
        .stdout(predicate::str::contains("EFFICIENCY"));
}

#[test]
fn test_init_config_command() {
    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .arg("init-config")
        .assert()
        .success()
        .stdout(predicate::str::contains("[scoring.weights]"));
}

#[test]
fn test_missing_input_file() {
    let dir = TempDir::new().unwrap();
    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "score",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            "nonexistent.jsonl",
            "-o",
            "out.parquet",
        ])
        .assert()
        .failure();
}

#[test]
fn test_report_command() {
    let (dir, input) = setup_test_data();
    let output = dir.path().join("report.html");

    Command::cargo_bin("reasonmetrics")
        .unwrap()
        .args([
            "report",
            "--config",
            dir.path().join("nonexistent.toml").to_str().unwrap(),
            "-i",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let html = std::fs::read_to_string(&output).unwrap();
    assert!(html.contains("reasonmetrics quality report"));
}
