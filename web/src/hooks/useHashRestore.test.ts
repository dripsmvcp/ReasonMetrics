// @vitest-environment happy-dom

// Tests for hash-based restore on load: a valid `#t=...` fragment should
// drive the same render callback the input panel uses (no forked render
// path); an invalid/missing fragment must be ignored silently.

import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { encodeShareFragment } from "../lib/share";
import type { TraceInput } from "../lib/types";
import { useHashRestore } from "./useHashRestore";

beforeEach(() => {
  location.hash = "";
});

afterEach(() => {
  location.hash = "";
  vi.restoreAllMocks();
});

describe("useHashRestore", () => {
  it("decodes a valid #t= fragment and calls renderAnalysis with the trace", () => {
    const trace: TraceInput = { id: "9", problem: "p", thinking: "t", answer: "a" };
    const encoded = encodeShareFragment(trace);
    if ("tooLarge" in encoded) throw new Error("unexpectedly too large");
    location.hash = `t=${encoded.fragment}`;

    const renderAnalysis = vi.fn<(record: TraceInput) => void>();
    renderHook(() => useHashRestore(renderAnalysis));

    expect(renderAnalysis).toHaveBeenCalledExactlyOnceWith(trace);
  });

  it("does nothing when there is no hash", () => {
    const renderAnalysis = vi.fn<(record: TraceInput) => void>();
    renderHook(() => useHashRestore(renderAnalysis));
    expect(renderAnalysis).not.toHaveBeenCalled();
  });

  it("ignores a corrupt fragment silently (warns, never throws, never calls renderAnalysis)", () => {
    location.hash = "#t=not-a-real-payload!!!";
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const renderAnalysis = vi.fn<(record: TraceInput) => void>();

    expect(() => renderHook(() => useHashRestore(renderAnalysis))).not.toThrow();

    expect(renderAnalysis).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalled();
  });

  it("ignores an unrelated hash (no t= assignment), warning but never throwing", () => {
    location.hash = "#somethingElse";
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const renderAnalysis = vi.fn<(record: TraceInput) => void>();

    renderHook(() => useHashRestore(renderAnalysis));

    expect(renderAnalysis).not.toHaveBeenCalled();
    expect(warnSpy).toHaveBeenCalled();
  });
});
