// WebAssembly surface for the reasonmetrics scoring engine.
// Keep this crate a thin JSON-in/JSON-out shim: all scoring logic lives in
// reasonmetrics-core so the CLI and browser stay behaviorally identical.

use wasm_bindgen::prelude::*;

use reasonmetrics_core::annotate::annotate;
use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::build_scorers;
use reasonmetrics_core::scoring::score_one_detailed;
use reasonmetrics_core::trace::{extract_thinking, TraceRecord};

/// Pure implementation, testable on any target.
pub fn analyze_impl(trace_json: &str) -> Result<String, String> {
    let record: TraceRecord =
        serde_json::from_str(trace_json).map_err(|e| format!("invalid trace JSON: {e}"))?;
    let config = ScoringConfig::default();
    let scorers = build_scorers(&config);
    // score_one_detailed scores once and hands back both the flattened
    // ScoredTrace and the raw per-scorer results, so the `scores` array
    // below doesn't require re-scoring the trace.
    let (scored, score_results) = score_one_detailed(&record, &scorers);
    // Annotation offsets index into the extracted thinking, so return it too:
    // the UI must highlight against this exact string, not the raw input.
    let extracted = extract_thinking(&record.thinking);
    let annotations = annotate(&extracted);
    let scores: Vec<_> = scorers
        .iter()
        .zip(score_results.iter())
        .map(|(scorer, result)| {
            serde_json::json!({
                "name": scorer.name(),
                "score": result.score,
                "weight": scorer.weight(),
                "diagnostics": result.diagnostics,
            })
        })
        .collect();
    let result = serde_json::json!({
        "scored": scored,
        "extracted_thinking": extracted,
        "annotations": annotations,
        "scores": scores,
    });
    serde_json::to_string(&result).map_err(|e| format!("serialization failed: {e}"))
}

/// Analyze a single trace record (JSON object) and return the scored trace as JSON.
#[wasm_bindgen]
pub fn analyze(trace_json: &str) -> Result<String, JsError> {
    analyze_impl(trace_json).map_err(|e| JsError::new(&e))
}

/// Pure implementation, testable on any target.
pub fn registry_json_impl() -> String {
    serde_json::to_string(reasonmetrics_core::registry::entries()).unwrap_or_else(|_| "[]".into())
}

/// The embedded model-family registry (extraction formats, costs, lexicons)
/// as a JSON array — lets the web app share one source of truth with the CLI.
#[wasm_bindgen]
pub fn registry_json() -> String {
    registry_json_impl()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_minimal_trace_natively() {
        let json = r#"{"id":"1","problem":"2+2?","thinking":"<think>2+2 is 4. Let me verify: 2+2=4.</think>","answer":"4"}"#;
        let out = analyze_impl(json).unwrap();
        assert!(out.contains("\"quality_score\""));
        assert!(out.contains("\"efficiency_score\""));
        assert!(out.contains("\"annotations\""));
        assert!(out.contains("\"verification\""));
    }

    #[test]
    fn includes_nine_per_scorer_score_entries() {
        let json = r#"{"id":"1","problem":"2+2?","thinking":"<think>2+2 is 4. Let me verify: 2+2=4.</think>","answer":"4"}"#;
        let out = analyze_impl(json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let scores = parsed["scores"].as_array().expect("scores is an array");
        assert_eq!(scores.len(), 9, "one entry per registered scorer");
        for entry in scores {
            assert!(entry["name"].is_string());
            assert!(entry["score"].is_number());
            assert!(entry["weight"].is_number());
            assert!(entry["diagnostics"].is_array());
        }
    }

    #[test]
    fn rejects_malformed_json() {
        let err = analyze_impl("not json").unwrap_err();
        assert!(err.contains("invalid trace JSON"));
    }

    #[test]
    fn accepts_field_aliases() {
        let json = r#"{"id":7,"question":"Q","reasoning":"R is the reasoning.","solution":"A"}"#;
        assert!(analyze_impl(json).is_ok());
    }

    #[test]
    fn registry_json_is_a_populated_array() {
        let parsed: serde_json::Value = serde_json::from_str(&registry_json_impl()).unwrap();
        let entries = parsed.as_array().expect("registry_json is an array");
        assert!(!entries.is_empty(), "registry should ship seed entries");
        for entry in entries {
            assert!(entry["id"].is_string());
            assert!(entry["display_name"].is_string());
        }
    }
}
