// The "Live" tab: Ollama base URL / model settings, a prompt box, and
// start/stop streaming with a throttled re-analysis loop. NDJSON parsing,
// the throttle helper, and the trace-assembly rule live in ../lib/ollama.ts
// (pure, unit-tested there); this component only builds the UI, persists
// settings via ../lib/storage.ts, and turns stream events into onAnalyze calls.

import { useEffect, useRef, useState } from "react";
import {
  listModels,
  OllamaHttpError,
  streamChat,
  throttle,
  toTraceInput,
  type Throttled,
} from "../lib/ollama";
import { readStorage, writeStorage } from "../lib/storage";
import { analyzeTrace } from "../lib/wasm";
import type { AnalysisResult, TraceInput } from "../lib/types";
import { CompareScores } from "./CompareScores";

const STORAGE_BASE_URL = "reasonmetrics.ollama.baseUrl";
const STORAGE_MODEL = "reasonmetrics.ollama.model";
const STORAGE_MODEL_B = "reasonmetrics.ollama.modelB";
const DEFAULT_BASE_URL = "http://localhost:11434";
const THROTTLE_MS = 500;

interface LivePanelProps {
  onAnalyze: (record: TraceInput) => void;
  /** True while the Live tab is the active mode. Drives the one-time
   * model-list fetch on first activation — never at mount, and never more
   * than once for the component's lifetime — matching the "no network
   * without user interaction" rule. The Refresh button still probes
   * explicitly at any time regardless of activation state. */
  active: boolean;
}

function isAbortError(err: unknown): boolean {
  return err instanceof DOMException && err.name === "AbortError";
}

export function LivePanel({ onAnalyze, active }: LivePanelProps) {
  const [baseUrl, setBaseUrl] = useState(() => readStorage(STORAGE_BASE_URL) ?? DEFAULT_BASE_URL);
  const [models, setModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [compareMode, setCompareMode] = useState(false);
  const [modelB, setModelB] = useState("");
  const [resultA, setResultA] = useState<AnalysisResult | null>(null);
  const [resultB, setResultB] = useState<AnalysisResult | null>(null);

  // Mutable refs, not state: Stop must see and cancel the in-flight
  // controller/throttle synchronously, in the same tick as the click — an
  // extra render cycle here would leave a window where a stale trailing
  // analysis (or a duplicate stream) could fire after Stop.
  const controllerRef = useRef<AbortController | null>(null);
  const activeThrottlesRef = useRef<Throttled<[TraceInput]>[]>([]);
  const activatedRef = useRef(false);
  const baseUrlRef = useRef(baseUrl);
  baseUrlRef.current = baseUrl;
  const selectedModelRef = useRef(selectedModel);
  selectedModelRef.current = selectedModel;
  const modelBRef = useRef(modelB);
  modelBRef.current = modelB;
  // Last trace assembled per compare side, for "Open in detail".
  const recordARef = useRef<TraceInput | null>(null);
  const recordBRef = useRef<TraceInput | null>(null);

  function showConnectionError(err: unknown): void {
    if (err instanceof OllamaHttpError) {
      setError(`Ollama returned an error: HTTP ${err.status}`);
    } else if (err instanceof TypeError) {
      setError(
        `Could not reach Ollama at ${baseUrlRef.current}. ` +
          `Ollama blocks cross-origin requests by default — ` +
          `run OLLAMA_ORIGINS=${location.origin} ollama serve`,
      );
    } else {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function refreshModels(): Promise<void> {
    setError(null);
    try {
      const list = await listModels(baseUrlRef.current);
      const savedModel = readStorage(STORAGE_MODEL);
      const savedModelB = readStorage(STORAGE_MODEL_B);
      setModels(list);
      setSelectedModel(savedModel && list.includes(savedModel) ? savedModel : (list[0] ?? ""));
      setModelB(
        savedModelB && list.includes(savedModelB) ? savedModelB : (list[1] ?? list[0] ?? ""),
      );
    } catch (err) {
      showConnectionError(err);
    }
  }

  useEffect(() => {
    if (active && !activatedRef.current) {
      activatedRef.current = true;
      void refreshModels();
    }
    // Deliberately mount/active-flip only: refreshModels/showConnectionError
    // close over refs, not reactive state, so omitting them is safe, and
    // this must fire at most once ever regardless of how often `active`
    // toggles.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  function handleBaseUrlChange(value: string): void {
    setBaseUrl(value);
    writeStorage(STORAGE_BASE_URL, value);
  }

  function handleModelChange(value: string): void {
    setSelectedModel(value);
    if (value) writeStorage(STORAGE_MODEL, value);
  }

  function handleModelBChange(value: string): void {
    setModelB(value);
    if (value) writeStorage(STORAGE_MODEL_B, value);
  }

  async function startStreaming(): Promise<void> {
    setError(null);
    writeStorage(STORAGE_BASE_URL, baseUrlRef.current);
    if (selectedModelRef.current) writeStorage(STORAGE_MODEL, selectedModelRef.current);

    const controller = new AbortController();
    controllerRef.current = controller;
    setStreaming(true);

    const analyzeThrottled = throttle(onAnalyze, THROTTLE_MS);
    activeThrottlesRef.current = [analyzeThrottled];
    const currentPrompt = prompt;

    try {
      await streamChat({
        baseUrl: baseUrlRef.current,
        model: selectedModelRef.current,
        prompt: currentPrompt,
        signal: controller.signal,
        onDelta: (delta) => analyzeThrottled(toTraceInput(currentPrompt, delta)),
        onDone: (final) => {
          analyzeThrottled.cancel();
          onAnalyze(toTraceInput(currentPrompt, final));
        },
      });
    } catch (err) {
      if (!isAbortError(err)) {
        showConnectionError(err);
      }
    } finally {
      analyzeThrottled.cancel();
      activeThrottlesRef.current = [];
      controllerRef.current = null;
      setStreaming(false);
    }
  }

  /** One compare side: stream a model, re-scoring the partial trace through
   * the same wasm analyzeTrace the main pipeline uses, into that side's
   * result slot. Mid-stream analysis failures are ignored (the last good
   * tick stays on screen, matching the single-model keep-last semantics). */
  async function streamCompareSide(
    model: string,
    currentPrompt: string,
    controller: AbortController,
    setResult: (result: AnalysisResult) => void,
    recordRef: { current: TraceInput | null },
  ): Promise<void> {
    const analyze = (rec: TraceInput) => {
      recordRef.current = rec;
      try {
        setResult(analyzeTrace(rec));
      } catch {
        /* keep the previous tick's result */
      }
    };
    const analyzeThrottled = throttle(analyze, THROTTLE_MS);
    activeThrottlesRef.current.push(analyzeThrottled);

    try {
      await streamChat({
        baseUrl: baseUrlRef.current,
        model,
        prompt: currentPrompt,
        signal: controller.signal,
        onDelta: (delta) => analyzeThrottled(toTraceInput(currentPrompt, delta)),
        onDone: (final) => {
          analyzeThrottled.cancel();
          analyze(toTraceInput(currentPrompt, final));
        },
      });
    } catch (err) {
      if (!isAbortError(err)) {
        showConnectionError(err);
      }
    } finally {
      analyzeThrottled.cancel();
    }
  }

  async function startComparing(): Promise<void> {
    setError(null);
    writeStorage(STORAGE_BASE_URL, baseUrlRef.current);
    if (selectedModelRef.current) writeStorage(STORAGE_MODEL, selectedModelRef.current);
    if (modelBRef.current) writeStorage(STORAGE_MODEL_B, modelBRef.current);

    const controller = new AbortController();
    controllerRef.current = controller;
    setStreaming(true);
    setResultA(null);
    setResultB(null);
    recordARef.current = null;
    recordBRef.current = null;
    activeThrottlesRef.current = [];
    const currentPrompt = prompt;

    try {
      await Promise.all([
        streamCompareSide(selectedModelRef.current, currentPrompt, controller, setResultA, recordARef),
        streamCompareSide(modelBRef.current, currentPrompt, controller, setResultB, recordBRef),
      ]);
    } finally {
      activeThrottlesRef.current = [];
      controllerRef.current = null;
      setStreaming(false);
    }
  }

  function handleStartStopClick(): void {
    if (controllerRef.current) {
      for (const throttled of activeThrottlesRef.current) throttled.cancel();
      controllerRef.current.abort();
      return;
    }
    if (prompt.trim().length === 0) return;
    if (!selectedModelRef.current) {
      setError('No model selected — click "Refresh models" to load the list.');
      return;
    }
    if (compareMode) {
      if (!modelBRef.current) {
        setError('No second model selected — click "Refresh models" to load the list.');
        return;
      }
      void startComparing();
      return;
    }
    void startStreaming();
  }

  return (
    <div className="live-panel">
      <div className="live-settings">
        <label className="live-base-url-label">
          Ollama base URL{" "}
          <input
            type="text"
            className="live-base-url"
            value={baseUrl}
            onChange={(event) => handleBaseUrlChange(event.target.value)}
          />
        </label>

        <select
          className="live-model"
          value={selectedModel}
          onChange={(event) => handleModelChange(event.target.value)}
        >
          {models.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>

        {compareMode && (
          <select
            className="live-model-b"
            value={modelB}
            onChange={(event) => handleModelBChange(event.target.value)}
          >
            {models.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        )}

        <button type="button" className="live-refresh" onClick={() => void refreshModels()}>
          Refresh models
        </button>

        <label className="live-compare-toggle">
          <input
            type="checkbox"
            checked={compareMode}
            disabled={streaming}
            onChange={(event) => setCompareMode(event.target.checked)}
          />{" "}
          Compare two models
        </label>
      </div>

      <textarea
        className="live-prompt"
        placeholder="Prompt to send to the model"
        value={prompt}
        onChange={(event) => setPrompt(event.target.value)}
      />

      <button type="button" className="live-start" onClick={handleStartStopClick}>
        {streaming ? "Stop" : "Start"}
      </button>

      <p className="live-error" hidden={!error}>
        {error ?? ""}
      </p>

      {compareMode && (resultA || resultB) && (
        <div className="live-compare">
          <CompareScores labelA={selectedModel} labelB={modelB} a={resultA} b={resultB} />
          <div className="live-compare-actions">
            <button
              type="button"
              className="live-open-a"
              disabled={!resultA}
              onClick={() => recordARef.current && onAnalyze(recordARef.current)}
            >
              {`Open ${selectedModel || "A"} in detail`}
            </button>
            <button
              type="button"
              className="live-open-b"
              disabled={!resultB}
              onClick={() => recordBRef.current && onAnalyze(recordBRef.current)}
            >
              {`Open ${modelB || "B"} in detail`}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
