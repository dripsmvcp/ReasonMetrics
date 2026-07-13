// Python surface for the reasonmetrics scoring engine.
// Thin dict-in/dict-out shim mirroring the wasm crate's JSON contract, so the
// CLI, browser, and Python stay behaviorally identical. All scoring logic
// lives in reasonmetrics-core.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pythonize::{depythonize, pythonize};
use rayon::prelude::*;

use reasonmetrics_core::annotate::annotate as annotate_core;
use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::{build_scorers, Scorer};
use reasonmetrics_core::scoring::score_one_detailed;
use reasonmetrics_core::trace::{extract_thinking, TraceRecord};

fn parse_config(config: Option<&Bound<'_, PyAny>>) -> PyResult<ScoringConfig> {
    match config {
        None => Ok(ScoringConfig::default()),
        Some(obj) => {
            depythonize(obj).map_err(|e| PyValueError::new_err(format!("invalid config: {e}")))
        }
    }
}

/// One record's full analysis — same shape the wasm `analyze()` returns.
fn analyze_to_value(record: &TraceRecord, scorers: &[Box<dyn Scorer>]) -> serde_json::Value {
    let (scored, score_results) = score_one_detailed(record, scorers);
    let extracted = extract_thinking(&record.thinking);
    let annotations = annotate_core(&extracted);
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
    serde_json::json!({
        "scored": scored,
        "extracted_thinking": extracted,
        "annotations": annotations,
        "scores": scores,
    })
}

/// Score one trace record (a dict with problem/thinking/answer — the same
/// field aliases as the CLI apply). Returns the full analysis as a dict.
#[pyfunction]
#[pyo3(signature = (record, config=None))]
fn score(
    py: Python<'_>,
    record: Bound<'_, PyAny>,
    config: Option<Bound<'_, PyAny>>,
) -> PyResult<PyObject> {
    let rec: TraceRecord =
        depythonize(&record).map_err(|e| PyValueError::new_err(format!("invalid record: {e}")))?;
    let cfg = parse_config(config.as_ref())?;
    let scorers = build_scorers(&cfg);
    let value = py.allow_threads(|| analyze_to_value(&rec, &scorers));
    pythonize(py, &value)
        .map(Bound::unbind)
        .map_err(|e| PyValueError::new_err(format!("result conversion failed: {e}")))
}

/// Score a list of records in parallel (the GIL is released while scoring).
#[pyfunction]
#[pyo3(signature = (records, config=None))]
fn score_many(
    py: Python<'_>,
    records: Bound<'_, PyAny>,
    config: Option<Bound<'_, PyAny>>,
) -> PyResult<PyObject> {
    let recs: Vec<TraceRecord> = depythonize(&records)
        .map_err(|e| PyValueError::new_err(format!("invalid records: {e}")))?;
    let cfg = parse_config(config.as_ref())?;
    let scorers = build_scorers(&cfg);
    let values: Vec<serde_json::Value> = py.allow_threads(|| {
        recs.par_iter()
            .map(|r| analyze_to_value(r, &scorers))
            .collect()
    });
    pythonize(py, &values)
        .map(Bound::unbind)
        .map_err(|e| PyValueError::new_err(format!("result conversion failed: {e}")))
}

/// Extract the thinking from raw text (handles <think> tags) and annotate it
/// with restart/verification/repetition spans. Offsets index into
/// `extracted_thinking`, not the raw input.
#[pyfunction]
fn annotate(py: Python<'_>, thinking: &str) -> PyResult<PyObject> {
    let extracted = extract_thinking(thinking);
    let annotations = annotate_core(&extracted);
    let value = serde_json::json!({
        "extracted_thinking": extracted,
        "annotations": annotations,
    });
    pythonize(py, &value)
        .map(Bound::unbind)
        .map_err(|e| PyValueError::new_err(format!("result conversion failed: {e}")))
}

/// The embedded model-family registry as a list of dicts.
#[pyfunction]
fn registry(py: Python<'_>) -> PyResult<PyObject> {
    pythonize(py, reasonmetrics_core::registry::entries())
        .map(Bound::unbind)
        .map_err(|e| PyValueError::new_err(format!("result conversion failed: {e}")))
}

#[pymodule]
fn reasonmetrics(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(score, m)?)?;
    m.add_function(wrap_pyfunction!(score_many, m)?)?;
    m.add_function(wrap_pyfunction!(annotate, m)?)?;
    m.add_function(wrap_pyfunction!(registry, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_to_value_mirrors_wasm_contract() {
        let record: TraceRecord = serde_json::from_str(
            r#"{"id":"1","problem":"2+2?","thinking":"<think>2+2 is 4. Let me verify: 2+2=4.</think>","answer":"4"}"#,
        )
        .unwrap();
        let scorers = build_scorers(&ScoringConfig::default());
        let value = analyze_to_value(&record, &scorers);
        assert!(value["scored"]["quality_score"].is_number());
        assert_eq!(value["scores"].as_array().unwrap().len(), 9);
        assert!(value["annotations"].is_array());
        assert!(value["extracted_thinking"]
            .as_str()
            .unwrap()
            .contains("verify"));
    }
}
