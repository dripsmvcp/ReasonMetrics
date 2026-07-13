// Tests for the pure span logic behind the anatomy view: UTF-8-byte →
// UTF-16-index offset conversion, the flat segment partition, and
// repetition grouping. Property tests use fast-check per the plan.

import fc from "fast-check";
import { beforeAll, describe, expect, it } from "vitest";
import {
  annotationsToUtf16,
  buildByteToUtf16Map,
  buildSegments,
  groupRepetitions,
} from "./spans";
import type { Annotation, AnnotationKind } from "./types";
import { analyzeTrace, initWasm } from "./wasm";

// "计算 2+2。验证一下:等于4。再次验证一下:还是4。" — every CJK char is
// 3 UTF-8 bytes but 1 UTF-16 unit, so byte offsets diverge from JS indices.
const CJK_TEXT = "计算 2+2。验证一下:等于4。再次验证一下:还是4。";

function ann(start: number, end: number, kind: AnnotationKind, note = ""): Annotation {
  return { start, end, kind, note };
}

describe("buildByteToUtf16Map", () => {
  it("maps ASCII byte offsets 1:1", () => {
    const map = buildByteToUtf16Map("abc");
    expect(map[0]).toBe(0);
    expect(map[1]).toBe(1);
    expect(map[2]).toBe(2);
    expect(map[3]).toBe(3);
  });

  it("maps 3-byte CJK chars to single UTF-16 units", () => {
    const map = buildByteToUtf16Map(CJK_TEXT);
    // "计算 2+2。" = 2×3 + 1 + 3 + 3 = 13 bytes but 7 UTF-16 units.
    expect(map[13]).toBe(7);
    // "验证一下" spans bytes 13..25 and UTF-16 indices 7..11.
    expect(map[25]).toBe(11);
  });

  it("maps astral emoji (4 UTF-8 bytes, 2 UTF-16 units)", () => {
    const map = buildByteToUtf16Map("a\u{1F642}b");
    expect(map[0]).toBe(0);
    expect(map[1]).toBe(1); // after "a"
    expect(map[5]).toBe(3); // after the emoji (4 bytes, 2 units)
    expect(map[6]).toBe(4); // after "b"
  });
});

describe("annotationsToUtf16", () => {
  it("converts CJK byte offsets so slicing yields the annotated phrase", () => {
    const [converted] = annotationsToUtf16(CJK_TEXT, [ann(13, 25, "verification")]);
    expect(CJK_TEXT.slice(converted.start, converted.end)).toBe("验证一下");
    // Naive byte-offset slicing would have produced garbage:
    expect(CJK_TEXT.slice(13, 25)).not.toBe("验证一下");
  });

  it("keeps ASCII offsets unchanged", () => {
    const [converted] = annotationsToUtf16("Let me verify: ok.", [ann(0, 13, "verification")]);
    expect(converted.start).toBe(0);
    expect(converted.end).toBe(13);
  });
});

describe("buildSegments", () => {
  it("returns a single kind-less segment when there are no annotations", () => {
    expect(buildSegments(10, [])).toEqual([{ start: 0, end: 10, kinds: [] }]);
  });

  it("returns no segments for empty text", () => {
    expect(buildSegments(0, [])).toEqual([]);
    expect(buildSegments(0, [ann(0, 0, "restart")])).toEqual([]);
  });

  it("splits overlapping annotations into segments with stacked kinds", () => {
    const segments = buildSegments(10, [ann(0, 6, "restart"), ann(4, 8, "verification")]);
    expect(segments).toEqual([
      { start: 0, end: 4, kinds: ["restart"] },
      { start: 4, end: 6, kinds: ["restart", "verification"] },
      { start: 6, end: 8, kinds: ["verification"] },
      { start: 8, end: 10, kinds: [] },
    ]);
  });

  it("handles an annotation nested fully inside another", () => {
    const segments = buildSegments(12, [ann(0, 12, "repetition"), ann(3, 6, "restart")]);
    expect(segments).toEqual([
      { start: 0, end: 3, kinds: ["repetition"] },
      { start: 3, end: 6, kinds: ["restart", "repetition"] },
      { start: 6, end: 12, kinds: ["repetition"] },
    ]);
  });
});

// --- property tests (plan-mandated): random span sets ---

const kindArb = fc.constantFrom<AnnotationKind>("restart", "verification", "repetition");

function annotationArb(maxOffset: number) {
  return fc
    .tuple(
      fc.integer({ min: 0, max: maxOffset }),
      fc.integer({ min: 0, max: maxOffset }),
      kindArb,
    )
    .map(([a, b, kind]) => ann(Math.min(a, b), Math.max(a, b), kind));
}

const lengthAndAnnotationsArb = fc
  .integer({ min: 0, max: 300 })
  .chain((len) =>
    fc.tuple(fc.constant(len), fc.array(annotationArb(len), { maxLength: 15 })),
  );

const textAndAnnotationsArb = fc
  .string({ unit: "binary", maxLength: 150 })
  .chain((text) =>
    fc.tuple(fc.constant(text), fc.array(annotationArb(text.length), { maxLength: 15 })),
  );

describe("buildSegments properties", () => {
  it("partitions [0, textLength) exactly: sorted, no gaps, no overlaps", () => {
    fc.assert(
      fc.property(lengthAndAnnotationsArb, ([len, anns]) => {
        const segments = buildSegments(len, anns);
        if (len === 0) {
          expect(segments).toEqual([]);
          return;
        }
        expect(segments[0].start).toBe(0);
        expect(segments[segments.length - 1].end).toBe(len);
        for (let i = 0; i < segments.length; i++) {
          expect(segments[i].start).toBeLessThan(segments[i].end);
          if (i > 0) expect(segments[i].start).toBe(segments[i - 1].end);
        }
      }),
    );
  });

  it("concatenated segment texts reconstruct the input text", () => {
    fc.assert(
      fc.property(textAndAnnotationsArb, ([text, anns]) => {
        const segments = buildSegments(text.length, anns);
        expect(segments.map((s) => text.slice(s.start, s.end)).join("")).toBe(text);
      }),
    );
  });

  it("covers every annotation's range exactly with segments carrying its kind", () => {
    fc.assert(
      fc.property(lengthAndAnnotationsArb, ([len, anns]) => {
        const segments = buildSegments(len, anns);
        for (const a of anns) {
          if (a.start >= a.end) continue;
          let covered = 0;
          for (const s of segments) {
            if (s.start >= a.end || s.end <= a.start) continue;
            // Segments never straddle an annotation boundary…
            expect(s.start).toBeGreaterThanOrEqual(a.start);
            expect(s.end).toBeLessThanOrEqual(a.end);
            // …and every segment inside the annotation carries its kind.
            expect(s.kinds).toContain(a.kind);
            covered += s.end - s.start;
          }
          expect(covered).toBe(a.end - a.start);
        }
        // Conversely, no segment claims a kind that no annotation gives it.
        for (const s of segments) {
          for (const kind of s.kinds) {
            expect(
              anns.some((a) => a.kind === kind && a.start <= s.start && a.end >= s.end),
            ).toBe(true);
          }
        }
      }),
    );
  });
});

describe("groupRepetitions", () => {
  const sentence = "The quick brown fox jumps over the lazy dog.";

  it("groups duplicates of one sentence and counts the unannotated first occurrence", () => {
    const text = `${sentence} ${sentence} ${sentence}`;
    const anns = [
      ann(sentence.length, sentence.length * 2 + 1, "repetition", "duplicate occurrence #2 of an earlier sentence"),
      ann(sentence.length * 2 + 1, text.length, "repetition", "duplicate occurrence #3 of an earlier sentence"),
    ];
    const groups = groupRepetitions(text, anns);
    expect(groups).toHaveLength(1);
    expect(groups[0].spans).toHaveLength(2);
    expect(groups[0].total).toBe(3); // 2 duplicates + the first occurrence
  });

  it("normalizes case, whitespace, and trailing punctuation when grouping", () => {
    const first = "It is what it is right now.";
    const second = "IT IS WHAT IT IS RIGHT NOW!!!";
    const third = "it  is\twhat it is right now";
    const text = `${first} ${second} ${third}`;
    const anns = [
      ann(text.indexOf(second), text.indexOf(second) + second.length, "repetition", "#2"),
      ann(text.indexOf(third), text.length, "repetition", "#3"),
    ];
    const groups = groupRepetitions(text, anns);
    expect(groups).toHaveLength(1);
    expect(groups[0].total).toBe(3);
  });

  it("keeps distinct sentences in distinct groups and ignores other kinds", () => {
    const a = "Alpha sentence repeating in this trace.";
    const b = "Beta sentence repeating in this trace.";
    const text = `${a} ${b} ${a} ${b}`;
    const anns = [
      ann(text.indexOf(a, 1), text.indexOf(a, 1) + a.length, "repetition", "#2"),
      ann(text.indexOf(b, text.indexOf(b) + 1), text.length, "repetition", "#2"),
      ann(0, 5, "restart", "restart / backtrack phrase"),
    ];
    const groups = groupRepetitions(text, anns);
    expect(groups).toHaveLength(2);
    expect(groups.map((g) => g.total)).toEqual([2, 2]);
  });
});

describe("CJK annotations from the real wasm engine", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("slices the verification phrase 验证一下 correctly after offset conversion", () => {
    const result = analyzeTrace({ problem: "计算 2+2", thinking: CJK_TEXT, answer: "4" });
    const converted = annotationsToUtf16(result.extractedThinking, result.annotations);
    const verifications = converted.filter((a) => a.kind === "verification");
    expect(verifications.length).toBeGreaterThan(0);
    for (const v of verifications) {
      expect(result.extractedThinking.slice(v.start, v.end)).toBe("验证一下");
    }
  });
});
