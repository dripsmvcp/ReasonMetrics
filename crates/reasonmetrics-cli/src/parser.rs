//Streaming JSONL reader

use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use tracing::{info, warn};

use reasonmetrics_core::errors::ReasonMetricsError;
use reasonmetrics_core::trace::TraceRecord;

pub fn read_jsonl(path: &Path, strict: bool) -> Result<Vec<TraceRecord>, ReasonMetricsError> {
    let file = File::open(path).map_err(|e| ReasonMetricsError::IoRead {
        path: path.to_path_buf(),
        source: e,
    })?;

    let is_gzip = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("gz"))
        .unwrap_or(false);

    let reader: Box<dyn Read> = if is_gzip {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let buf_reader = BufReader::new(reader);
    let mut traces = Vec::new();
    let mut error_count = 0usize;
    let mut line_number = 0usize;

    for line_result in buf_reader.lines() {
        line_number += 1;

        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                if strict {
                    return Err(ReasonMetricsError::IoRead {
                        path: path.to_path_buf(),
                        source: e,
                    });
                }
                warn!("I/O error on line {}: {}", line_number, e);
                error_count += 1;
                continue;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<TraceRecord>(trimmed) {
            Ok(trace) => traces.push(trace),
            Err(e) => {
                if strict {
                    return Err(ReasonMetricsError::JsonParse {
                        line_number,
                        message: e.to_string(),
                    });
                }
                warn!("Skipping line {}: {}", line_number, e);
                error_count += 1;
            }
        }
    }

    if error_count > 0 {
        warn!(
            "Parsed {} traces with {} skipped lines",
            traces.len(),
            error_count
        );
    } else {
        info!("Parsed {} traces from {}", traces.len(), path.display());
    }

    if traces.is_empty() {
        return Err(ReasonMetricsError::EmptyInput {
            path: path.to_path_buf(),
        });
    }

    Ok(traces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn temp_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f.flush().unwrap();
        f
    }

    #[test]
    fn test_valid_jsonl() {
        let f = temp_jsonl(&[
            r#"{"id":"1","problem":"Q","thinking":"T","answer":"A"}"#,
            r#"{"id":"2","problem":"Q","thinking":"T","answer":"A"}"#,
        ]);
        let traces = read_jsonl(f.path(), false).unwrap();
        assert_eq!(traces.len(), 2);
    }

    #[test]
    fn test_aliases() {
        let f = temp_jsonl(&[r#"{"id":"1","question":"Q","reasoning":"R","solution":"A"}"#]);
        let t = read_jsonl(f.path(), false).unwrap();
        assert_eq!(t[0].problem, "Q");
        assert_eq!(t[0].thinking, "R");
    }

    #[test]
    fn test_skip_bad_lines() {
        let f = temp_jsonl(&[
            r#"{"id":"1","problem":"Q","thinking":"T","answer":"A"}"#,
            "not json",
            r#"{"id":"3","problem":"Q","thinking":"T","answer":"A"}"#,
        ]);
        assert_eq!(read_jsonl(f.path(), false).unwrap().len(), 2);
    }

    #[test]
    fn test_strict_fails() {
        let f = temp_jsonl(&["not json"]);
        assert!(read_jsonl(f.path(), true).is_err());
    }

    #[test]
    fn test_empty_file() {
        let f = temp_jsonl(&[]);
        assert!(read_jsonl(f.path(), false).is_err());
    }
}
