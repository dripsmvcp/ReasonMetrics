use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
pub fn write_jsonl<T: Serialize>(items: &[T], path: &Path) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    for item in items {
        // serde_json::to_string converts a Rust struct to a JSON string
        let json = serde_json::to_string(item)?;
        writeln!(writer, "{}", json)?;
    }

    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reasonmetrics_core::trace::TraceRecord;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_jsonl() {
        let traces = vec![TraceRecord {
            id: "1".into(),
            problem: "Q".into(),
            thinking: "T".into(),
            answer: "A".into(),
            domain: None,
            source: None,
            expected_answer: None,
            extra: HashMap::new(),
        }];
        let tmp = NamedTempFile::new().unwrap();
        write_jsonl(&traces, tmp.path()).unwrap();

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("\"id\":\"1\""));
    }
}
