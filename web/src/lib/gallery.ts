// Pure gallery loader: no DOM. Knows the fixture JSON shape stored under
// web/public/gallery/, how to resolve those files against Vite's base path
// (import.meta.env.BASE_URL differs between dev "/" and GitHub Pages
// "/reasonmetrics/"), and how to map a fixture down to the TraceInput the
// existing analyze path accepts. web/src/ui/galleryStrip.ts wires this into
// the DOM; this file only knows fetch/JSON.

import type { TraceInput } from "./types";

/** One row of gallery/index.json. */
export interface GalleryEntry {
  id: string;
  label: string;
  description: string;
  file: string;
}

/** One pre-baked trace fixture in web/public/gallery/. Everything beyond
 * the TraceInput fields is provenance metadata: which model/prompt produced
 * it, and whether it was trimmed after generation (`curated`). */
export interface GalleryFixture {
  id: string;
  label: string;
  description: string;
  model: string;
  problem: string;
  thinking: string;
  answer: string;
  generated_with: { prompt: string; options: Record<string, unknown> };
  curated: boolean;
  curation_note?: string;
}

/** Resolve a gallery file against the Vite base path. */
export function galleryUrl(base: string, file: string): string {
  return `${base.replace(/\/+$/, "")}/gallery/${file}`;
}

/** Strip gallery metadata down to the shape `analyzeTrace` accepts. */
export function fixtureToTraceInput(fixture: GalleryFixture): TraceInput {
  return {
    id: fixture.id,
    problem: fixture.problem,
    thinking: fixture.thinking,
    answer: fixture.answer,
  };
}

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`GET ${url} failed with status ${res.status}`);
  }
  return (await res.json()) as T;
}

/** Load gallery/index.json (same-origin) from under `base`. */
export function loadGalleryIndex(base: string): Promise<GalleryEntry[]> {
  return fetchJson<GalleryEntry[]>(galleryUrl(base, "index.json"));
}

/** Load one fixture file (same-origin) from under `base`. */
export function loadGalleryFixture(base: string, file: string): Promise<GalleryFixture> {
  return fetchJson<GalleryFixture>(galleryUrl(base, file));
}
