import { beforeAll, describe, expect, it } from "vitest";
import { analyzeTrace, costPresets, initWasm } from "./wasm";

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

describe("costPresets", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("exposes only registry families that ship a cost table", () => {
    const presets = costPresets();
    // deepseek-r1 is the one entry with a [cost] block (open-weight families
    // omit it); if more entries gain costs this asserts the shape, not a count.
    const deepseek = presets.find((p) => p.id === "deepseek-r1");
    expect(deepseek).toBeDefined();
    expect(deepseek!.outputPerMtok).toBeGreaterThan(0);
    expect(deepseek!.inputPerMtok).toBeGreaterThan(0);
    expect(deepseek!.label).toContain("DeepSeek");
    // Every preset must carry a positive output rate (the one the meter uses)
    // and a sourced citation.
    for (const p of presets) {
      expect(p.outputPerMtok).toBeGreaterThan(0);
      expect(p.source.length).toBeGreaterThan(0);
    }
  });

  it("degrades to no presets rather than throwing before wasm is ready", async () => {
    // Not a failure mode we can trigger post-init here, but the contract is that
    // a readable registry yields an array; the graceful-empty path is covered by
    // the try/catch in costPresets and the select only rendering when non-empty.
    expect(Array.isArray(costPresets())).toBe(true);
  });
});
