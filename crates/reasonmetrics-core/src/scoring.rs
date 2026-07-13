// Scoring pipeline: pure per-trace scoring shared by the CLI, WASM, and library users.
// I/O, progress reporting, and thread-pool tuning live in the callers.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::config::ScoringConfig;
use crate::scorers::{
    build_scorers, compute_composite, Scorer, ALIGNMENT_IDX, EFFICIENCY_IDX, LANGUAGE_IDX,
    LENGTH_IDX, OVERTHINKING_IDX, REPETITION_IDX, STRUCTURE_IDX, VERIFICATION_IDX,
};
use crate::trace::{
    estimated_token_count, extract_thinking, ScoreResult, ScoredTrace, TraceRecord,
};

/// Score a single trace with an already-built scorer set.
pub fn score_one(trace: &TraceRecord, scorers: &[Box<dyn Scorer>]) -> ScoredTrace {
    score_one_detailed(trace, scorers).0
}

/// Score a single trace, returning both the flattened `ScoredTrace` and the
/// raw per-scorer `ScoreResult`s (name, weight, and diagnostics live on the
/// scorer/result pair, not on `ScoredTrace`). Callers that need per-scorer
/// detail — e.g. the wasm bridge's `scores` array — use this instead of
/// re-scoring the trace a second time.
pub fn score_one_detailed(
    trace: &TraceRecord,
    scorers: &[Box<dyn Scorer>],
) -> (ScoredTrace, Vec<ScoreResult>) {
    let extracted = extract_thinking(&trace.thinking);
    let score_results: Vec<_> = scorers
        .iter()
        .map(|scorer| scorer.score(trace, &extracted))
        .collect();

    let quality_score = compute_composite(&score_results, scorers);

    let word_count = estimated_token_count(&extracted) as u32;

    let restart_count = score_results[EFFICIENCY_IDX]
        .diagnostics
        .iter()
        .find(|(k, _)| k == "restart_count")
        .and_then(|(_, v)| v.parse::<u32>().ok())
        .unwrap_or(0);

    let detected_language = score_results[LANGUAGE_IDX]
        .diagnostics
        .iter()
        .find(|(k, _)| k == "detected_language")
        .and_then(|(_, v)| v.parse::<String>().ok())
        .unwrap_or_default();

    let has_verification = score_results[VERIFICATION_IDX]
        .diagnostics
        .iter()
        .find(|(k, _)| k == "has_verification")
        .map(|(_, v)| v == "true")
        .unwrap_or(false);

    let is_language_mixed = score_results[LANGUAGE_IDX]
        .diagnostics
        .iter()
        .find(|(k, _)| k == "is_mixed")
        .map(|(_, v)| v == "true")
        .unwrap_or(false);

    let answer_in_trace_end = score_results[ALIGNMENT_IDX]
        .diagnostics
        .iter()
        .find(|(k, _)| k == "answer_in_trace_end")
        .map(|(_, v)| v == "true")
        .unwrap_or(false);

    let scored = ScoredTrace {
        id: trace.id.clone(),
        problem: trace.problem.clone(),
        thinking: trace.thinking.clone(),
        answer: trace.answer.clone(),
        quality_score,
        efficiency_score: score_results[EFFICIENCY_IDX].score,
        language_score: score_results[LANGUAGE_IDX].score,
        answer_alignment_score: score_results[ALIGNMENT_IDX].score,
        structural_score: score_results[STRUCTURE_IDX].score,
        repetition_score: score_results[REPETITION_IDX].score,
        overthinking_score: score_results[OVERTHINKING_IDX].score,
        verification_score: score_results[VERIFICATION_IDX].score,
        length_score: score_results[LENGTH_IDX].score,
        thinking_word_count: word_count,
        restart_count,
        detected_language,
        has_self_verification: has_verification,
        is_language_mixed,
        answer_in_trace_end,
    };

    (scored, score_results)
}

#[cfg(feature = "parallel")]
pub fn score_traces(traces: &[TraceRecord], scoring_config: &ScoringConfig) -> Vec<ScoredTrace> {
    let scorers = build_scorers(scoring_config);
    traces
        .par_iter()
        .map(|trace| score_one(trace, &scorers))
        .collect()
}

#[cfg(not(feature = "parallel"))]
pub fn score_traces(traces: &[TraceRecord], scoring_config: &ScoringConfig) -> Vec<ScoredTrace> {
    let scorers = build_scorers(scoring_config);
    traces
        .iter()
        .map(|trace| score_one(trace, &scorers))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_trace(id: &str, thinking: &str) -> TraceRecord {
        TraceRecord {
            id: id.into(),
            problem: "What is 2+2?".into(),
            thinking: thinking.into(),
            answer: "4".into(),
            domain: None,
            source: None,
            expected_answer: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_score_single_trace() {
        let traces = vec![make_trace(
            "1",
            "Let me compute 2+2. It equals 4. Therefore the answer is 4.",
        )];
        let config = ScoringConfig::default();
        let scored = score_traces(&traces, &config);
        assert_eq!(scored.len(), 1);
        assert!(scored[0].quality_score > 0.0);
        assert!(scored[0].quality_score <= 100.0);
    }

    #[test]
    fn test_score_multiple_traces() {
        let traces = vec![
            make_trace("1", "Simple calculation: 2+2=4."),
            make_trace("2", "Wait, let me reconsider. Actually no, let me start over. Hmm let me try again. The answer is 4."),
        ];
        let config = ScoringConfig::default();
        let scored = score_traces(&traces, &config);
        assert_eq!(scored.len(), 2);
        // The clean trace should score higher than the restart-heavy one
        assert!(
            scored[0].quality_score > scored[1].quality_score,
            "Clean trace ({}) should beat restart-heavy ({})",
            scored[0].quality_score,
            scored[1].quality_score
        );
    }

    #[test]
    fn test_score_one_matches_batch() {
        let trace = make_trace("1", "Compute 2+2 = 4. Let me verify: 4 - 2 = 2. Correct.");
        let config = ScoringConfig::default();
        let scorers = build_scorers(&config);
        let single = score_one(&trace, &scorers);
        let batch = score_traces(std::slice::from_ref(&trace), &config);
        assert_eq!(single.quality_score, batch[0].quality_score);
    }

    #[test]
    fn test_score_one_detailed_matches_score_one() {
        let trace = make_trace("1", "Compute 2+2 = 4. Let me verify: 4 - 2 = 2. Correct.");
        let config = ScoringConfig::default();
        let scorers = build_scorers(&config);
        let (scored, score_results) = score_one_detailed(&trace, &scorers);
        let plain = score_one(&trace, &scorers);
        assert_eq!(scored.quality_score, plain.quality_score);
        assert_eq!(score_results.len(), scorers.len());
        assert_eq!(score_results[EFFICIENCY_IDX].score, scored.efficiency_score);
    }
}
