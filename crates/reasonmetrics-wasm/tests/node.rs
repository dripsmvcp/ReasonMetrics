// Runs under `wasm-pack test --node`: exercises the actual wasm-bindgen surface.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn analyzes_minimal_trace() {
    let json = r#"{"id":"1","problem":"2+2?","thinking":"<think>2+2 is 4. Let me verify: 2+2=4.</think>","answer":"4"}"#;
    let out = reasonmetrics_wasm::analyze(json)
        .ok()
        .expect("analyze should succeed");
    assert!(out.contains("\"quality_score\""));
}

#[wasm_bindgen_test]
fn analyze_includes_nine_per_scorer_entries() {
    let json = r#"{"id":"1","problem":"2+2?","thinking":"<think>2+2 is 4. Let me verify: 2+2=4.</think>","answer":"4"}"#;
    let out = reasonmetrics_wasm::analyze(json)
        .ok()
        .expect("analyze should succeed");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let scores = parsed["scores"].as_array().expect("scores is an array");
    assert_eq!(scores.len(), 9, "one entry per registered scorer");
    for entry in scores {
        assert!(entry["name"].is_string());
        assert!(entry["score"].is_number());
        assert!(entry["weight"].is_number());
        assert!(entry["diagnostics"].is_array());
    }
}

#[wasm_bindgen_test]
fn malformed_json_is_an_error() {
    assert!(reasonmetrics_wasm::analyze("{oops").is_err());
}
