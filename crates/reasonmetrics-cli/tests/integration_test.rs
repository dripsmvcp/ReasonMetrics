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
