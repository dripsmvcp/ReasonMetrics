// Span-level annotations over an extracted thinking string.
// Powers UI highlighting: restart loops, self-verification, repeated sentences.
// Offsets are byte offsets into the input and always fall on char boundaries.

use serde::Serialize;
use std::collections::HashMap;

use crate::scorers::efficiency::RESTART_REGEXES;
use crate::scorers::verification::EXPLICIT_VERIFICATION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationKind {
    Restart,
    Verification,
    Repetition,
}

#[derive(Debug, Clone, Serialize)]
pub struct Annotation {
    pub start: usize,
    pub end: usize,
    pub kind: AnnotationKind,
    pub note: String,
}

/// Lowercase copy of `text` plus, per shadow byte, the start and end byte
/// offsets of the originating char in `text`. Lowercasing can change byte
/// lengths (and even char counts), so matches found in the shadow must be
/// mapped back through these tables instead of reusing shadow offsets.
fn lowercase_shadow(text: &str) -> (String, Vec<usize>, Vec<usize>) {
    let mut shadow = String::with_capacity(text.len());
    let mut starts = Vec::with_capacity(text.len());
    let mut ends = Vec::with_capacity(text.len());
    for (orig_idx, ch) in text.char_indices() {
        let orig_end = orig_idx + ch.len_utf8();
        for lc in ch.to_lowercase() {
            let before = shadow.len();
            shadow.push(lc);
            for _ in before..shadow.len() {
                starts.push(orig_idx);
                ends.push(orig_end);
            }
        }
    }
    (shadow, starts, ends)
}

fn find_phrases(
    text: &str,
    phrases: &[&str],
    kind: AnnotationKind,
    note: &str,
    out: &mut Vec<Annotation>,
) {
    let (shadow, starts, ends) = lowercase_shadow(text);
    for phrase in phrases {
        for (s, m) in shadow.match_indices(phrase) {
            let e = s + m.len();
            out.push(Annotation {
                start: starts[s],
                end: ends[e - 1],
                kind,
                note: note.to_string(),
            });
        }
    }
}

/// Sentence spans using the same boundary rules as the repetition scorer:
/// split on `.!?` and CJK equivalents once a sentence exceeds 10 chars.
fn sentence_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    let mut char_count = 0usize;

    for (idx, ch) in text.char_indices() {
        char_count += 1;
        let is_sentence_end = matches!(ch, '.' | '!' | '?' | '\u{3002}' | '\u{FF01}' | '\u{FF1F}');
        if is_sentence_end && char_count > 10 {
            let end = idx + ch.len_utf8();
            spans.push((start, end));
            start = end;
            char_count = 0;
        }
    }
    if start < text.len() {
        spans.push((start, text.len()));
    }
    spans
}

fn normalize_sentence(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_repetitions(text: &str, out: &mut Vec<Annotation>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (start, end) in sentence_spans(text) {
        let sentence = text[start..end].trim();
        if sentence.is_empty() {
            continue;
        }
        let normalized = normalize_sentence(sentence);
        // Ignore very short fragments ("Yes.") that repeat legitimately.
        if normalized.chars().count() < 15 {
            continue;
        }
        let count = seen.entry(normalized).or_insert(0);
        *count += 1;
        if *count > 1 {
            out.push(Annotation {
                start,
                end,
                kind: AnnotationKind::Repetition,
                note: format!("duplicate occurrence #{count} of an earlier sentence"),
            });
        }
    }
}

/// Extract UI-ready annotations from an extracted thinking string.
/// Returns annotations sorted by start offset; kinds may overlap.
pub fn annotate(thinking: &str) -> Vec<Annotation> {
    let mut out = Vec::new();

    for re in RESTART_REGEXES.iter() {
        for m in re.find_iter(thinking) {
            out.push(Annotation {
                start: m.start(),
                end: m.end(),
                kind: AnnotationKind::Restart,
                note: "restart / backtrack phrase".to_string(),
            });
        }
    }

    find_phrases(
        thinking,
        EXPLICIT_VERIFICATION,
        AnnotationKind::Verification,
        "explicit self-verification",
        &mut out,
    );

    find_repetitions(thinking, &mut out);

    out.sort_by_key(|a| (a.start, a.end));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_restart_and_verification_spans() {
        let t = "Let me try again. 2+2=4. Let me verify: yes 4.";
        let anns = annotate(t);
        assert!(anns.iter().any(|a| a.kind == AnnotationKind::Restart));
        let v = anns
            .iter()
            .find(|a| a.kind == AnnotationKind::Verification)
            .expect("verification span");
        assert_eq!(&t[v.start..v.end], "Let me verify");
    }

    #[test]
    fn verification_matching_is_case_insensitive() {
        let t = "Done. LET ME VERIFY the result now.";
        let anns = annotate(t);
        let v = anns
            .iter()
            .find(|a| a.kind == AnnotationKind::Verification)
            .expect("verification span");
        assert_eq!(&t[v.start..v.end], "LET ME VERIFY");
    }

    #[test]
    fn cjk_input_does_not_panic_and_offsets_are_char_safe() {
        let t = "计算 2+2。验证一下:等于4。再次验证一下:还是4。";
        for a in annotate(t) {
            assert!(t.is_char_boundary(a.start), "start not on boundary");
            assert!(t.is_char_boundary(a.end), "end not on boundary");
        }
        // CJK explicit verification phrase should be found
        assert!(annotate(t)
            .iter()
            .any(|a| a.kind == AnnotationKind::Verification));
    }

    #[test]
    fn flags_repeated_sentences() {
        let t = "The sum is four because two plus two equals four. \
                 The sum is four because two plus two equals four.";
        let anns = annotate(t);
        assert!(anns.iter().any(|a| a.kind == AnnotationKind::Repetition));
    }

    #[test]
    fn short_repeated_fragments_are_ignored() {
        let t = "Yes. Yes. Yes. The detailed reasoning follows here.";
        let anns = annotate(t);
        assert!(!anns.iter().any(|a| a.kind == AnnotationKind::Repetition));
    }

    #[test]
    fn clean_trace_has_no_annotations() {
        let t = "Compute 2+2 step by step. Two plus two equals four. Done here.";
        let restarts_or_reps = annotate(t)
            .iter()
            .filter(|a| a.kind != AnnotationKind::Verification)
            .count();
        assert_eq!(restarts_or_reps, 0);
    }

    #[test]
    fn annotations_are_sorted_by_start() {
        let t = "Let me verify early. Wait, let me restart. Let me check the end.";
        let anns = annotate(t);
        let starts: Vec<usize> = anns.iter().map(|a| a.start).collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted);
    }
}
