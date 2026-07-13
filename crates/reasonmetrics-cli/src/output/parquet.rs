use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow::array::{ArrayRef, BooleanArray, Float32Array, StringArray, UInt32Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use reasonmetrics_core::trace::ScoredTrace;

pub fn write_parquet(scored: &[ScoredTrace], path: &Path) -> anyhow::Result<()> {
    // Step 1: Define the schema (column names and types)
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("problem", DataType::Utf8, false),
        Field::new("thinking", DataType::Utf8, false),
        Field::new("answer", DataType::Utf8, false),
        Field::new("quality_score", DataType::Float32, false),
        Field::new("efficiency_score", DataType::Float32, false),
        Field::new("language_score", DataType::Float32, false),
        Field::new("answer_alignment_score", DataType::Float32, false),
        Field::new("structural_score", DataType::Float32, false),
        Field::new("repetition_score", DataType::Float32, false),
        Field::new("overthinking_score", DataType::Float32, false),
        Field::new("verification_score", DataType::Float32, false),
        Field::new("length_score", DataType::Float32, false),
        Field::new("thinking_word_count", DataType::UInt32, false),
        Field::new("restart_count", DataType::UInt32, false),
        Field::new("detected_language", DataType::Utf8, false),
        Field::new("has_self_verification", DataType::Boolean, false),
        Field::new("is_language_mixed", DataType::Boolean, false),
        Field::new("answer_in_trace_end", DataType::Boolean, false),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(
            scored.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            scored
                .iter()
                .map(|s| s.problem.as_str())
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            scored
                .iter()
                .map(|s| s.thinking.as_str())
                .collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            scored.iter().map(|s| s.answer.as_str()).collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored.iter().map(|s| s.quality_score).collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.efficiency_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored.iter().map(|s| s.language_score).collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.answer_alignment_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.structural_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.repetition_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.overthinking_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored
                .iter()
                .map(|s| s.verification_score)
                .collect::<Vec<_>>(),
        )),
        Arc::new(Float32Array::from(
            scored.iter().map(|s| s.length_score).collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from(
            scored
                .iter()
                .map(|s| s.thinking_word_count)
                .collect::<Vec<_>>(),
        )),
        Arc::new(UInt32Array::from(
            scored.iter().map(|s| s.restart_count).collect::<Vec<_>>(),
        )),
        Arc::new(StringArray::from(
            scored
                .iter()
                .map(|s| s.detected_language.as_str())
                .collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            scored
                .iter()
                .map(|s| s.has_self_verification)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            scored
                .iter()
                .map(|s| s.is_language_mixed)
                .collect::<Vec<_>>(),
        )),
        Arc::new(BooleanArray::from(
            scored
                .iter()
                .map(|s| s.answer_in_trace_end)
                .collect::<Vec<_>>(),
        )),
    ];

    let batch = RecordBatch::try_new(schema.clone(), columns)?;
    let file = File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();

    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn make_scored() -> ScoredTrace {
        ScoredTrace {
            id: "test_1".into(),
            problem: "Q".into(),
            thinking: "T".into(),
            answer: "A".into(),
            quality_score: 75.0,
            efficiency_score: 80.0,
            language_score: 100.0,
            answer_alignment_score: 70.0,
            structural_score: 60.0,
            repetition_score: 90.0,
            overthinking_score: 85.0,
            verification_score: 65.0,
            length_score: 100.0,
            thinking_word_count: 500,
            restart_count: 2,
            detected_language: "Eng".into(),
            has_self_verification: true,
            is_language_mixed: false,
            answer_in_trace_end: true,
        }
    }

    #[test]
    fn test_write_parquet() {
        let scored = vec![make_scored()];
        let tmp = NamedTempFile::new().unwrap();
        let result = write_parquet(&scored, tmp.path());
        assert!(result.is_ok(), "Parquet write failed: {:?}", result.err());
        // Check the file is not empty
        assert!(tmp.path().metadata().unwrap().len() > 0);
    }
}
