// Bridge to the reasonmetrics-wasm build. Every later feature (trace input UI,
// anatomy view, Ollama streaming, share links) calls through analyzeTrace,
// so this file is the only place that knows the wasm-bindgen loading and
// JSON-shape details.

import init, {
  analyze as wasmAnalyze,
  registry_json as wasmRegistryJson,
} from "../pkg/reasonmetrics_wasm.js";
import type {
  AnalysisResult,
  Annotation,
  ScoreEntry,
  ScoredTrace,
  TraceInput,
} from "./types";

/** Raw JSON shape returned by the wasm `analyze()` binding. */
interface RawAnalyzeOutput {
  scored: ScoredTrace;
  extracted_thinking: string;
  annotations: Annotation[];
  scores: ScoreEntry[];
}

/** The subset of a registry entry the web app reads. Mirrors
 * `reasonmetrics_core::registry::RegistryEntry`'s serde shape; only the fields
 * consumed here are typed. */
interface RawRegistryEntry {
  id: string;
  display_name: string;
  cost?: { input_per_mtok: number; output_per_mtok: number; source: string };
}

/** A per-family cost preset for the anatomy view's cost meter. `outputPerMtok`
 * is the rate that matters here: the meter counts *thinking* tokens, which are
 * generated (output) tokens, so the cost of producing a reasoning trace is
 * output-priced. Sourced from the embedded registry so the CLI, wasm, and web
 * share one cost table. */
export interface CostPreset {
  id: string;
  label: string;
  outputPerMtok: number;
  inputPerMtok: number;
  source: string;
}

let costPresetsCache: CostPreset[] | null = null;

let readyPromise: Promise<void> | null = null;

/**
 * Load the wasm module. Idempotent: the first call starts loading, every
 * later call awaits the same promise.
 *
 * wasm-bindgen's `--target web` output defaults to
 * `fetch(new URL('reasonmetrics_wasm_bg.wasm', import.meta.url))`, which works
 * once Vite serves the app but not under Node (vitest): Node's fetch can't
 * read `file://` wasm binaries. So under Node we read the bytes ourselves
 * and hand them to `init()`; in the browser we let the default URL-based
 * fetch run, and Vite resolves/bundles that asset.
 */
export function initWasm(): Promise<void> {
  if (!readyPromise) {
    readyPromise = loadWasm();
    // A failed load must stay retryable: drop the cached promise on rejection
    // so a later call attempts a fresh load instead of replaying the error.
    readyPromise.catch(() => {
      readyPromise = null;
    });
  }
  return readyPromise;
}

async function loadWasm(): Promise<void> {
  if (typeof window === "undefined") {
    const { readFile } = await import("node:fs/promises");
    const { fileURLToPath } = await import("node:url");
    const wasmPath = fileURLToPath(
      new URL("../pkg/reasonmetrics_wasm_bg.wasm", import.meta.url),
    );
    const bytes = await readFile(wasmPath);
    await init({ module_or_path: bytes });
  } else {
    await init();
  }
}

/**
 * Analyze a trace with the wasm scoring engine. `initWasm()` must have
 * resolved before calling this.
 */
export function analyzeTrace(trace: TraceInput): AnalysisResult {
  const record = {
    id: trace.id ?? "1",
    problem: trace.problem,
    thinking: trace.thinking,
    answer: trace.answer,
    expected_answer: trace.expected_answer,
  };
  const raw = JSON.parse(wasmAnalyze(JSON.stringify(record))) as RawAnalyzeOutput;

  return {
    composite: raw.scored.quality_score,
    scores: raw.scores,
    annotations: raw.annotations,
    tokenCount: raw.scored.thinking_word_count,
    extractedThinking: raw.extracted_thinking,
    scored: raw.scored,
  };
}

/**
 * Per-family cost presets from the embedded registry, for the cost meter.
 * Only families that ship a `[cost]` table appear (open-weight models omit it
 * because pricing is host-dependent), so the list grows as registry entries
 * gain cost data — no web change needed. `initWasm()` must have resolved.
 * Parsed once and cached.
 */
export function costPresets(): CostPreset[] {
  if (costPresetsCache) return costPresetsCache;
  let entries: RawRegistryEntry[];
  try {
    entries = JSON.parse(wasmRegistryJson()) as RawRegistryEntry[];
  } catch {
    // Presets are an enhancement; if the registry can't be read (e.g. wasm not
    // yet initialized) degrade to none rather than breaking the view. Not
    // cached, so a later call after init still gets the real list.
    return [];
  }
  costPresetsCache = entries
    .filter((e): e is RawRegistryEntry & { cost: NonNullable<RawRegistryEntry["cost"]> } =>
      e.cost != null,
    )
    .map((e) => ({
      id: e.id,
      label: e.display_name,
      outputPerMtok: e.cost.output_per_mtok,
      inputPerMtok: e.cost.input_per_mtok,
      source: e.cost.source,
    }));
  return costPresetsCache;
}
