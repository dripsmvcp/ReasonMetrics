// Pure logic for share artifacts: encoding/decoding a trace into the URL
// fragment (never sent to a server — this is a 100% client-side app) and
// building sanitized PNG export filenames. No DOM here; web/src/ui/shareBar.ts
// wires this into the toolbar.

import {
  compressToEncodedURIComponent,
  decompressFromEncodedURIComponent,
} from "lz-string";
import type { TraceInput } from "./types";

/** Compressed fragments above this length (chars, which for
 * compressToEncodedURIComponent's ASCII-only alphabet is also the byte
 * count) are too large to put in a URL — 30 KB. */
const CAP_CHARS = 30_720;

export type EncodeResult = { fragment: string } | { tooLarge: true; bytes: number };

/**
 * Compress `trace` for the URL fragment. Returns the compressed payload
 * (without the `t=` prefix — callers assemble `location.hash` themselves)
 * or `{ tooLarge: true, bytes }` when the compressed form exceeds the 30 KB
 * cap, so callers can fall back to a file download.
 */
export function encodeShareFragment(trace: TraceInput): EncodeResult {
  const compressed = compressToEncodedURIComponent(JSON.stringify(trace));
  if (compressed.length > CAP_CHARS) {
    return { tooLarge: true, bytes: compressed.length };
  }
  return { fragment: compressed };
}

function isTraceInput(value: unknown): value is TraceInput {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  if (typeof v.problem !== "string") return false;
  if (typeof v.thinking !== "string") return false;
  if (typeof v.answer !== "string") return false;
  if (v.id !== undefined && typeof v.id !== "string") return false;
  if (v.expected_answer !== undefined && typeof v.expected_answer !== "string") return false;
  return true;
}

/**
 * Decode a `#t=…` (or bare `t=…`) URL fragment back into a `TraceInput`.
 * Never throws: any garbage input — wrong prefix, corrupt compression,
 * valid JSON of the wrong shape — yields `null` so callers can ignore it
 * silently.
 */
export function decodeShareFragment(hash: string): TraceInput | null {
  const stripped = hash.startsWith("#") ? hash.slice(1) : hash;
  if (!stripped.startsWith("t=")) return null;
  const payload = stripped.slice(2);
  if (payload.length === 0) return null;

  try {
    const json = decompressFromEncodedURIComponent(payload);
    if (!json) return null;
    const parsed: unknown = JSON.parse(json);
    return isTraceInput(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function sanitizeFilenamePart(raw: string): string {
  return raw.toLowerCase().replace(/[^a-z0-9._-]+/g, "-");
}

/**
 * Build the PNG export filename: `reasonmetrics-<model|id|"trace">-<composite
 * to 1 decimal>.png`, sanitized to a safe, lowercase filename.
 */
export function exportFilename(meta: { model?: string; id?: string }, composite: number): string {
  const base = meta.model || meta.id || "trace";
  return sanitizeFilenamePart(`reasonmetrics-${base}-${composite.toFixed(1)}.png`);
}
