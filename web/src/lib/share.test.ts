// Round-trip, cap-boundary, and garbage-input coverage for the pure
// share-link/export-filename logic. The round trip also feeds both sides
// through the real wasm `analyzeTrace` to prove the fragment truly carries
// the trace, not just that the two JS objects happen to look alike.

import { beforeAll, describe, expect, it } from "vitest";
import { compressToEncodedURIComponent } from "lz-string";
import { analyzeTrace, initWasm } from "./wasm";
import { decodeShareFragment, encodeShareFragment, exportFilename } from "./share";
import type { TraceInput } from "./types";

beforeAll(async () => {
  await initWasm();
});

function randomAscii(length: number): string {
  const alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let out = "";
  for (let i = 0; i < length; i++) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)];
  }
  return out;
}

describe("encodeShareFragment / decodeShareFragment: round trip", () => {
  it("carries a trace with <think> tags and non-ASCII text through link -> parse -> identical analysis", () => {
    const trace: TraceInput = {
      id: "t-42",
      problem: "计算 2+2 是多少？",
      thinking:
        "<think>让我验证一下：2+2=4。Wait, let me try again — émoji check: 🧠✅.</think>",
      answer: "4，答案正确 ✅",
      expected_answer: "4",
    };

    const encoded = encodeShareFragment(trace);
    if ("tooLarge" in encoded) throw new Error("unexpectedly too large");

    const decoded = decodeShareFragment(`#t=${encoded.fragment}`);
    expect(decoded).toEqual(trace);

    const resultA = analyzeTrace(trace);
    const resultB = analyzeTrace(decoded!);
    expect(resultB).toEqual(resultA);
  });

  it("also accepts a bare 't=...' fragment (no leading #)", () => {
    const trace: TraceInput = { problem: "p", thinking: "t", answer: "a" };
    const encoded = encodeShareFragment(trace);
    if ("tooLarge" in encoded) throw new Error("unexpectedly too large");

    expect(decodeShareFragment(`t=${encoded.fragment}`)).toEqual(trace);
  });
});

describe("encodeShareFragment: cap boundary", () => {
  it("returns tooLarge with a byte count once compressed size exceeds 30 KB (30720 chars)", () => {
    // Random ASCII barely compresses, so a large-enough random payload
    // reliably pushes the compressed form past the cap.
    const trace: TraceInput = {
      problem: "p",
      thinking: randomAscii(28_000),
      answer: "a",
    };

    const encoded = encodeShareFragment(trace);
    expect(encoded).toHaveProperty("tooLarge", true);
    if (!("tooLarge" in encoded)) throw new Error("expected tooLarge");
    expect(encoded.bytes).toBeGreaterThan(30_720);
  });

  it("stays under the cap for an ordinary-sized trace", () => {
    const trace: TraceInput = { problem: "p", thinking: "short thinking", answer: "a" };
    const encoded = encodeShareFragment(trace);
    expect(encoded).not.toHaveProperty("tooLarge");
  });
});

describe("decodeShareFragment: invalid input", () => {
  it("returns null for an empty hash", () => {
    expect(decodeShareFragment("")).toBeNull();
    expect(decodeShareFragment("#")).toBeNull();
  });

  it("returns null for a garbage fragment instead of throwing", () => {
    expect(() => decodeShareFragment("#t=not-valid-lzstring-!!!")).not.toThrow();
    expect(decodeShareFragment("#t=not-valid-lzstring-!!!")).toBeNull();
  });

  it("returns null when the payload decompresses to something that isn't a TraceInput", () => {
    // "5" is valid JSON (a number) but not a TraceInput shape.
    const payload = compressToEncodedURIComponent("5");
    expect(decodeShareFragment(`#t=${payload}`)).toBeNull();
  });

  it("returns null for a hash with no t= assignment", () => {
    expect(decodeShareFragment("#foo=bar")).toBeNull();
  });

  it("returns null for a 't=' with an empty value", () => {
    expect(decodeShareFragment("#t=")).toBeNull();
  });
});

describe("exportFilename", () => {
  it("prefers model over id, rounds composite to 1 decimal", () => {
    expect(exportFilename({ model: "gpt-4", id: "ignored" }, 82.567)).toBe(
      "reasonmetrics-gpt-4-82.6.png",
    );
  });

  it("falls back to id when model is absent", () => {
    expect(exportFilename({ id: "trace-7" }, 50)).toBe("reasonmetrics-trace-7-50.0.png");
  });

  it('falls back to "trace" when neither model nor id is given', () => {
    expect(exportFilename({}, 0)).toBe("reasonmetrics-trace-0.0.png");
  });

  it("lowercases and collapses disallowed characters into single dashes", () => {
    expect(exportFilename({ model: "GPT-4 Turbo/Preview:Beta" }, 99)).toBe(
      "reasonmetrics-gpt-4-turbo-preview-beta-99.0.png",
    );
  });
});
