// @vitest-environment happy-dom

// Component tests for the Live (Ollama) panel: settings persistence, model
// picker population, CORS vs HTTP error rendering, start/stop streaming,
// and abort semantics. `listModels`/`streamChat` are mocked (real parsing
// behavior is covered directly against `streamChat` in ../lib/ollama.test.ts);
// `throttle`/`toTraceInput` are left real so the throttled wiring here is
// exercised end to end. "Activation" (the old panel.activate()) is now the
// `active` prop flipping true, driven via `rerender`.

import { fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OllamaHttpError } from "../lib/ollama";
import type { TraceInput } from "../lib/types";
import { COMPARE_STALL_MS, LivePanel } from "./LivePanel";

vi.mock("../lib/ollama", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ollama")>();
  return {
    ...actual,
    listModels: vi.fn(),
    streamChat: vi.fn(),
  };
});

// The real analyzeTrace needs the wasm module; compare mode calls it directly
// (single-model mode still routes through onAnalyze and never touches it).
vi.mock("../lib/wasm", () => ({
  initWasm: vi.fn(),
  analyzeTrace: vi.fn(),
}));

import { listModels, streamChat } from "../lib/ollama";
import { analyzeTrace } from "../lib/wasm";
import type { AnalysisResult, ScoredTrace } from "../lib/types";

const listModelsMock = vi.mocked(listModels);
const streamChatMock = vi.mocked(streamChat);
const analyzeTraceMock = vi.mocked(analyzeTrace);

function fakeResult(rec: TraceInput): AnalysisResult {
  return {
    composite: rec.thinking.length,
    scores: [{ name: "structure", score: 50, weight: 0.1, diagnostics: [] }],
    annotations: [],
    tokenCount: 1,
    extractedThinking: rec.thinking,
    scored: { quality_score: rec.thinking.length } as ScoredTrace,
  };
}

function setup(initialActive = false) {
  const onAnalyze = vi.fn<(record: TraceInput) => void>();
  const { container, rerender } = render(<LivePanel onAnalyze={onAnalyze} active={initialActive} />);

  function activate() {
    rerender(<LivePanel onAnalyze={onAnalyze} active={true} />);
  }

  return {
    container,
    onAnalyze,
    activate,
    rerender,
    baseUrlInput: container.querySelector<HTMLInputElement>("input.live-base-url")!,
    modelSelect: container.querySelector<HTMLSelectElement>("select.live-model")!,
    promptBox: container.querySelector<HTMLTextAreaElement>("textarea.live-prompt")!,
    startButton: container.querySelector<HTMLButtonElement>("button.live-start")!,
    errorArea: container.querySelector<HTMLParagraphElement>("p.live-error")!,
    refreshButton: container.querySelector<HTMLButtonElement>("button.live-refresh")!,
  };
}

beforeEach(() => {
  localStorage.clear();
  listModelsMock.mockReset().mockResolvedValue(["llama3", "tinyllama"]);
  streamChatMock.mockReset();
  analyzeTraceMock.mockReset().mockImplementation(fakeResult);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("LivePanel: settings", () => {
  it("defaults the base URL to localhost:11434 without loading models on mount", () => {
    const { baseUrlInput } = setup();

    expect(baseUrlInput.value).toBe("http://localhost:11434");
    expect(listModelsMock).not.toHaveBeenCalled();
  });

  it("restores a persisted base URL from localStorage without loading models on mount", () => {
    localStorage.setItem("reasonmetrics.ollama.baseUrl", "http://localhost:9999");
    localStorage.setItem("reasonmetrics.ollama.model", "tinyllama");
    listModelsMock.mockResolvedValue(["llama3", "tinyllama"]);

    const { baseUrlInput } = setup();

    expect(baseUrlInput.value).toBe("http://localhost:9999");
    expect(listModelsMock).not.toHaveBeenCalled();
  });

  it("persists base URL and model changes to localStorage", async () => {
    const { activate, baseUrlInput, modelSelect } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));

    fireEvent.change(baseUrlInput, { target: { value: "http://localhost:12345" } });
    fireEvent.change(modelSelect, { target: { value: "tinyllama" } });

    expect(localStorage.getItem("reasonmetrics.ollama.baseUrl")).toBe("http://localhost:12345");
    expect(localStorage.getItem("reasonmetrics.ollama.model")).toBe("tinyllama");
  });
});

describe("LivePanel: deferred model probe (no auto-connect on load)", () => {
  it("does not call listModels (no network probe) at mount time", () => {
    setup();
    expect(listModelsMock).not.toHaveBeenCalled();
  });

  it("loads models on first activation, and the Refresh button still works before activation", async () => {
    const { activate, modelSelect } = setup();
    expect(listModelsMock).not.toHaveBeenCalled();

    activate();
    await vi.waitFor(() => expect(listModelsMock).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
  });

  it("does not re-probe on repeated activations", async () => {
    const { rerender, onAnalyze } = setup();

    rerender(<LivePanel onAnalyze={onAnalyze} active={true} />);
    await vi.waitFor(() => expect(listModelsMock).toHaveBeenCalledTimes(1));

    rerender(<LivePanel onAnalyze={onAnalyze} active={false} />);
    rerender(<LivePanel onAnalyze={onAnalyze} active={true} />);
    expect(listModelsMock).toHaveBeenCalledTimes(1);
  });

  it("the Refresh models button still probes explicitly, independent of activation", async () => {
    const { refreshButton } = setup();
    expect(listModelsMock).not.toHaveBeenCalled();

    fireEvent.click(refreshButton);
    await vi.waitFor(() => expect(listModelsMock).toHaveBeenCalledTimes(1));
  });
});

describe("LivePanel: storage resilience", () => {
  it("mounts with defaults when localStorage throws on access (e.g. blocked storage)", () => {
    const getItemSpy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });
    const setItemSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });

    try {
      const onAnalyze = vi.fn<(record: TraceInput) => void>();
      let container!: HTMLElement;

      expect(() => {
        container = render(<LivePanel onAnalyze={onAnalyze} active={false} />).container;
      }).not.toThrow();

      const baseUrlInput = container.querySelector<HTMLInputElement>("input.live-base-url")!;
      expect(baseUrlInput.value).toBe("http://localhost:11434");
    } finally {
      getItemSpy.mockRestore();
      setItemSpy.mockRestore();
    }
  });

  it("does not throw when persisting settings while storage is blocked", async () => {
    const { activate, baseUrlInput, modelSelect } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));

    const setItemSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The operation is insecure.", "SecurityError");
    });

    try {
      expect(() =>
        fireEvent.change(baseUrlInput, { target: { value: "http://localhost:12345" } }),
      ).not.toThrow();
    } finally {
      setItemSpy.mockRestore();
    }
  });
});

describe("LivePanel: error rendering", () => {
  it("renders the OLLAMA_ORIGINS instruction with the real origin on a network/CORS error", async () => {
    listModelsMock.mockRejectedValue(new TypeError("Failed to fetch"));

    const { activate, errorArea } = setup();
    activate();

    await vi.waitFor(() => expect(errorArea.hidden).toBe(false));
    expect(errorArea.textContent).toContain(`OLLAMA_ORIGINS=${location.origin} ollama serve`);
    expect(errorArea.textContent?.toLowerCase()).toContain("cross-origin");
  });

  it("shows the HTTP status for a non-2xx response without the CORS instructions", async () => {
    listModelsMock.mockRejectedValue(new OllamaHttpError(500, "boom"));

    const { activate, errorArea } = setup();
    activate();

    await vi.waitFor(() => expect(errorArea.hidden).toBe(false));
    expect(errorArea.textContent).toContain("500");
    expect(errorArea.textContent).not.toContain("OLLAMA_ORIGINS");
  });
});

describe("LivePanel: no model selected guard", () => {
  it("shows an inline error and never calls streamChat when Start is clicked with no model loaded", () => {
    const { promptBox, startButton, errorArea } = setup();
    // Deliberately never activated: the model list never loaded, so
    // selectedModel is still "".

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);

    expect(streamChatMock).not.toHaveBeenCalled();
    expect(errorArea.hidden).toBe(false);
    expect(errorArea.textContent?.toLowerCase()).toContain("no model selected");
    expect(errorArea.textContent?.toLowerCase()).toContain("refresh models");
    expect(startButton.textContent).toBe("Start");
  });
});

describe("LivePanel: streaming", () => {
  it("streams deltas through to onAnalyze using the thinking/content trace-assembly rule", async () => {
    let captured: Parameters<typeof streamChat>[0] | undefined;
    streamChatMock.mockImplementation(async (opts) => {
      captured = opts;
      opts.onDelta({ thinking: "reasoning so far", content: "" });
      opts.onDone({ thinking: "reasoning so far", content: "final answer" });
    });

    const { activate, promptBox, modelSelect, startButton, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);

    await vi.waitFor(() => expect(onAnalyze).toHaveBeenCalledTimes(2));
    expect(onAnalyze).toHaveBeenNthCalledWith(1, {
      problem: "2+2?",
      thinking: "reasoning so far",
      answer: "",
    });
    expect(onAnalyze).toHaveBeenNthCalledWith(2, {
      problem: "2+2?",
      thinking: "reasoning so far",
      answer: "final answer",
    });
    expect(captured?.model).toBe("llama3");
    expect(captured?.baseUrl).toBe("http://localhost:11434");
  });

  it("falls back to accumulated content as thinking when no thinking fragments ever arrive", async () => {
    streamChatMock.mockImplementation(async (opts) => {
      opts.onDelta({ thinking: "", content: "partial" });
      opts.onDone({ thinking: "", content: "partial answer" });
    });

    const { activate, promptBox, modelSelect, startButton, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));

    fireEvent.change(promptBox, { target: { value: "hi" } });
    fireEvent.click(startButton);

    await vi.waitFor(() => expect(onAnalyze).toHaveBeenCalledTimes(2));
    expect(onAnalyze).toHaveBeenNthCalledWith(2, {
      problem: "hi",
      thinking: "partial answer",
      answer: "",
    });
  });

  it("swaps Start to Stop while streaming and back to Start when done", async () => {
    let resolveStream: () => void;
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve) => {
          resolveStream = () => {
            opts.onDone({ thinking: "", content: "x" });
            resolve();
          };
        }),
    );

    const { activate, promptBox, modelSelect, startButton } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    fireEvent.change(promptBox, { target: { value: "hi" } });

    fireEvent.click(startButton);
    await vi.waitFor(() => expect(startButton.textContent).toBe("Stop"));

    resolveStream!();
    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
  });

  it("cancels a pending trailing analysis synchronously at Stop time", async () => {
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve, reject) => {
          // Two rapid deltas: the first fires the throttle's leading edge,
          // the second leaves a trailing call pending when Stop is clicked.
          opts.onDelta({ thinking: "", content: "a" });
          opts.onDelta({ thinking: "", content: "ab" });
          opts.signal?.addEventListener("abort", () => {
            reject(new DOMException("aborted", "AbortError"));
          });
          void resolve;
        }),
    );

    const { activate, promptBox, modelSelect, startButton, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    vi.useFakeTimers();
    fireEvent.change(promptBox, { target: { value: "hi" } });

    fireEvent.click(startButton); // Start
    expect(onAnalyze).toHaveBeenCalledTimes(1); // leading edge only

    fireEvent.click(startButton); // Stop
    vi.advanceTimersByTime(1000);
    expect(onAnalyze).toHaveBeenCalledTimes(1); // trailing call must not fire

    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
    expect(onAnalyze).toHaveBeenCalledTimes(1);
  });

  it("aborts via the Stop button, keeps the last analysis, and shows no error", async () => {
    let signalRef: AbortSignal | undefined;
    let rejectStream: (err: unknown) => void;
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve, reject) => {
          signalRef = opts.signal;
          rejectStream = reject;
          opts.onDelta({ thinking: "", content: "partial" });
          opts.signal?.addEventListener("abort", () => {
            reject(new DOMException("aborted", "AbortError"));
          });
          void resolve;
        }),
    );

    const { activate, promptBox, modelSelect, startButton, errorArea, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    fireEvent.change(promptBox, { target: { value: "hi" } });

    fireEvent.click(startButton);
    await vi.waitFor(() => expect(onAnalyze).toHaveBeenCalledTimes(1));

    fireEvent.click(startButton); // Stop
    expect(signalRef?.aborted).toBe(true);
    void rejectStream!;

    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
    expect(errorArea.hidden).toBe(true);
    expect(onAnalyze).toHaveBeenCalledTimes(1);
  });
});

describe("LivePanel: compare mode", () => {
  function enableCompare(container: HTMLElement) {
    const toggle = container.querySelector<HTMLInputElement>(".live-compare-toggle input")!;
    fireEvent.click(toggle);
    return toggle;
  }

  it("shows the second model select when toggled, defaulting to the second model", async () => {
    const { container, activate, modelSelect } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));

    expect(container.querySelector(".live-model-b")).toBeNull();
    enableCompare(container);

    const modelBSelect = container.querySelector<HTMLSelectElement>("select.live-model-b")!;
    expect(modelBSelect.value).toBe("tinyllama");

    fireEvent.change(modelBSelect, { target: { value: "llama3" } });
    expect(localStorage.getItem("reasonmetrics.ollama.modelB")).toBe("llama3");
  });

  it("streams both models on Start, renders the side-by-side table, and leaves the main pipeline alone", async () => {
    streamChatMock.mockImplementation(async (opts) => {
      opts.onDone({ thinking: `thoughts:${opts.model}`, content: "ans" });
    });

    const { container, activate, promptBox, modelSelect, startButton, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);

    await vi.waitFor(() => expect(streamChatMock).toHaveBeenCalledTimes(2));
    const streamedModels = streamChatMock.mock.calls.map(([opts]) => opts.model).sort();
    expect(streamedModels).toEqual(["llama3", "tinyllama"]);

    await vi.waitFor(() =>
      expect(container.querySelector(".compare-scores")).not.toBeNull(),
    );
    // fakeResult sets composite = thinking.length, so both sides rendered
    // "thoughts:<model>".length as the composite.
    const composite = container.querySelector(".compare-composite")!;
    expect(composite.textContent).toContain(`${"thoughts:llama3".length}.0`);
    expect(composite.textContent).toContain(`${"thoughts:tinyllama".length}.0`);

    // Compare mode renders inline — the shared detail pipeline is not driven.
    expect(onAnalyze).not.toHaveBeenCalled();
  });

  it("opens a side in the detail view via onAnalyze with that side's assembled trace", async () => {
    streamChatMock.mockImplementation(async (opts) => {
      opts.onDone({ thinking: `thoughts:${opts.model}`, content: "ans" });
    });

    const { container, activate, promptBox, modelSelect, startButton, onAnalyze } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);
    // The table renders as soon as the run starts, so wait for B's own turn to
    // finish — its "Open in detail" button stays disabled until it has a result.
    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));

    fireEvent.click(container.querySelector<HTMLButtonElement>("button.live-open-b")!);
    expect(onAnalyze).toHaveBeenCalledTimes(1);
    expect(onAnalyze).toHaveBeenCalledWith({
      problem: "2+2?",
      thinking: "thoughts:tinyllama",
      answer: "ans",
    });
  });

  it("streams the two models one at a time, never concurrently", async () => {
    // A single local Ollama can rarely hold two models at once: racing them
    // starves one side, which produces no bytes at all until the other is
    // done. Compare mode must therefore serialize the two runs.
    const started: string[] = [];
    const finishers: (() => void)[] = [];
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve) => {
          started.push(opts.model);
          finishers.push(() => {
            opts.onDone({ thinking: `thoughts:${opts.model}`, content: "ans" });
            resolve();
          });
        }),
    );

    const { container, activate, promptBox, modelSelect, startButton } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);

    // Only model A is in flight; B must not have been requested yet.
    await vi.waitFor(() => expect(started).toEqual(["llama3"]));
    expect(streamChatMock).toHaveBeenCalledTimes(1);

    finishers[0]();
    await vi.waitFor(() => expect(started).toEqual(["llama3", "tinyllama"]));

    finishers[1]();
    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
  });

  it("renders side A's scores while side B is still queued, with per-side status", async () => {
    const finishers: (() => void)[] = [];
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve) => {
          finishers.push(() => {
            opts.onDone({ thinking: `thoughts:${opts.model}`, content: "ans" });
            resolve();
          });
        }),
    );

    const { container, activate, promptBox, modelSelect, startButton } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    fireEvent.click(startButton);

    const statusA = () => container.querySelector(".live-status-a")!.textContent ?? "";
    const statusB = () => container.querySelector(".live-status-b")!.textContent ?? "";

    await vi.waitFor(() => expect(statusA()).toContain("streaming"));
    expect(statusB()).toContain("waiting");

    // A finishes: its column must render immediately, not wait for B.
    finishers[0]();
    await vi.waitFor(() => expect(statusA()).toContain("done"));
    expect(container.querySelector(".compare-composite")!.textContent).toContain(
      `${"thoughts:llama3".length}.0`,
    );
    await vi.waitFor(() => expect(statusB()).toContain("streaming"));

    finishers[1]();
    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
  });

  it("abandons a side that never responds after the stall timeout, then runs the other", async () => {
    const signals: Record<string, AbortSignal | undefined> = {};
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((resolve, reject) => {
          signals[opts.model] = opts.signal;
          if (opts.model === "llama3") {
            // Model A never emits a single byte (Ollama queued it behind a
            // model it cannot co-load) — exactly the observed hang.
            opts.signal?.addEventListener("abort", () => {
              reject(new DOMException("aborted", "AbortError"));
            });
            return;
          }
          opts.onDone({ thinking: `thoughts:${opts.model}`, content: "ans" });
          resolve();
        }),
    );

    const { container, activate, promptBox, modelSelect, startButton } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "2+2?" } });
    vi.useFakeTimers();
    fireEvent.click(startButton);

    await vi.waitFor(() => expect(signals["llama3"]).toBeDefined());
    expect(signals["llama3"]?.aborted).toBe(false);

    vi.advanceTimersByTime(COMPARE_STALL_MS + 1000);
    await vi.waitFor(() => expect(signals["llama3"]?.aborted).toBe(true));

    // The stalled side is reported, and B still gets its turn.
    await vi.waitFor(() =>
      expect(container.querySelector(".live-status-a")!.textContent?.toLowerCase()).toContain(
        "no response",
      ),
    );
    await vi.waitFor(() => expect(signals["tinyllama"]).toBeDefined());
    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
  });

  it("Stop aborts the in-flight side and never starts the second model", async () => {
    const started: string[] = [];
    const signals: (AbortSignal | undefined)[] = [];
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((_resolve, reject) => {
          started.push(opts.model);
          signals.push(opts.signal);
          opts.signal?.addEventListener("abort", () => {
            reject(new DOMException("aborted", "AbortError"));
          });
        }),
    );

    const { container, activate, promptBox, modelSelect, startButton, errorArea } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "hi" } });
    fireEvent.click(startButton);
    await vi.waitFor(() => expect(started).toEqual(["llama3"]));

    fireEvent.click(startButton); // Stop
    expect(signals.every((signal) => signal?.aborted)).toBe(true);

    await vi.waitFor(() => expect(startButton.textContent).toBe("Start"));
    expect(started).toEqual(["llama3"]); // B must never have been requested
    expect(errorArea.hidden).toBe(true);
  });

  it("keeps the toggle disabled while streaming", async () => {
    streamChatMock.mockImplementation(
      (opts) =>
        new Promise<void>((_resolve, reject) => {
          opts.signal?.addEventListener("abort", () => {
            reject(new DOMException("aborted", "AbortError"));
          });
        }),
    );

    const { container, activate, promptBox, modelSelect, startButton } = setup();
    activate();
    await vi.waitFor(() => expect(modelSelect.children).toHaveLength(2));
    const toggle = enableCompare(container);

    fireEvent.change(promptBox, { target: { value: "hi" } });
    fireEvent.click(startButton);
    await vi.waitFor(() => expect(startButton.textContent).toBe("Stop"));
    expect(toggle.disabled).toBe(true);

    fireEvent.click(startButton); // Stop
    await vi.waitFor(() => expect(toggle.disabled).toBe(false));
  });
});
