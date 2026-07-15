//Configuration loading with sensible defaults

//If no config file exists, defaults are used.

use crate::errors::ReasonMetricsError;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub input: InputConfig,
    pub scoring: ScoringConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub strict: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ScoringConfig {
    pub weights: ScorerWeights,
    pub efficiency: EfficiencyConfig,
    pub language: LanguageConfig,
    pub length: LengthConfig,
    pub accuracy_efficiency: AccuracyEfficiencyConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ScorerWeights {
    pub efficiency: f32,
    pub language: f32,
    pub alignment: f32,
    pub structure: f32,
    pub repetition: f32,
    pub overthinking: f32,
    pub verification: f32,
    pub length: f32,
    /// Ground-truth-gated scorer; 0.0 by default so unlabeled datasets keep
    /// the upstream composite. Rebalance the other weights when raising it.
    pub accuracy_efficiency: f32,
}

impl Default for ScorerWeights {
    fn default() -> Self {
        Self {
            efficiency: 0.20,
            language: 0.12,
            alignment: 0.18,
            structure: 0.10,
            repetition: 0.15,
            overthinking: 0.10,
            verification: 0.08,
            length: 0.07,
            accuracy_efficiency: 0.0,
        }
    }
}
impl ScorerWeights {
    //returns weights in scorer order (must match build_scorers() order)
    pub fn as_array(&self) -> [f32; 9] {
        [
            self.efficiency,
            self.language,
            self.alignment,
            self.structure,
            self.repetition,
            self.overthinking,
            self.verification,
            self.length,
            self.accuracy_efficiency,
        ]
    }
    //check if weights are finite, in range, and sum to ~1.0
    pub fn validate(&self) -> std::result::Result<(), ReasonMetricsError> {
        let weights = self.as_array();
        let sum: f32 = weights.iter().sum();
        let has_invalid_weight = weights
            .iter()
            .any(|w| !w.is_finite() || *w < 0.0 || *w > 1.0);
        //NaN comparision always returns false, so we must check is_finite() first.
        if has_invalid_weight || !sum.is_finite() || (sum - 1.0).abs() > 0.001 {
            Err(ReasonMetricsError::InvalidWeights { sum })
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EfficiencyConfig {
    pub restart_penalty_per_1k: f32,
}
impl Default for EfficiencyConfig {
    fn default() -> Self {
        Self {
            restart_penalty_per_1k: 8.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LanguageConfig {
    pub num_chunks: usize,
    pub min_words_per_chunk: usize,
}
impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            num_chunks: 10,
            min_words_per_chunk: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LengthConfig {
    pub sweet_spot_min: usize,
    pub sweet_spot_max: usize,
}
impl Default for LengthConfig {
    fn default() -> Self {
        Self {
            sweet_spot_min: 200,
            sweet_spot_max: 3000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AccuracyEfficiencyConfig {
    pub token_min: usize,
    pub token_max: usize,
}
impl Default for AccuracyEfficiencyConfig {
    fn default() -> Self {
        Self {
            token_min: 50,
            token_max: 5000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Parquet,
    Jsonl,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    pub min_score: f32,
    pub format: OutputFormat,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            min_score: 70.0,
            format: OutputFormat::Parquet,
        }
    }
}

impl Config {
    /// Load from TOML file, falling back to defaults if file missing.
    pub fn load(path: &Path) -> std::result::Result<Self, ReasonMetricsError> {
        if path.exists() {
            let content =
                std::fs::read_to_string(path).map_err(|e| ReasonMetricsError::ConfigReadError {
                    path: path.to_path_buf(),
                    error: e,
                })?;
            let config: Config =
                toml::from_str(&content).map_err(|e| ReasonMetricsError::ConfigParseError {
                    path: path.to_path_buf(),
                    error: e,
                })?;
            config.scoring.weights.validate()?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn default_toml() -> String {
        r#"# reasonmetrics.toml
[input]
strict = false

[scoring.weights]
efficiency = 0.20
language = 0.12
alignment = 0.18
structure = 0.10
repetition = 0.15
overthinking = 0.10
verification = 0.08
length = 0.07
accuracy_efficiency = 0.0   # needs expected_answer ground truth; rebalance others when raising

[scoring.efficiency]
restart_penalty_per_1k = 8.0

[scoring.accuracy_efficiency]
token_min = 50
token_max = 5000

[scoring.language]
num_chunks = 10
min_words_per_chunk = 20

[scoring.length]
sweet_spot_min = 200
sweet_spot_max = 3000

[output]
min_score = 70.0      # Used by `filter` when --min-score is omitted
format = "parquet"    # Used by `score` when -o/--output is omitted
"#
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights_sum_to_one() {
        assert!(ScorerWeights::default().validate().is_ok());
    }

    #[test]
    fn spec_v1_default_weights_are_frozen() {
        // These are the SPEC 1.0.0 frozen default weights (see SPEC.md §3).
        // Changing any of them changes published scores, so it is a breaking
        // change: update SPEC.md's version and changelog in the same PR, then
        // update this test. It exists so the two cannot drift silently.
        let w = ScorerWeights::default();
        assert_eq!(w.efficiency, 0.20);
        assert_eq!(w.language, 0.12);
        assert_eq!(w.alignment, 0.18);
        assert_eq!(w.structure, 0.10);
        assert_eq!(w.repetition, 0.15);
        assert_eq!(w.overthinking, 0.10);
        assert_eq!(w.verification, 0.08);
        assert_eq!(w.length, 0.07);
        assert_eq!(w.accuracy_efficiency, 0.0);
    }

    #[test]
    fn test_invalid_weights_detected() {
        let w = ScorerWeights {
            efficiency: 0.50,
            ..ScorerWeights::default()
        };
        assert!(w.validate().is_err());
    }

    #[test]
    fn test_out_of_range_weights_detected_even_if_sum_is_one() {
        let w = ScorerWeights {
            efficiency: 0.33,
            language: -0.01,
            ..ScorerWeights::default()
        };
        assert!(w.validate().is_err());
    }

    #[test]
    fn test_partial_toml() {
        let toml_str = r#"
[scoring.weights]
efficiency = 0.25
language = 0.12
alignment = 0.18
structure = 0.10
repetition = 0.15
overthinking = 0.10
verification = 0.05
length = 0.05
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scoring.weights.efficiency, 0.25);
        assert!(!config.input.strict); // default preserved
    }
}
