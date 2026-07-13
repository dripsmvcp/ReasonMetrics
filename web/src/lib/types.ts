// TS mirrors of the JSON shapes produced by the reasonmetrics-wasm `analyze()`
// binding. Field names on the Rust-facing types (ScoredTrace, Annotation)
// match the wasm crate's serde output exactly; camelCase fields are added
// on the client-facing AnalysisResult by the wasm.ts bridge.

/** Input accepted by `analyzeTrace`. Mirrors the core `TraceRecord` fields
 * the wasm crate actually reads; `id` defaults to `"1"` when omitted. */
export interface TraceInput {
  id?: string;
  problem: string;
  thinking: string;
  answer: string;
  expected_answer?: string;
}

/** One row of the per-scorer breakdown, in registry order. */
export interface ScoreEntry {
  name: string;
  score: number;
  weight: number;
  diagnostics: [string, string][];
}

export type AnnotationKind = "restart" | "verification" | "repetition";

/** A span-level annotation. `start`/`end` are byte offsets into
 * `AnalysisResult.extractedThinking`. */
export interface Annotation {
  start: number;
  end: number;
  kind: AnnotationKind;
  note: string;
}

/** Mirrors the core `ScoredTrace` struct (serde field names, snake_case). */
export interface ScoredTrace {
  id: string;
  problem: string;
  thinking: string;
  answer: string;
  quality_score: number;
  efficiency_score: number;
  language_score: number;
  answer_alignment_score: number;
  structural_score: number;
  repetition_score: number;
  overthinking_score: number;
  verification_score: number;
  length_score: number;
  thinking_word_count: number;
  restart_count: number;
  detected_language: string;
  has_self_verification: boolean;
  is_language_mixed: boolean;
  answer_in_trace_end: boolean;
}

/** Client-facing result of `analyzeTrace`. */
export interface AnalysisResult {
  composite: number;
  scores: ScoreEntry[];
  annotations: Annotation[];
  tokenCount: number;
  /** Thinking text after tag-extraction; annotation offsets index into this. */
  extractedThinking: string;
  /** Raw scored trace, kept around for the anatomy view (later task). */
  scored: ScoredTrace;
}
