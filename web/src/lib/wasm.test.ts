import { beforeAll, describe, expect, it } from "vitest";
import { analyzeTrace, initWasm } from "./wasm";

describe("analyzeTrace", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("scores a minimal trace between 0 and 100", () => {
    const result = analyzeTrace({
      problem: "2+2?",
      thinking: "4",
      answer: "4",
    });

    expect(result.composite).toBeGreaterThanOrEqual(0);
    expect(result.composite).toBeLessThanOrEqual(100);
  });
});
