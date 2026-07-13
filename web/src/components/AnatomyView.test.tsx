// @vitest-environment happy-dom

// Component tests for the anatomy view: header (tokens / cost / dial),
// legend, annotated thinking text (restart badge, verification highlight,
// repetition accordion), and the 9-row score card. Uses fabricated
// AnalysisResults — annotation offsets are UTF-8 byte offsets, exactly as
// the wasm bridge delivers them.

import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type {
  AnalysisResult,
  Annotation,
  AnnotationKind,
  ScoreEntry,
  ScoredTrace,
} from "../lib/types";
import { AnatomyView } from "./AnatomyView";

const SCORER_NAMES = [
  "efficiency",
  "language",
  "answer_alignment",
  "structural",
  "repetition",
  "verification",
  "length",
  "overthinking",
  "quality",
];

function makeScores(score = 70): ScoreEntry[] {
  return SCORER_NAMES.map((name) => ({
    name,
    score,
    weight: 1,
    diagnostics: [
      [`top_${name}`, `value_${name}`],
      ["second_key", "ignored"],
    ] as [string, string][],
  }));
}

function makeScored(): ScoredTrace {
  return {
    id: "t1",
    problem: "p",
    thinking: "t",
    answer: "a",
    quality_score: 0,
    efficiency_score: 0,
    language_score: 0,
    answer_alignment_score: 0,
    structural_score: 0,
    repetition_score: 0,
    overthinking_score: 0,
    verification_score: 0,
    length_score: 0,
    thinking_word_count: 0,
    restart_count: 0,
    detected_language: "english",
    has_self_verification: false,
    is_language_mixed: false,
    answer_in_trace_end: false,
  };
}

function makeResult(overrides: Partial<AnalysisResult> = {}): AnalysisResult {
  return {
    composite: 82,
    scores: makeScores(),
    annotations: [],
    tokenCount: 1234,
    extractedThinking: "hello world",
    scored: makeScored(),
    ...overrides,
  };
}

function ann(start: number, end: number, kind: AnnotationKind, note = ""): Annotation {
  return { start, end, kind, note };
}

function renderAnatomy(overrides: Partial<AnalysisResult> = {}): HTMLElement {
  const { container } = render(<AnatomyView result={makeResult(overrides)} />);
  return container;
}

describe("AnatomyView: header", () => {
  it("shows the token count", () => {
    const container = renderAnatomy({ tokenCount: 1234 });
    expect(container.querySelector(".token-count")?.textContent).toContain("1234");
  });

  it("shows cost at the default $3 / 1M-token rate, formatted $X.XXXX", () => {
    const container = renderAnatomy({ tokenCount: 1234 });
    const rate = container.querySelector<HTMLInputElement>(".rate-input")!;
    expect(rate.value).toBe("3");
    expect(container.querySelector(".cost-value")?.textContent).toBe("$0.0037");
  });

  it("labels the rate input '$ / 1M tokens'", () => {
    const container = renderAnatomy();
    expect(container.querySelector(".cost-block")?.textContent).toContain("$ / 1M tokens");
  });

  it("recomputes the cost when the rate changes", () => {
    const container = renderAnatomy({ tokenCount: 1234 });
    const rate = container.querySelector<HTMLInputElement>(".rate-input")!;
    fireEvent.change(rate, { target: { value: "10" } });
    expect(container.querySelector(".cost-value")?.textContent).toBe("$0.0123");
  });

  it("renders an SVG dial with the composite value centered", () => {
    const container = renderAnatomy({ composite: 82 });
    expect(container.querySelector("svg.dial")).not.toBeNull();
    expect(container.querySelector(".dial-value")?.textContent).toBe("82.0");
  });

  it.each([
    [42, "score-red"],
    [49.9, "score-red"],
    [50, "score-amber"],
    [74.9, "score-amber"],
    [75, "score-green"],
    [92, "score-green"],
  ])("colors the dial for composite %s with %s", (composite, cls) => {
    const container = renderAnatomy({ composite: composite as number });
    expect(container.querySelector(`.dial-arc.${cls}`)).not.toBeNull();
  });
});

describe("AnatomyView: legend", () => {
  it("shows three legend items for restart, verification, repetition", () => {
    const container = renderAnatomy();
    const items = container.querySelectorAll(".legend .legend-item");
    expect(items).toHaveLength(3);
    expect(container.querySelector(".legend")?.textContent).toContain("restart");
    expect(container.querySelector(".legend")?.textContent).toContain("verification");
    expect(container.querySelector(".legend")?.textContent).toContain("repetition");
  });
});

describe("AnatomyView: annotated thinking text", () => {
  it("renders restart spans with a ⟲ badge after them", () => {
    const text = "Wait, let me try again. Done.";
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [ann(0, 22, "restart", "restart / backtrack phrase")],
    });
    const span = container.querySelector<HTMLElement>(".thinking-text .ann-restart")!;
    expect(span.textContent).toBe("Wait, let me try again");
    expect(span.title).toBe("restart / backtrack phrase");
    const badge = span.nextElementSibling as HTMLElement;
    expect(badge.classList.contains("restart-badge")).toBe(true);
    expect(badge.textContent).toBe("⟲");
  });

  it("renders verification spans as highlights with the note as title", () => {
    const text = "Let me verify: ok.";
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [ann(0, 13, "verification", "explicit self-verification")],
    });
    const span = container.querySelector<HTMLElement>(".thinking-text .ann-verification")!;
    expect(span.textContent).toBe("Let me verify");
    expect(span.title).toBe("explicit self-verification");
  });

  it("uses the annotation's actual note as title, not a generic constant", () => {
    // Note deliberately differs from the usual core boilerplate: the title
    // must come from annotation.note, never a hardcoded lookup.
    const note = "restart / backtrack phrase (3rd occurrence)";
    const text = "Wait, let me try again. Done.";
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [ann(0, 22, "restart", note)],
    });
    const span = container.querySelector<HTMLElement>(".thinking-text .ann-restart")!;
    expect(span.title).toBe(note);
    expect((span.nextElementSibling as HTMLElement).title).toBe(note);
  });

  it("joins the distinct notes of overlapping annotations with '; '", () => {
    const text = "abcdefghijklmnopqrst";
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [
        ann(0, 10, "restart", "custom restart note"),
        ann(5, 15, "verification", "custom verification note"),
      ],
    });
    const both = container.querySelector<HTMLElement>(
      ".thinking-text .ann-restart.ann-verification",
    )!;
    expect(both.title).toBe("custom restart note; custom verification note");
  });

  it("stacks classes on segments covered by overlapping annotations", () => {
    const text = "abcdefghijklmnopqrst";
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [ann(0, 10, "restart"), ann(5, 15, "verification")],
    });
    const both = container.querySelector<HTMLElement>(
      ".thinking-text .ann-restart.ann-verification",
    )!;
    expect(both.textContent).toBe(text.slice(5, 10));
  });

  it("converts byte offsets so CJK verification phrases highlight correctly", () => {
    const text = "计算 2+2。验证一下:等于4。";
    const container = renderAnatomy({
      extractedThinking: text,
      // Byte offsets: "验证一下" occupies bytes 13..25.
      annotations: [ann(13, 25, "verification", "explicit self-verification")],
    });
    const span = container.querySelector<HTMLElement>(".thinking-text .ann-verification")!;
    expect(span.textContent).toBe("验证一下");
  });

  it("collapses duplicate sentences behind a ×N pill that toggles the text", () => {
    const s = "This sentence repeats itself in the trace.";
    const text = `${s} ${s}`;
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [
        ann(s.length, text.length, "repetition", "duplicate occurrence #2 of an earlier sentence"),
      ],
    });

    const pills = container.querySelectorAll<HTMLButtonElement>(".thinking-text .rep-pill");
    expect(pills).toHaveLength(1); // first occurrence stays plain text
    const pill = pills[0];
    expect(pill.textContent).toBe("×2 ↕");
    expect(pill.title).toBe("duplicate occurrence #2 of an earlier sentence");

    const content = container.querySelector<HTMLElement>(".thinking-text .rep-content")!;
    expect(content.hidden).toBe(true);
    expect(content.textContent).toBe(text.slice(s.length));

    fireEvent.click(pill);
    expect(content.hidden).toBe(false);
    fireEvent.click(pill);
    expect(content.hidden).toBe(true);
  });

  it("counts all duplicates of a group in every pill (×3 for a thrice-repeated sentence)", () => {
    const s = "Another sentence that keeps coming back again.";
    const text = `${s} ${s} ${s}`;
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [
        ann(s.length, s.length * 2 + 1, "repetition", "duplicate occurrence #2 of an earlier sentence"),
        ann(s.length * 2 + 1, text.length, "repetition", "duplicate occurrence #3 of an earlier sentence"),
      ],
    });
    const pills = [...container.querySelectorAll(".thinking-text .rep-pill")];
    expect(pills.map((p) => p.textContent)).toEqual(["×3 ↕", "×3 ↕"]);
  });

  it("keeps other kinds' classes inside expanded repetition content", () => {
    const s = "Wait, let me try again because this failed before.";
    const text = `${s} ${s}`;
    const restartStart = s.length + 1; // "Wait, let me try again" of the 2nd occurrence
    const container = renderAnatomy({
      extractedThinking: text,
      annotations: [
        ann(s.length, text.length, "repetition", "duplicate occurrence #2 of an earlier sentence"),
        ann(restartStart, restartStart + 22, "restart", "restart / backtrack phrase"),
      ],
    });
    const nested = container.querySelector<HTMLElement>(".rep-content .ann-restart")!;
    expect(nested.textContent).toBe("Wait, let me try again");
  });

  it("renders unannotated text verbatim", () => {
    const container = renderAnatomy({ extractedThinking: "plain thinking text", annotations: [] });
    expect(container.querySelector(".thinking-text")?.textContent).toBe("plain thinking text");
  });
});

describe("AnatomyView: score card", () => {
  it("renders nine rows in the order received", () => {
    const container = renderAnatomy();
    const names = [...container.querySelectorAll(".score-card .score-row .score-name")].map(
      (n) => n.textContent,
    );
    expect(names).toEqual(SCORER_NAMES);
  });

  it("renders each row's bar width, colored class, and 1-decimal value", () => {
    const scores = makeScores();
    scores[0].score = 42.5;
    scores[1].score = 75;
    const container = renderAnatomy({ scores });
    const rows = container.querySelectorAll(".score-card .score-row");

    const fill0 = rows[0].querySelector<HTMLElement>(".score-bar-fill")!;
    expect(fill0.style.width).toBe("42.5%");
    expect(fill0.classList.contains("score-red")).toBe(true);
    expect(rows[0].querySelector(".score-value")?.textContent).toBe("42.5");

    const fill1 = rows[1].querySelector<HTMLElement>(".score-bar-fill")!;
    expect(fill1.classList.contains("score-green")).toBe(true);
    expect(rows[1].querySelector(".score-value")?.textContent).toBe("75.0");
  });

  it("shows the top diagnostic verbatim as key: value with a hover title", () => {
    const container = renderAnatomy();
    const diag = container.querySelector<HTMLElement>(".score-card .score-row .score-diag")!;
    expect(diag.textContent).toBe("top_efficiency: value_efficiency");
    expect(diag.title).toBe("top_efficiency: value_efficiency");
  });

  it("shows nothing for a row whose diagnostics are empty", () => {
    const scores = makeScores();
    scores[3].diagnostics = [];
    const container = renderAnatomy({ scores });
    const rows = container.querySelectorAll(".score-card .score-row");
    expect(rows[3].querySelector(".score-diag")).toBeNull();
    expect(rows[2].querySelector(".score-diag")).not.toBeNull();
  });
});

describe("AnatomyView: re-rendering", () => {
  it("replaces previous content instead of appending", () => {
    const result = makeResult();
    const { container, rerender } = render(<AnatomyView result={result} />);
    rerender(<AnatomyView result={result} />);
    expect(container.querySelectorAll(".anatomy")).toHaveLength(1);
  });
});
