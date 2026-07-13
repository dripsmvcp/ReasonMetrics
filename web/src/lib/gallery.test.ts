// Pure-logic tests for the gallery loader: URL joining against a Vite base
// path, fixture → TraceInput mapping (metadata stripped), and the fetch
// wrappers for index.json and per-case fixture files. No DOM here and no
// real network — fetch is stubbed per test, matching ollama.test.ts.

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fixtureToTraceInput,
  galleryUrl,
  loadGalleryFixture,
  loadGalleryIndex,
  type GalleryFixture,
} from "./gallery";

const FIXTURE: GalleryFixture = {
  id: "r1-rambling",
  label: "R1 rambling",
  description: "deepseek-r1:1.5b overthinking an easy percentage",
  model: "deepseek-r1:1.5b",
  problem: "What is 15% of 80?",
  thinking: "Okay, so 15% of 80. Wait, let me think again...",
  answer: "12",
  generated_with: { prompt: "What is 15% of 80?", options: { temperature: 0.7 } },
  curated: false,
};

function stubFetch(response: Response): ReturnType<typeof vi.fn> {
  const fn = vi.fn().mockResolvedValue(response);
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("galleryUrl", () => {
  it("joins a file onto the root base", () => {
    expect(galleryUrl("/", "index.json")).toBe("/gallery/index.json");
  });

  it("joins a file onto a project-pages base like /reasonmetrics/", () => {
    expect(galleryUrl("/reasonmetrics/", "r1-rambling.json")).toBe(
      "/reasonmetrics/gallery/r1-rambling.json",
    );
  });

  it("tolerates a base without a trailing slash", () => {
    expect(galleryUrl("/reasonmetrics", "index.json")).toBe("/reasonmetrics/gallery/index.json");
  });
});

describe("fixtureToTraceInput", () => {
  it("keeps id, problem, thinking, answer and drops gallery metadata", () => {
    expect(fixtureToTraceInput(FIXTURE)).toEqual({
      id: "r1-rambling",
      problem: "What is 15% of 80?",
      thinking: "Okay, so 15% of 80. Wait, let me think again...",
      answer: "12",
    });
  });
});

describe("loadGalleryIndex", () => {
  it("fetches gallery/index.json under the base and returns the entries", async () => {
    const entries = [
      { id: "a", label: "A", description: "da", file: "a.json" },
      { id: "b", label: "B", description: "db", file: "b.json" },
    ];
    const fetchMock = stubFetch(new Response(JSON.stringify(entries), { status: 200 }));

    const result = await loadGalleryIndex("/reasonmetrics/");

    expect(fetchMock).toHaveBeenCalledWith("/reasonmetrics/gallery/index.json");
    expect(result).toEqual(entries);
  });

  it("throws on a non-2xx response", async () => {
    stubFetch(new Response("nope", { status: 404 }));

    await expect(loadGalleryIndex("/")).rejects.toThrow(/404/);
  });
});

describe("loadGalleryFixture", () => {
  it("fetches the fixture file under the base and returns the parsed fixture", async () => {
    const fetchMock = stubFetch(new Response(JSON.stringify(FIXTURE), { status: 200 }));

    const result = await loadGalleryFixture("/", "r1-rambling.json");

    expect(fetchMock).toHaveBeenCalledWith("/gallery/r1-rambling.json");
    expect(result).toEqual(FIXTURE);
  });

  it("throws on a non-2xx response", async () => {
    stubFetch(new Response("nope", { status: 500 }));

    await expect(loadGalleryFixture("/", "missing.json")).rejects.toThrow(/500/);
  });
});
