// Integration test: raw pasted text carrying a <think> tag should flow
// through detectAndParse -> mapRecord -> the real wasm analyzer and come out
// with the tag stripped. This exercises the actual wasm build, like
// wasm.test.ts does for Task 7.

import { beforeAll, describe, expect, it } from "vitest";
import { mapRecord } from "./aliases";
import { detectAndParse } from "./input";
import { analyzeTrace, initWasm } from "./wasm";

describe("raw-text <think> extraction display path", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("strips <think> tags via the real wasm scorer", () => {
    const pasted = "<think>2+2 is 4. Let me verify: 2+2=4.</think>";

    const detected = detectAndParse(pasted);
    expect(detected.format).toBe("raw");

    const mapped = mapRecord(detected.records[0]);
    expect(mapped.input).toBeDefined();
    expect(mapped.missing).toEqual([]);

    const result = analyzeTrace(mapped.input!);

    expect(result.extractedThinking).not.toContain("<think>");
    expect(result.extractedThinking).not.toContain("</think>");
    expect(result.extractedThinking).toContain("2+2 is 4");
  });

  it("also strips <reasoning> tags", () => {
    const pasted = "<reasoning>Base case first, then induct.</reasoning>";

    const detected = detectAndParse(pasted);
    const mapped = mapRecord(detected.records[0]);
    const result = analyzeTrace(mapped.input!);

    expect(result.extractedThinking).toBe("Base case first, then induct.");
  });
});
