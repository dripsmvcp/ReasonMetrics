// @vitest-environment happy-dom

// Tests for the compare-mode score table: row layout, delta sign/formatting,
// and the placeholder rendering while one side is still streaming.

import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AnalysisResult, ScoredTrace } from "../lib/types";
import { CompareScores } from "./CompareScores";

function makeResult(composite: number, dims: [string, number][]): AnalysisResult {
  return {
    composite,
    scores: dims.map(([name, score]) => ({ name, score, weight: 0.1, diagnostics: [] })),
    annotations: [],
    tokenCount: 42,
    extractedThinking: "…",
    scored: { quality_score: composite } as ScoredTrace,
  };
}

describe("CompareScores", () => {
  it("renders composite plus one row per dimension with signed deltas", () => {
    const a = makeResult(80, [
      ["structure", 90],
      ["repetition", 70],
    ]);
    const b = makeResult(60.5, [
      ["structure", 90],
      ["repetition", 40],
    ]);
    const { container } = render(<CompareScores labelA="m-a" labelB="m-b" a={a} b={b} />);

    const rows = container.querySelectorAll(".compare-row");
    // header + composite + 2 dimensions
    expect(rows).toHaveLength(4);

    const composite = container.querySelector(".compare-composite")!;
    expect(composite.textContent).toContain("80.0");
    expect(composite.textContent).toContain("60.5");
    expect(composite.textContent).toContain("-19.5");

    const deltas = [...container.querySelectorAll(".compare-row:not(.compare-header) .compare-delta")];
    expect(deltas.map((el) => el.textContent)).toEqual(["-19.5", "0.0", "-30.0"]);
    expect(deltas[2].className).toContain("delta-neg");
    expect(deltas[1].className).not.toContain("delta-neg");
    expect(deltas[1].className).not.toContain("delta-pos");
  });

  it("marks positive deltas with a plus sign and delta-pos", () => {
    const a = makeResult(50, [["structure", 40]]);
    const b = makeResult(75, [["structure", 90]]);
    const { container } = render(<CompareScores labelA="a" labelB="b" a={a} b={b} />);

    const structureDelta = [...container.querySelectorAll(".compare-row")].at(-1)!
      .querySelector(".compare-delta")!;
    expect(structureDelta.textContent).toBe("+50.0");
    expect(structureDelta.className).toContain("delta-pos");
  });

  it("renders placeholders while one side has no result yet", () => {
    const a = makeResult(80, [["structure", 90]]);
    const { container } = render(<CompareScores labelA="m-a" labelB="m-b" a={a} b={null} />);

    // Rows still come from side A; B's cells and deltas are placeholders.
    const structureRow = [...container.querySelectorAll(".compare-row")].at(-1)!;
    const values = structureRow.querySelectorAll(".compare-value");
    expect(values[0].textContent).toBe("90.0");
    expect(values[1].textContent).toBe("–");
    expect(structureRow.querySelector(".compare-delta")!.textContent).toBe("–");
  });

  it("applies the shared red/amber/green thresholds to value cells", () => {
    const a = makeResult(49, [["structure", 74]]);
    const b = makeResult(75, [["structure", 75]]);
    const { container } = render(<CompareScores labelA="a" labelB="b" a={a} b={b} />);

    const compositeValues = container.querySelector(".compare-composite")!
      .querySelectorAll(".compare-value");
    expect(compositeValues[0].className).toContain("score-red");
    expect(compositeValues[1].className).toContain("score-green");

    const structureValues = [...container.querySelectorAll(".compare-row")].at(-1)!
      .querySelectorAll(".compare-value");
    expect(structureValues[0].className).toContain("score-amber");
  });
});
