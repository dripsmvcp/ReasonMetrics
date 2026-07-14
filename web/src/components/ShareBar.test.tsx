// @vitest-environment happy-dom

// Component tests for the share toolbar: PNG export (footer append/remove
// around a stubbed capture, download filename) and copy-share-link (hash +
// optional clipboard, over-cap file-download fallback). The real
// html-to-image rasterization can't run under happy-dom, so `capture` is
// always a stub here — see ../lib/share.test.ts for the pure encode/decode/
// filename coverage this toolbar builds on.

import { fireEvent, render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AnalysisResult, ScoreEntry, ScoredTrace, TraceInput } from "../lib/types";
import { ShareBar } from "./ShareBar";

function makeScored(): ScoredTrace {
  return {
    id: "t1",
    problem: "p",
    thinking: "t",
    answer: "a",
    quality_score: 82,
    raw_score: 95,
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

function makeResult(composite = 82.5): AnalysisResult {
  return {
    composite,
    scores: [] as ScoreEntry[],
    annotations: [],
    tokenCount: 10,
    extractedThinking: "hello",
    scored: makeScored(),
  };
}

function randomAscii(length: number): string {
  const alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let out = "";
  for (let i = 0; i < length; i++) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)];
  }
  return out;
}

function setup(
  trace: TraceInput,
  result: AnalysisResult,
  capture: (node: HTMLElement) => Promise<string>,
  model?: string,
) {
  const captureTargetRef = createRef<HTMLElement | null>();
  (captureTargetRef as { current: HTMLElement | null }).current = document.createElement("div");
  captureTargetRef.current!.className = "anatomy";

  const { container } = render(
    <ShareBar
      captureTargetRef={captureTargetRef}
      trace={trace}
      result={result}
      capture={capture}
      model={model}
    />,
  );
  return {
    container,
    captureTarget: captureTargetRef.current!,
    exportButton: container.querySelector<HTMLButtonElement>("button.share-export-btn")!,
    copyButton: container.querySelector<HTMLButtonElement>("button.share-copy-btn")!,
    status: container.querySelector<HTMLElement>(".share-status")!,
  };
}

let clickedAnchors: HTMLAnchorElement[];

beforeEach(() => {
  location.hash = "";
  clickedAnchors = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    clickedAnchors.push(this);
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  location.hash = "";
});

describe("ShareBar: toolbar", () => {
  it("renders an export-png and copy-share-link button", () => {
    const { exportButton, copyButton } = setup(
      { problem: "p", thinking: "t", answer: "a" },
      makeResult(),
      vi.fn(),
    );
    expect(exportButton.textContent).toBe("export png");
    expect(copyButton.textContent).toBe("copy share link");
  });
});

describe("ShareBar: export png", () => {
  it("appends an app-URL footer before capture and removes it after, then downloads the file", async () => {
    let footerTextDuringCapture = "";
    const capture = vi.fn(async (node: HTMLElement) => {
      footerTextDuringCapture = node.querySelector(".share-export-footer")?.textContent ?? "";
      return "data:image/png;base64,stub";
    });

    const trace: TraceInput = { id: "trace-7", problem: "p", thinking: "t", answer: "a" };
    const { exportButton, captureTarget } = setup(trace, makeResult(82.567), capture);

    fireEvent.click(exportButton);
    await vi.waitFor(() => expect(capture).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(clickedAnchors).toHaveLength(1));

    expect(footerTextDuringCapture).toBe(location.origin + location.pathname);
    expect(captureTarget.querySelector(".share-export-footer")).toBeNull();

    const anchor = clickedAnchors[0];
    expect(anchor.href).toBe("data:image/png;base64,stub");
    expect(anchor.download).toBe("reasonmetrics-trace-7-82.6.png");
  });

  it("names the export after the model when one is known (live mode)", async () => {
    const capture = vi.fn(async () => "data:image/png;base64,stub");
    const trace: TraceInput = { id: "live", problem: "p", thinking: "t", answer: "a" };
    const { exportButton } = setup(trace, makeResult(82.567), capture, "qwen3:1.7b");

    fireEvent.click(exportButton);
    await vi.waitFor(() => expect(clickedAnchors).toHaveLength(1));

    // The model wins over the trace id — that is the whole point of the
    // `model` branch in exportFilename, which no caller reached before.
    expect(clickedAnchors[0].download).toBe("reasonmetrics-qwen3-1.7b-82.6.png");
  });

  it("shows a failure status and still removes the footer when capture rejects", async () => {
    const capture = vi.fn(async () => {
      throw new Error("canvas tainted");
    });
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const trace: TraceInput = { problem: "p", thinking: "t", answer: "a" };
    const { exportButton, status, captureTarget } = setup(trace, makeResult(), capture);

    fireEvent.click(exportButton);
    await vi.waitFor(() => expect(status.textContent).toBe("could not export png"));

    expect(captureTarget.querySelector(".share-export-footer")).toBeNull();
    expect(clickedAnchors).toHaveLength(0);
    warnSpy.mockRestore();
  });
});

describe("ShareBar: copy share link", () => {
  it("sets location.hash and copies location.href via the clipboard API", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });

    const trace: TraceInput = { problem: "p", thinking: "t", answer: "a" };
    const { copyButton, status } = setup(trace, makeResult(), vi.fn());

    fireEvent.click(copyButton);
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));

    expect(location.hash).toMatch(/^#t=.+/);
    expect(writeText).toHaveBeenCalledWith(location.href);
    await vi.waitFor(() => expect(status.textContent).toBe("copied"));

    vi.unstubAllGlobals();
  });

  it("shows the URL for manual copy when the clipboard API is unavailable", async () => {
    vi.stubGlobal("navigator", { ...navigator, clipboard: undefined });

    const trace: TraceInput = { problem: "p", thinking: "t", answer: "a" };
    const { copyButton, status } = setup(trace, makeResult(), vi.fn());

    fireEvent.click(copyButton);
    await vi.waitFor(() => expect(location.hash).toMatch(/^#t=.+/));

    await vi.waitFor(() => expect(status.textContent).toBe(location.href));

    vi.unstubAllGlobals();
  });

  it("skips the hash/clipboard and downloads a .json file when the trace is over the 30KB cap", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });

    const trace: TraceInput = { id: "big", problem: "p", thinking: randomAscii(28_000), answer: "a" };
    const { copyButton, status } = setup(trace, makeResult(), vi.fn());

    fireEvent.click(copyButton);
    await vi.waitFor(() => expect(clickedAnchors).toHaveLength(1));

    expect(location.hash).toBe("");
    expect(writeText).not.toHaveBeenCalled();
    await vi.waitFor(() =>
      expect(status.textContent).toMatch(
        /too large for a link \(\d+ KB compressed\) — downloading file instead/,
      ),
    );

    const anchor = clickedAnchors[0];
    expect(anchor.download).toBe("reasonmetrics-big-82.5.json");
    expect(anchor.href).toMatch(/^blob:/);

    vi.unstubAllGlobals();
  });
});
