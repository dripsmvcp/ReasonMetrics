//! Embedded model/format registry.
//!
//! Each `registry/*.toml` at the repo root describes one model family: how to
//! extract its thinking (tag pairs and/or structured fields), optional cost
//! and tokenizer heuristics, and per-language lexicon additions. Entries are
//! embedded at build time so every surface (CLI, wasm, future bindings) ships
//! the same data, and the test harness in this module is the CI gate that
//! makes a registry PR without a working fixture unmergeable.
//!
//! Lexicons are data-only for now: they are carried and exposed, but not yet
//! merged into the scorers — that switch is gated on calibration evidence.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::LazyLock;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/registry_gen.rs"));
}
pub use generated::RAW_ENTRIES;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryEntry {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub extraction: Extraction,
    #[serde(default)]
    pub cost: Option<Cost>,
    #[serde(default)]
    pub heuristics: Option<Heuristics>,
    /// Language code → phrase additions (e.g. `lexicon.zh`).
    #[serde(default)]
    pub lexicon: BTreeMap<String, Lexicon>,
    /// Path relative to `registry/` proving extraction works; required.
    pub fixture: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    /// Ordered `[start, end]` tag pairs tried against raw text; first hit wins.
    #[serde(default)]
    pub think_tags: Vec<(String, String)>,
    /// Top-level JSON fields that hold the reasoning in structured records.
    #[serde(default)]
    pub reasoning_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    /// USD per 1M input tokens.
    pub input_per_mtok: f64,
    /// USD per 1M output tokens (thinking bills as output).
    pub output_per_mtok: f64,
    /// Where and when the numbers were checked — keeps stale pricing auditable.
    pub source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Heuristics {
    pub tokens_per_char: f64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lexicon {
    #[serde(default)]
    pub restart: Vec<String>,
    #[serde(default)]
    pub verification: Vec<String>,
}

/// Parse a single registry TOML document.
pub fn parse_entry(raw: &str) -> Result<RegistryEntry, toml::de::Error> {
    toml::from_str(raw)
}

static ENTRIES: LazyLock<Vec<RegistryEntry>> = LazyLock::new(|| {
    RAW_ENTRIES
        .iter()
        .filter_map(|(_, raw)| parse_entry(raw).ok())
        .collect()
});

/// All embedded entries. Invalid files are skipped here but fail the test
/// harness, so outside a broken build this is every file in `registry/`.
pub fn entries() -> &'static [RegistryEntry] {
    &ENTRIES
}

pub fn lookup(id: &str) -> Option<&'static RegistryEntry> {
    entries().iter().find(|e| e.id == id)
}

/// Extract thinking from raw text using the entry's tag pairs.
pub fn extract_thinking(entry: &RegistryEntry, raw: &str) -> Option<String> {
    for (start, end) in &entry.extraction.think_tags {
        if let Some(s) = raw.find(start.as_str()) {
            let after = &raw[s + start.len()..];
            if let Some(e) = after.find(end.as_str()) {
                return Some(after[..e].trim().to_string());
            }
        }
    }
    None
}

/// Extract thinking from a structured record via the entry's top-level fields.
pub fn extract_field_thinking(entry: &RegistryEntry, value: &serde_json::Value) -> Option<String> {
    for field in &entry.extraction.reasoning_fields {
        if let Some(s) = value.get(field).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_not_empty() {
        assert!(
            !RAW_ENTRIES.is_empty(),
            "no registry entries embedded — registry/ missing or build.rs glob broken"
        );
    }

    #[test]
    fn every_embedded_entry_parses() {
        for (name, raw) in RAW_ENTRIES {
            parse_entry(raw).unwrap_or_else(|e| panic!("registry/{name} failed to parse: {e}"));
        }
    }

    #[test]
    fn ids_match_filenames_and_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for (name, raw) in RAW_ENTRIES {
            let entry = parse_entry(raw).unwrap();
            let stem = name.trim_end_matches(".toml");
            assert_eq!(
                entry.id, stem,
                "registry/{name}: id must match the file name"
            );
            assert!(
                seen.insert(entry.id.clone()),
                "duplicate registry id {}",
                entry.id
            );
        }
    }

    /// The CI gate for contributor PRs: every entry must ship a fixture that
    /// its own extraction config actually works on.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn every_fixture_exists_and_extraction_matches() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../registry");
        for (name, raw) in RAW_ENTRIES {
            let entry = parse_entry(raw).unwrap();
            let fixture_path = root.join(&entry.fixture);
            let text = std::fs::read_to_string(&fixture_path).unwrap_or_else(|e| {
                panic!("registry/{name}: fixture {} unreadable: {e}", entry.fixture)
            });
            let fixture: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("registry/{name}: fixture is not valid JSON: {e}"));

            let mut exercised = false;
            if let Some(input) = fixture.get("input").and_then(|v| v.as_str()) {
                let expected = fixture
                    .get("expected_thinking")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        panic!("registry/{name}: fixture has input but no expected_thinking")
                    });
                let got = extract_thinking(&entry, input).unwrap_or_else(|| {
                    panic!("registry/{name}: think_tags failed to extract from fixture input")
                });
                assert_eq!(got, expected, "registry/{name}: tag extraction mismatch");
                exercised = true;
            }
            if let Some(input_json) = fixture.get("input_json") {
                let expected = fixture
                    .get("expected_field_thinking")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        panic!(
                            "registry/{name}: fixture has input_json but no expected_field_thinking"
                        )
                    });
                let got = extract_field_thinking(&entry, input_json).unwrap_or_else(|| {
                    panic!("registry/{name}: reasoning_fields failed on fixture input_json")
                });
                assert_eq!(got, expected, "registry/{name}: field extraction mismatch");
                exercised = true;
            }
            assert!(
                exercised,
                "registry/{name}: fixture exercises neither tag nor field extraction"
            );
        }
    }
}
