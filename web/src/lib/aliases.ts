// Alias table for mapping loosely-shaped trace records (arbitrary JSON key
// names) onto the canonical TraceInput fields the wasm scorer expects.
//
// Single source of truth: crates/reasonmetrics-core/src/trace.rs (the
// `TraceRecord` struct's serde `alias(...)` attributes). This table is a
// hand-maintained TS-side mirror of that list — keep them in sync manually,
// there is no shared codegen between the two.

import type { TraceInput } from "./types";

export type CanonicalField = "id" | "problem" | "thinking" | "answer" | "expected_answer";

const CANONICAL_FIELDS: CanonicalField[] = [
  "id",
  "problem",
  "thinking",
  "answer",
  "expected_answer",
];

const ALIASES: Record<CanonicalField, string[]> = {
  id: ["idx", "index", "uuid"],
  problem: ["question", "prompt", "query", "input"],
  thinking: ["reasoning", "chain_of_thought", "cot", "thought"],
  answer: ["solution", "response", "output", "result"],
  expected_answer: ["ground_truth", "label", "target"],
};

/** A parsed JSON record before field-name resolution: arbitrary keys, unknown shape. */
export type LooseTraceInput = Record<string, unknown>;

/** How a mapping (auto-suggested or dialog-chosen) assigns one raw key. */
export type FieldAssignment = CanonicalField | "ignore";

export interface MapResult {
  /** Present once `thinking` resolves; the record is ready to analyze. */
  input?: TraceInput;
  /** Keys in the record that matched no canonical name or alias. */
  unknownKeys: string[];
  /** Non-empty (["thinking"]) when the record has no resolvable thinking field. */
  missing: "thinking"[];
}

function fieldForKey(key: string): CanonicalField | undefined {
  for (const field of CANONICAL_FIELDS) {
    if (field === key || ALIASES[field].includes(key)) return field;
  }
  return undefined;
}

function resolve(obj: LooseTraceInput, field: CanonicalField): unknown {
  if (obj[field] !== undefined) return obj[field];
  for (const alias of ALIASES[field]) {
    if (obj[alias] !== undefined) return obj[alias];
  }
  return undefined;
}

/**
 * Map a loosely-typed parsed record onto a `TraceInput`, resolving canonical
 * field names first and then the alias table. `thinking` is the only field
 * that must resolve; a missing `problem`/`answer` defaults to `""` and a
 * missing `id` defaults to `fallbackId` (typically the record's 1-based
 * position within its batch).
 */
export function mapRecord(obj: LooseTraceInput, fallbackId = "1"): MapResult {
  const recognized = new Set<string>();
  for (const field of CANONICAL_FIELDS) {
    if (field in obj) recognized.add(field);
    for (const alias of ALIASES[field]) {
      if (alias in obj) recognized.add(alias);
    }
  }
  const unknownKeys = Object.keys(obj).filter((key) => !recognized.has(key));

  const thinking = resolve(obj, "thinking");
  if (thinking === undefined) {
    return { unknownKeys, missing: ["thinking"] };
  }

  const id = resolve(obj, "id");
  const problem = resolve(obj, "problem");
  const answer = resolve(obj, "answer");
  const expected = resolve(obj, "expected_answer");

  const input: TraceInput = {
    id: id === undefined ? fallbackId : String(id),
    problem: problem === undefined ? "" : String(problem),
    thinking: String(thinking),
    answer: answer === undefined ? "" : String(answer),
  };
  if (expected !== undefined) input.expected_answer = String(expected);

  return { input, unknownKeys, missing: [] };
}

/**
 * Pre-fill a mapping dialog: for each key in `obj`, suggest the canonical
 * field it resolves to via the alias table, or "ignore" if none match.
 */
export function suggestMapping(obj: LooseTraceInput): Record<string, FieldAssignment> {
  const mapping: Record<string, FieldAssignment> = {};
  for (const key of Object.keys(obj)) {
    mapping[key] = fieldForKey(key) ?? "ignore";
  }
  return mapping;
}

/**
 * `thinking` is the only hard-required field after mapping, so a dialog
 * mapping is only valid once some key is assigned to it. The Apply path
 * must check this before calling `applyMapping`, which otherwise defaults
 * an unassigned `thinking` to `""` like the optional fields.
 */
export function hasThinkingAssignment(mapping: Record<string, FieldAssignment>): boolean {
  return Object.values(mapping).includes("thinking");
}

/**
 * Apply a user-chosen (or suggested) key -> field mapping to a record,
 * bypassing alias auto-detection entirely. Used once the mapping dialog
 * resolves an otherwise-unmappable schema; the same mapping is then applied
 * uniformly to every record in the batch. Callers must validate the mapping
 * with `hasThinkingAssignment` first.
 */
export function applyMapping(
  obj: LooseTraceInput,
  mapping: Record<string, FieldAssignment>,
  fallbackId = "1",
): TraceInput {
  const values: Partial<Record<CanonicalField, unknown>> = {};
  for (const [key, field] of Object.entries(mapping)) {
    if (field === "ignore" || !(key in obj)) continue;
    values[field] = obj[key];
  }

  const input: TraceInput = {
    id: values.id === undefined ? fallbackId : String(values.id),
    problem: values.problem === undefined ? "" : String(values.problem),
    thinking: values.thinking === undefined ? "" : String(values.thinking),
    answer: values.answer === undefined ? "" : String(values.answer),
  };
  if (values.expected_answer !== undefined) {
    input.expected_answer = String(values.expected_answer);
  }
  return input;
}
