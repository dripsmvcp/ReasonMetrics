// Pure logic behind the anatomy view. No DOM here: byte-offset conversion
// (Rust annotations index extractedThinking by UTF-8 bytes, JS strings by
// UTF-16 units), the flat segment partition used by the renderer, and
// grouping of repetition annotations into ×N accordion groups.

import type { Annotation, AnnotationKind } from "./types";

/** One piece of the flat, sorted, non-overlapping partition of the text.
 * `kinds` is every annotation kind covering [start, end), in the fixed
 * restart / verification / repetition order. */
export interface Segment {
  start: number;
  end: number;
  kinds: AnnotationKind[];
}

export interface RepetitionSpan {
  start: number;
  end: number;
  note: string;
}

/** Repetition annotations whose span text normalizes identically. `total`
 * counts the un-annotated first occurrence, so a sentence appearing three
 * times has two spans and total 3. */
export interface RepetitionGroup {
  key: string;
  spans: RepetitionSpan[];
  total: number;
}

const KIND_ORDER: AnnotationKind[] = ["restart", "verification", "repetition"];

function utf8Length(codePoint: number): number {
  if (codePoint < 0x80) return 1;
  if (codePoint < 0x800) return 2;
  if (codePoint < 0x10000) return 3;
  return 4;
}

/**
 * Map from UTF-8 byte offset to UTF-16 string index for `text`. Entries are
 * defined (>= 0) only at char boundaries — exactly the offsets Rust can
 * emit; interior bytes are left at -1.
 */
export function buildByteToUtf16Map(text: string): Int32Array {
  let byteLength = 0;
  for (const ch of text) byteLength += utf8Length(ch.codePointAt(0)!);

  const map = new Int32Array(byteLength + 1).fill(-1);
  let byte = 0;
  let unit = 0;
  for (const ch of text) {
    map[byte] = unit;
    byte += utf8Length(ch.codePointAt(0)!);
    unit += ch.length;
  }
  map[byte] = unit;
  return map;
}

/**
 * Convert annotations whose start/end are UTF-8 byte offsets into `text`
 * (as produced by the wasm engine) to UTF-16 string indices safe for
 * `text.slice`. Throws if an offset is out of range or not a char boundary
 * — the Rust side guarantees neither happens.
 */
export function annotationsToUtf16(text: string, annotations: Annotation[]): Annotation[] {
  const map = buildByteToUtf16Map(text);
  return annotations.map((a) => {
    const start = map[a.start];
    const end = map[a.end];
    if (start === undefined || end === undefined || start < 0 || end < 0) {
      throw new Error(`annotation byte offset not on a char boundary: ${a.start}..${a.end}`);
    }
    return { ...a, start, end };
  });
}

/**
 * Partition [0, textLength) into flat, sorted, non-overlapping segments,
 * each carrying the set of annotation kinds covering it. Overlapping or
 * nested annotations therefore become segments with stacked kinds — the
 * renderer walks this list and can never emit mis-nested markup.
 * Annotation offsets are clamped to the range; empty spans are ignored.
 */
export function buildSegments(
  textLength: number,
  annotations: { start: number; end: number; kind: AnnotationKind }[],
): Segment[] {
  const clamp = (n: number) => Math.max(0, Math.min(textLength, n));
  const spans = annotations
    .map((a) => ({ start: clamp(a.start), end: clamp(a.end), kind: a.kind }))
    .filter((a) => a.start < a.end);

  const cuts = new Set<number>([0, textLength]);
  for (const span of spans) {
    cuts.add(span.start);
    cuts.add(span.end);
  }
  const points = [...cuts].sort((a, b) => a - b);

  const segments: Segment[] = [];
  for (let i = 0; i + 1 < points.length; i++) {
    const start = points[i];
    const end = points[i + 1];
    const kinds = KIND_ORDER.filter((kind) =>
      spans.some((span) => span.kind === kind && span.start <= start && span.end >= end),
    );
    segments.push({ start, end, kinds });
  }
  return segments;
}

/** Lowercase, collapse whitespace, strip trailing punctuation — the
 * TS-side normalization used to decide which duplicates belong together. */
function normalizeSentence(sentence: string): string {
  return sentence
    .toLowerCase()
    .split(/\s+/u)
    .filter((word) => word.length > 0)
    .join(" ")
    .replace(/[.!?,;:。！？，；：…]+$/u, "");
}

/**
 * Group repetition annotations (UTF-16 offsets into `text`) by normalized
 * span text. Non-repetition annotations are ignored. Groups preserve first
 * appearance order; spans preserve annotation order.
 */
export function groupRepetitions(text: string, annotations: Annotation[]): RepetitionGroup[] {
  const groups = new Map<string, RepetitionGroup>();
  for (const a of annotations) {
    if (a.kind !== "repetition") continue;
    const key = normalizeSentence(text.slice(a.start, a.end));
    let group = groups.get(key);
    if (!group) {
      group = { key, spans: [], total: 1 };
      groups.set(key, group);
    }
    group.spans.push({ start: a.start, end: a.end, note: a.note });
    group.total += 1;
  }
  return [...groups.values()];
}
