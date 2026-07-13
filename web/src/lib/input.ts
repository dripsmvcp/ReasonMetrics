// Pure parsing/format-detection for pasted or dropped trace text. No DOM
// access here; `ui/inputPanel.ts` wires this to the paste box and drop zone.

import type { LooseTraceInput } from "./aliases";

/** Max records surfaced from a JSONL batch; the rest are dropped with a
 * "showing first 1,000 of N" note in the UI. */
export const JSONL_RECORD_CAP = 1000;

export type DetectedFormat = "json" | "jsonl" | "raw";

export interface DetectResult {
  format: DetectedFormat;
  /** Parsed records, before alias mapping. Capped at JSONL_RECORD_CAP for jsonl. */
  records: LooseTraceInput[];
  /** Total records found before capping (equals records.length unless capped). */
  totalCount: number;
  /** True when totalCount exceeds JSONL_RECORD_CAP and records were dropped. */
  capped: boolean;
}

function tryParseJsonObject(text: string): LooseTraceInput | null {
  try {
    const value: unknown = JSON.parse(text);
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      return value as LooseTraceInput;
    }
  } catch {
    // not JSON; caller falls through to the next format
  }
  return null;
}

/**
 * Auto-detect the pasted/dropped text's format and parse it into raw
 * records (before alias mapping applies):
 *   1. the whole text parses as one JSON object -> a single "json" record.
 *   2. every non-empty line parses as a JSON object -> "jsonl" records,
 *      capped at JSONL_RECORD_CAP.
 *   3. otherwise -> a single "raw" record with the text as `thinking`.
 *      No literal `<think>` tag is required here: core's `extract_thinking`
 *      handles plain text too, and pulls tag content out when present.
 */
export function detectAndParse(text: string): DetectResult {
  const trimmed = text.trim();

  const whole = tryParseJsonObject(trimmed);
  if (whole) {
    return { format: "json", records: [whole], totalCount: 1, capped: false };
  }

  const lines = trimmed.split(/\r?\n/).filter((line) => line.trim().length > 0);
  if (lines.length > 1) {
    const parsedLines = lines.map(tryParseJsonObject);
    if (parsedLines.every((record): record is LooseTraceInput => record !== null)) {
      const totalCount = parsedLines.length;
      const records = parsedLines.slice(0, JSONL_RECORD_CAP);
      return { format: "jsonl", records, totalCount, capped: totalCount > records.length };
    }
  }

  return {
    format: "raw",
    records: [{ problem: "", thinking: text, answer: "" }],
    totalCount: 1,
    capped: false,
  };
}
