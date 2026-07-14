// @vitest-environment node

// Smoke test for the REAL gallery fixtures shipped in web/public/gallery/.
// Everything else exercises the gallery through mocked fetches, so a renamed
// field, a malformed fixture, or index/file drift would otherwise ship
// silently and only break in a user's browser. This reads the files off disk
// and checks them against the types in ./gallery.

import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { fixtureToTraceInput, type GalleryEntry, type GalleryFixture } from "./gallery";

const GALLERY_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "public", "gallery");

async function readJson<T>(file: string): Promise<T> {
  return JSON.parse(await readFile(join(GALLERY_DIR, file), "utf8")) as T;
}

const index = await readJson<GalleryEntry[]>("index.json");

describe("gallery index.json", () => {
  it("is a non-empty array of fully-populated entries with unique ids", () => {
    expect(Array.isArray(index)).toBe(true);
    expect(index.length).toBeGreaterThan(0);

    for (const entry of index) {
      for (const field of ["id", "label", "description", "file"] as const) {
        expect(typeof entry[field], `${entry.id ?? "?"}.${field}`).toBe("string");
        expect(entry[field].length, `${entry.id ?? "?"}.${field}`).toBeGreaterThan(0);
      }
    }

    expect(new Set(index.map((entry) => entry.id)).size).toBe(index.length);
  });

  it("references only files that exist, and every fixture on disk is listed", async () => {
    const onDisk = (await readdir(GALLERY_DIR))
      .filter((name) => name.endsWith(".json") && name !== "index.json")
      .sort();
    const listed = index.map((entry) => entry.file).sort();

    // Both directions: a missing file 404s the card; an unlisted file is a
    // fixture nobody can reach.
    expect(listed).toEqual(onDisk);
  });
});

describe.each(index.map((entry) => [entry.id, entry] as const))(
  "gallery fixture %s",
  (id, entry) => {
    it("parses into the GalleryFixture shape the loader expects", async () => {
      const fixture = await readJson<GalleryFixture>(entry.file);

      for (const field of ["id", "label", "description", "model", "problem", "thinking"] as const) {
        expect(typeof fixture[field], `${id}.${field}`).toBe("string");
        expect(fixture[field].length, `${id}.${field}`).toBeGreaterThan(0);
      }

      // `answer` must be present but may legitimately be empty: r1-rambling
      // exists precisely because the model never reaches an answer.
      expect(typeof fixture.answer, `${id}.answer`).toBe("string");

      // The id in the file must match the one the index advertises, or the
      // card and the analyzed trace disagree about what is on screen.
      expect(fixture.id).toBe(entry.id);

      expect(typeof fixture.generated_with?.prompt).toBe("string");
      expect(typeof fixture.generated_with?.options).toBe("object");

      if (fixture.curated !== undefined) {
        expect(typeof fixture.curated, `${id}.curated`).toBe("boolean");
      }
      if (fixture.curation_note !== undefined) {
        expect(typeof fixture.curation_note, `${id}.curation_note`).toBe("string");
      }
    });

    it("reduces to a TraceInput the analyzer can accept", async () => {
      const trace = fixtureToTraceInput(await readJson<GalleryFixture>(entry.file));

      expect(trace.id).toBe(entry.id);
      expect(typeof trace.problem).toBe("string");
      expect(typeof trace.answer).toBe("string");
      // Thinking is the one field the scorers cannot do without.
      expect(trace.thinking.length).toBeGreaterThan(0);
    });
  },
);
