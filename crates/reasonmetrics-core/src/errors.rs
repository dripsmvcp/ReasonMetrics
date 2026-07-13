use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReasonMetricsError {
    #[error("Failed to read file '{path}': {source}")]
    IoRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to write file '{path}': {source}")]
    IoWrite {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Invalid JSON on line {line_number}: {message}")]
    JsonParse { line_number: usize, message: String },

    #[error("No valid traces found in '{path}'")]
    EmptyInput { path: PathBuf },

    #[error("Failed to read config '{path}': {error}")]
    ConfigReadError {
        path: PathBuf,
        error: std::io::Error,
    },

    #[error("Failed to parse config '{path}': {error}")]
    ConfigParseError {
        path: PathBuf,
        error: toml::de::Error,
    },

    #[error("Invalid configuration: {message}")]
    ConfigError { message: String },

    #[error("Scorer weights must each be between 0.0 and 1.0 and sum to 1.0, got sum {sum:.3}")]
    InvalidWeights { sum: f32 },

    #[error("Parquet write error: {message}")]
    ParquetError { message: String },
}

pub type Result<T> = std::result::Result<T, ReasonMetricsError>;
