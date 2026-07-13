//! Adversarial fixtures: traces engineered to game one scorer each.
//!
//! These tests PIN the failure modes documented in docs/LIMITATIONS.md — they
//! assert that the gaming WORKS. When a scorer improvement closes a hole, its
//! assertion here fails on purpose: update the fixture's row in LIMITATIONS.md
//! and the expectation together, so the doc never silently drifts from the code.

use reasonmetrics_core::config::ScoringConfig;
use reasonmetrics_core::scorers::build_scorers;
use reasonmetrics_core::scoring::score_one_detailed;
use reasonmetrics_core::trace::{ScoredTrace, TraceRecord};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct DiagExpect {
    scorer: String,
    key: String,
    min: f64,
}

#[derive(Deserialize)]
struct Fixture {
    gamed_dimension: String,
    why: String,
    trace: TraceRecord,
    #[serde(default)]
    expect_min: BTreeMap<String, f32>,
    #[serde(default)]
    expect_diag: Vec<DiagExpect>,
}

fn dimension(scored: &ScoredTrace, field: &str) -> f32 {
    match field {
        "quality_score" => scored.quality_score,
        "efficiency_score" => scored.efficiency_score,
        "language_score" => scored.language_score,
        "answer_alignment_score" => scored.answer_alignment_score,
        "structural_score" => scored.structural_score,
        "repetition_score" => scored.repetition_score,
        "overthinking_score" => scored.overthinking_score,
        "verification_score" => scored.verification_score,
        "length_score" => scored.length_score,
        other => panic!("unknown dimension in expect_min: {other}"),
    }
}

#[test]
fn adversarial_fixtures_still_game_their_scorers() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/adversarial");
    let config = ScoringConfig::default();
    let scorers = build_scorers(&config);

    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("adversarial fixture dir exists")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    assert!(paths.len() >= 7, "expected the 7 adversarial fixtures");

    for path in paths {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let fixture: Fixture = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("{name}: bad fixture JSON: {e}"));
        let (scored, results) = score_one_detailed(&fixture.trace, &scorers);

        eprintln!(
            "{name}: quality={:.0} eff={:.0} lang={:.0} align={:.0} struct={:.0} rep={:.0} over={:.0} verif={:.0} len={:.0}",
            scored.quality_score,
            scored.efficiency_score,
            scored.language_score,
            scored.answer_alignment_score,
            scored.structural_score,
            scored.repetition_score,
            scored.overthinking_score,
            scored.verification_score,
            scored.length_score,
        );

        for (field, min) in &fixture.expect_min {
            let got = dimension(&scored, field);
            assert!(
                got >= *min,
                "{name} (games {}): expected {field} >= {min}, got {got}.\n\
                 Failure mode: {}\n\
                 If a scorer fix closed this hole — good! Update docs/LIMITATIONS.md \
                 and this fixture's expectation together.",
                fixture.gamed_dimension,
                fixture.why
            );
        }

        for d in &fixture.expect_diag {
            let idx = scorers
                .iter()
                .position(|s| s.name() == d.scorer)
                .unwrap_or_else(|| panic!("{name}: no scorer named {}", d.scorer));
            let raw = results[idx]
                .diagnostics
                .iter()
                .find(|(k, _)| k == &d.key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("{name}: {} has no diagnostic {}", d.scorer, d.key));
            let val: f64 = raw
                .parse()
                .unwrap_or_else(|_| panic!("{name}: diagnostic {} = {raw} not numeric", d.key));
            assert!(
                val >= d.min,
                "{name}: expected {}.{} >= {}, got {val} — see docs/LIMITATIONS.md",
                d.scorer,
                d.key,
                d.min
            );
        }
    }
}
