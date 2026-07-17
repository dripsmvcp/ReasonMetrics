// @vitest-environment happy-dom

// Integration tests for App's single analyze pipeline and the `#t=...`
// share-link hash semantics around it. Every input source (paste, JSONL row
// select, mapping-dialog apply, gallery card, live stream, hash restore)
// converges on the same render path; analyzing a NEW record clears a stale
// hash left over from an earlier restore/copy, while restoring FROM a hash
// on load must leave that hash alone so a freshly-loaded share link stays
// shareable. `analyzeTrace` is mocked so this exercises App's wiring, not
// wasm scoring — that's covered in wasm.test.ts; the individual panels each
// have their own test file.

import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { encodeShareFragment } from "./lib/share";
import type { AnalysisResult, ScoredTrace, TraceInput } from "./lib/types";

vi.mock("./lib/wasm", () => ({
  analyzeTrace: vi.fn(),
  // The anatomy header reads cost presets from the registry; App wiring tests
  // don't exercise them, so an empty list keeps the meter to its manual rate.
  costPresets: () => [],
}));

import { analyzeTrace } from "./lib/wasm";
import App from "./App";

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

function fakeResult(): AnalysisResult {
  return {
    composite: 10,
    scores: [],
    annotations: [],
    tokenCount: 0,
    extractedThinking: "",
    scored: fakeScored(),
  };
}

beforeEach(() => {
  analyzeTraceMock.mockReset().mockReturnValue(fakeResult());
  location.hash = "";
  // The gallery strip fetches on mount; keep it quiet and out of the way
  // for tests that aren't exercising it.
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 404 })));
});

afterEach(() => {
  vi.unstubAllGlobals();
  location.hash = "";
});

function pasteRecord(record: TraceInput) {
  const textarea = screen.getByPlaceholderText<HTMLTextAreaElement>(/paste a trace/i);
  fireEvent.change(textarea, { target: { value: JSON.stringify(record) } });
  fireEvent.click(screen.getByRole("button", { name: "Analyze" }));
}

describe("App: stale share-link hash on re-analysis", () => {
  it("clears a stale hash when a new record is analyzed via the normal paste path", async () => {
    location.hash = "t=stale-fragment";
    render(<App />);

    pasteRecord({ problem: "p2", thinking: "t2", answer: "a2" });

    await vi.waitFor(() => expect(analyzeTraceMock).toHaveBeenCalledTimes(1));
    expect(location.hash).toBe("");
  });

  it("leaves the hash intact when restoring from a share link on load", async () => {
    const trace: TraceInput = { id: "9", problem: "p1", thinking: "t1", answer: "a1" };
    const encoded = encodeShareFragment(trace);
    if ("tooLarge" in encoded) throw new Error("unexpectedly too large");
    location.hash = `t=${encoded.fragment}`;

    render(<App />);

    await vi.waitFor(() => expect(analyzeTraceMock).toHaveBeenCalledWith(trace));
    expect(location.hash).toBe(`#t=${encoded.fragment}`);
  });

  it("does nothing when there is no hash to clear", async () => {
    render(<App />);
    expect(location.hash).toBe("");

    pasteRecord({ problem: "p2", thinking: "t2", answer: "a2" });

    await vi.waitFor(() => expect(analyzeTraceMock).toHaveBeenCalledTimes(1));
    expect(location.hash).toBe("");
  });

  it("leaves a non-share anchor like #faq alone when a new record is analyzed", async () => {
    location.hash = "#faq";
    render(<App />);

    pasteRecord({ problem: "p2", thinking: "t2", answer: "a2" });

    await vi.waitFor(() => expect(analyzeTraceMock).toHaveBeenCalledTimes(1));
    expect(location.hash).toBe("#faq");
  });
});

describe("App: analyzeTrace error handling", () => {
  it("renders an inline error and preserves the previous result when analyzeTrace throws, clearing on the next success", async () => {
    const { container } = render(<App />);

    pasteRecord({ problem: "p", thinking: "t", answer: "a" });
    await vi.waitFor(() => expect(container.querySelector(".anatomy")).not.toBeNull());
    const anatomyBefore = container.querySelector(".anatomy")?.innerHTML;

    analyzeTraceMock.mockImplementationOnce(() => {
      throw new Error("boom");
    });

    expect(() =>
      pasteRecord({ problem: "p2", thinking: "t2", answer: "a2" }),
    ).not.toThrow();

    expect(container.textContent).toContain("analysis failed: boom");
    expect(container.querySelector(".anatomy")?.innerHTML).toBe(anatomyBefore);

    analyzeTraceMock.mockReturnValue(fakeResult());
    pasteRecord({ problem: "p3", thinking: "t3", answer: "a3" });

    await vi.waitFor(() => expect(analyzeTraceMock).toHaveBeenCalledTimes(3));
    expect(container.textContent).not.toContain("analysis failed");
  });

  it("announces the failure to assistive tech", () => {
    const { container } = render(<App />);

    analyzeTraceMock.mockImplementationOnce(() => {
      throw new Error("boom");
    });
    pasteRecord({ problem: "p", thinking: "t", answer: "a" });

    // role="alert" on a freshly-mounted node is the pattern screen readers
    // announce reliably; a purely visual error leaves a non-sighted user
    // staring at an app that silently did nothing.
    const error = container.querySelector<HTMLElement>(".analysis-error")!;
    expect(error).not.toBeNull();
    expect(error.getAttribute("role")).toBe("alert");
  });
});

describe("App: single analyze pipeline", () => {
  it("renders the anatomy view and share bar after a paste analysis", async () => {
    const { container } = render(<App />);

    pasteRecord({ problem: "p", thinking: "t", answer: "a" });

    await vi.waitFor(() => expect(container.querySelector(".anatomy")).not.toBeNull());
    expect(container.querySelector(".share-bar")).not.toBeNull();
  });

  it("renders through the same pipeline for a gallery card click", async () => {
    const index = [
      { id: "ex", label: "Example", description: "an example trace", file: "ex.json" },
    ];
    const fixture = {
      id: "ex",
      label: "Example",
      description: "an example trace",
      model: "m",
      problem: "P",
      thinking: "T",
      answer: "A",
      generated_with: { prompt: "P", options: {} },
      curated: false,
    };
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        if (url.endsWith("/gallery/index.json")) {
          return Promise.resolve(new Response(JSON.stringify(index), { status: 200 }));
        }
        if (url.endsWith("/gallery/ex.json")) {
          return Promise.resolve(new Response(JSON.stringify(fixture), { status: 200 }));
        }
        return Promise.resolve(new Response("not found", { status: 404 }));
      }),
    );

    const { container } = render(<App />);

    const card = await vi.waitFor(() => {
      const found = container.querySelector<HTMLButtonElement>("button.gallery-card");
      expect(found).not.toBeNull();
      return found!;
    });
    fireEvent.click(card);

    await vi.waitFor(() =>
      expect(analyzeTraceMock).toHaveBeenCalledWith({ id: "ex", problem: "P", thinking: "T", answer: "A" }),
    );
    await vi.waitFor(() => expect(container.querySelector(".anatomy")).not.toBeNull());
    expect(container.querySelector(".share-bar")).not.toBeNull();
  });
});

describe("App: compare tab", () => {
  it("lazily mounts the compare panel when the Compare tab is selected", () => {
    const { container } = render(<App />);

    const compareContainer = container.querySelector("#compare-panel-container")!;
    expect(compareContainer.hasAttribute("hidden")).toBe(true);
    // Not mounted until first visited — so the single-trace flow pays nothing
    // for it and there's only one paste box on the page.
    expect(container.querySelector(".compare-panel")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Compare" }));

    expect(compareContainer.hasAttribute("hidden")).toBe(false);
    expect(container.querySelector(".compare-panel")).not.toBeNull();
    expect(container.querySelector("#input-panel")!.hasAttribute("hidden")).toBe(true);
  });

  it("keeps a loaded slot when switching away and back", async () => {
    const { container } = render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Compare" }));

    const slotA = container.querySelectorAll<HTMLElement>(".compare-slot")[0];
    const textarea = within(slotA).getByPlaceholderText(/paste a trace/i);
    fireEvent.change(textarea, {
      target: { value: JSON.stringify({ problem: "p", thinking: "t", answer: "a" }) },
    });
    fireEvent.click(within(slotA).getByRole("button", { name: "Analyze" }));

    await vi.waitFor(() =>
      expect(container.querySelectorAll(".compare-slot")[0].querySelector(".anatomy")).not.toBeNull(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Paste" }));
    fireEvent.click(screen.getByRole("button", { name: "Compare" }));

    // The panel stayed mounted (just hidden), so the slot is still loaded.
    expect(container.querySelectorAll(".compare-slot")[0].querySelector(".anatomy")).not.toBeNull();
  });
});
