// @vitest-environment happy-dom

// ComparePanel owns the two slots: it wires each slot's onSelect to the same
// analyzeTrace the single-trace flow uses, holds the per-slot result/error, and
// renders the reused CompareScores delta table above the two columns. These
// tests exercise that wiring with analyzeTrace mocked (scoring itself is covered
// in wasm.test.ts) — load-into-A, delta on load-into-B, Replace, and per-slot
// error isolation.

import { fireEvent, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AnalysisResult, ScoredTrace, TraceInput } from "../lib/types";

vi.mock("../lib/wasm", () => ({
  analyzeTrace: vi.fn(),
  costPresets: () => [],
}));

import { analyzeTrace } from "../lib/wasm";
import { ComparePanel } from "./ComparePanel";

const analyzeTraceMock = vi.mocked(analyzeTrace);

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

function fakeResult(composite: number): AnalysisResult {
  return {
    composite,
    scores: [{ name: "efficiency", score: composite, weight: 0.2, diagnostics: [] }],
    annotations: [],
    tokenCount: 0,
    extractedThinking: "",
    scored: fakeScored(),
  };
}

function slots(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(".compare-slot"));
}

function pasteInto(slot: HTMLElement, record: TraceInput) {
  const textarea = within(slot).getByPlaceholderText(/paste a trace/i);
  fireEvent.change(textarea, { target: { value: JSON.stringify(record) } });
  fireEvent.click(within(slot).getByRole("button", { name: "Analyze" }));
}

const RECORD_A: TraceInput = { problem: "problem A", thinking: "t", answer: "a" };
const RECORD_B: TraceInput = { problem: "problem B", thinking: "t", answer: "a" };

beforeEach(() => {
  analyzeTraceMock.mockReset();
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 404 })));
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ComparePanel", () => {
  it("renders two empty loaders and no delta table before anything is loaded", () => {
    const { container } = render(<ComparePanel />);
    expect(slots(container)).toHaveLength(2);
    expect(container.querySelector(".compare-scores")).toBeNull();
  });

  it("shows A's anatomy and a delta table (B as –) after loading only A", async () => {
    analyzeTraceMock.mockReturnValue(fakeResult(40));
    const { container } = render(<ComparePanel />);

    pasteInto(slots(container)[0], RECORD_A);

    await vi.waitFor(() => expect(slots(container)[0].querySelector(".anatomy")).not.toBeNull());
    const composite = container.querySelector<HTMLElement>(".compare-composite")!;
    expect(composite).not.toBeNull();
    expect(composite.textContent).toContain("40.0");
    expect(composite.textContent).toContain("–");
  });

  it("computes the B−A delta once both slots are loaded", async () => {
    const { container } = render(<ComparePanel />);

    analyzeTraceMock.mockReturnValue(fakeResult(40));
    pasteInto(slots(container)[0], RECORD_A);
    await vi.waitFor(() => expect(slots(container)[0].querySelector(".anatomy")).not.toBeNull());

    analyzeTraceMock.mockReturnValue(fakeResult(78));
    pasteInto(slots(container)[1], RECORD_B);
    await vi.waitFor(() => expect(slots(container)[1].querySelector(".anatomy")).not.toBeNull());

    const composite = container.querySelector<HTMLElement>(".compare-composite")!;
    expect(composite.textContent).toContain("40.0");
    expect(composite.textContent).toContain("78.0");
    expect(composite.textContent).toContain("+38.0");
  });

  it("returns A to its loader on Replace while B stays loaded", async () => {
    const { container } = render(<ComparePanel />);

    analyzeTraceMock.mockReturnValue(fakeResult(40));
    pasteInto(slots(container)[0], RECORD_A);
    await vi.waitFor(() => expect(slots(container)[0].querySelector(".anatomy")).not.toBeNull());

    analyzeTraceMock.mockReturnValue(fakeResult(78));
    pasteInto(slots(container)[1], RECORD_B);
    await vi.waitFor(() => expect(slots(container)[1].querySelector(".anatomy")).not.toBeNull());

    fireEvent.click(within(slots(container)[0]).getByRole("button", { name: /replace/i }));

    // A is back to a loader; B is untouched.
    expect(within(slots(container)[0]).getByPlaceholderText(/paste a trace/i)).not.toBeNull();
    expect(slots(container)[0].querySelector(".anatomy")).toBeNull();
    expect(slots(container)[1].querySelector(".anatomy")).not.toBeNull();
  });

  it("isolates a per-slot analyze error without blanking the other slot", async () => {
    const { container } = render(<ComparePanel />);

    analyzeTraceMock.mockReturnValue(fakeResult(78));
    pasteInto(slots(container)[1], RECORD_B);
    await vi.waitFor(() => expect(slots(container)[1].querySelector(".anatomy")).not.toBeNull());

    analyzeTraceMock.mockImplementationOnce(() => {
      throw new Error("boom");
    });
    pasteInto(slots(container)[0], RECORD_A);

    await vi.waitFor(() => {
      const alert = slots(container)[0].querySelector<HTMLElement>(".slot-error");
      expect(alert).not.toBeNull();
      expect(alert!.getAttribute("role")).toBe("alert");
      expect(alert!.textContent).toContain("boom");
    });
    // B's anatomy survives A's failure.
    expect(slots(container)[1].querySelector(".anatomy")).not.toBeNull();
  });
});
