use crate::config::ScoringConfig;
use crate::trace::{ScoreResult, TraceRecord};

pub mod accuracy_efficiency;
pub mod alignment;
pub mod efficiency;
pub mod language;
pub mod length;
pub mod overthinking;
pub mod repetition;
pub mod structure;
pub mod verification;

pub const EFFICIENCY_IDX: usize = 0;
pub const LANGUAGE_IDX: usize = 1;
pub const ALIGNMENT_IDX: usize = 2;
pub const STRUCTURE_IDX: usize = 3;
pub const REPETITION_IDX: usize = 4;
pub const OVERTHINKING_IDX: usize = 5;
pub const VERIFICATION_IDX: usize = 6;
pub const LENGTH_IDX: usize = 7;
pub const ACCURACY_EFFICIENCY_IDX: usize = 8;

pub trait Scorer: Send + Sync {
    fn name(&self) -> &str;
    fn weight(&self) -> f32;
    fn score(&self, trace: &TraceRecord, extracted_thinking: &str) -> ScoreResult;
}

pub fn build_scorers(config: &ScoringConfig) -> Vec<Box<dyn Scorer>> {
    let w = &config.weights;
    vec![
        Box::new(efficiency::EfficiencyScorer::new(
            w.efficiency,
            config.efficiency.restart_penalty_per_1k,
        )),
        Box::new(language::LanguageScorer::new(
            w.language,
            config.language.num_chunks,
            config.language.min_words_per_chunk,
        )),
        Box::new(alignment::AlignmentScorer::new(w.alignment)),
        Box::new(structure::StructureScorer::new(w.structure)),
        Box::new(repetition::RepetitionScorer::new(w.repetition)),
        Box::new(overthinking::OverthinkingScorer::new(w.overthinking)),
        Box::new(verification::VerificationScorer::new(w.verification)),
        Box::new(length::LengthScorer::new(
            w.length,
            config.length.sweet_spot_min,
            config.length.sweet_spot_max,
        )),
        Box::new(accuracy_efficiency::AccuracyEfficiencyScorer::new(
            w.accuracy_efficiency,
            config.accuracy_efficiency.token_min,
            config.accuracy_efficiency.token_max,
        )),
    ]
}

pub fn compute_composite(scores: &[ScoreResult], scorers: &[Box<dyn Scorer>]) -> f32 {
    assert_eq!(scores.len(), scorers.len(), "score/scorer count mismatch");

    let raw: f32 = scores
        .iter()
        .zip(scorers.iter())
        .map(|(result, scorer)| result.score * scorer.weight())
        .sum();
    raw.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_build_scorers_matches_weight_count() {
        let config = Config::default();
        let scorers = build_scorers(&config.scoring);

        assert_eq!(scorers.len(), config.scoring.weights.as_array().len());
    }

    #[test]
    fn test_compute_composite_respects_weights() {
        let config = Config::default();
        let scorers = build_scorers(&config.scoring);
        let scores = scorers
            .iter()
            .map(|_| ScoreResult::new(100.0))
            .collect::<Vec<_>>();

        assert_eq!(compute_composite(&scores, &scorers), 100.0);
    }
}
