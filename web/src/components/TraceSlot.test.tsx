// @vitest-environment happy-dom

// TraceSlot is the presentational half of the compare view: EMPTY renders the
// same InputPanel + GalleryStrip the single-trace flow uses; LOADED collapses
// to a header (slot label + trace id/problem) + Replace + the AnatomyView.
// analyzeTrace is never called here (ComparePanel owns it) — only costPresets,
// which the AnatomyHeader reads from the registry; an empty list keeps the
// cost meter to its manual rate.

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AnalysisResult, ScoredTrace, TraceInput } from "../lib/types";

vi.mock("../lib/wasm", () => ({
  analyzeTrace: vi.fn(),
  costPresets: () => [],
}));

import { TraceSlot } from "./TraceSlot";

function fakeScored(): ScoredTrace {
  return {
    id: "1",
    problem: "p",
    thinking: "t",
    answer: "a",
    quality_score: 10,
    raw_score: 78,
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

function fakeResult(composite = 42): AnalysisResult {
  return {
    composite,
    scores: [{ name: "efficiency", score: composite, weight: 0.2, diagnostics: [] }],
    annotations: [],
    tokenCount: 0,
    extractedThinking: "",
    scored: fakeScored(),
  };
}

const noop = () => {};

beforeEach(() => {
  // GalleryStrip fetches its index on mount; 404 hides it and keeps the DOM
  // focused on what each test asserts.
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 404 })));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("TraceSlot", () => {
  it("renders the input surface and no anatomy when empty", () => {
    const { container } = render(
      <TraceSlot slotLabel="A" loaded={null} error={null} onSelect={noop} onReplace={noop} resetToken={0} />,
    );

    expect(screen.getByPlaceholderText(/paste a trace/i)).not.toBeNull();
    expect(container.querySelector(".anatomy")).toBeNull();
    expect(screen.queryByRole("button", { name: /replace/i })).toBeNull();
  });

  it("renders the header, Replace button, and anatomy when loaded", () => {
    const record: TraceInput = { id: "7", problem: "Solve for x in the equation", thinking: "t", answer: "a" };
    const { container } = render(
      <TraceSlot
        slotLabel="B"
        loaded={{ record, result: fakeResult() }}
        error={null}
        onSelect={noop}
        onReplace={noop}
        resetToken={0}
      />,
    );

    expect(container.querySelector(".anatomy")).not.toBeNull();
    expect(screen.getByRole("button", { name: /replace/i })).not.toBeNull();

    const header = container.querySelector(".compare-slot-header");
    expect(header).not.toBeNull();
    expect(header!.textContent).toContain("B");
    expect(header!.textContent).toContain("Solve for x");
    // A loaded slot hides its loader.
    expect(screen.queryByPlaceholderText(/paste a trace/i)).toBeNull();
  });

  it("fires onReplace when Replace is clicked", () => {
    const onReplace = vi.fn();
    render(
      <TraceSlot
        slotLabel="A"
        loaded={{ record: { id: "1", problem: "p", thinking: "t", answer: "a" }, result: fakeResult() }}
        error={null}
        onSelect={noop}
        onReplace={onReplace}
        resetToken={0}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /replace/i }));
    expect(onReplace).toHaveBeenCalledTimes(1);
  });

  it("announces a slot error to assistive tech while keeping the loader available", () => {
    const { container } = render(
      <TraceSlot slotLabel="A" loaded={null} error="boom" onSelect={noop} onReplace={noop} resetToken={0} />,
    );

    const alert = container.querySelector<HTMLElement>(".slot-error");
    expect(alert).not.toBeNull();
    expect(alert!.getAttribute("role")).toBe("alert");
    expect(alert!.textContent).toContain("boom");
    // The error sits above the still-usable loader, not in place of it.
    expect(screen.getByPlaceholderText(/paste a trace/i)).not.toBeNull();
  });
});
