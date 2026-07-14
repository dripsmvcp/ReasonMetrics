// Side-by-side score table for the Live tab's compare mode: the composite
// plus one row per scorer, each with both models' values and the B−A delta.
// Purely presentational — both results come from the same wasm analyzeTrace
// the single-model flow uses; a side still streaming renders as "–".

import type { AnalysisResult } from "../lib/types";
import { scoreClass } from "./scoreClass";

interface CompareScoresProps {
  labelA: string;
  labelB: string;
  a: AnalysisResult | null;
  b: AnalysisResult | null;
}

function deltaCell(a: number | undefined, b: number | undefined) {
  if (a === undefined || b === undefined) {
    return <span className="compare-delta">–</span>;
  }
  const d = b - a;
  const cls = d > 0 ? "compare-delta delta-pos" : d < 0 ? "compare-delta delta-neg" : "compare-delta";
  return <span className={cls}>{`${d > 0 ? "+" : ""}${d.toFixed(1)}`}</span>;
}

function scoreCell(value: number | undefined) {
  if (value === undefined) {
    return <span className="compare-value">–</span>;
  }
  return <span className={`compare-value ${scoreClass(value)}`}>{value.toFixed(1)}</span>;
}

export function CompareScores({ labelA, labelB, a, b }: CompareScoresProps) {
  // Row order follows whichever side exists first; both sides come from the
  // same engine, so the dimension lists match whenever both are present.
  const names = (a ?? b)?.scores.map((entry) => entry.name) ?? [];
  const byName = (result: AnalysisResult | null, name: string) =>
    result?.scores.find((entry) => entry.name === name)?.score;

  return (
    <div className="compare-scores">
      <div className="compare-row compare-header">
        <span className="compare-name" />
        <span className="compare-label" title={labelA}>
          {labelA}
        </span>
        <span className="compare-label" title={labelB}>
          {labelB}
        </span>
        <span className="compare-delta">Δ</span>
      </div>
      <div className="compare-row compare-composite">
        <span className="compare-name">composite</span>
        {scoreCell(a?.composite)}
        {scoreCell(b?.composite)}
        {deltaCell(a?.composite, b?.composite)}
      </div>
      {names.map((name) => (
        <div className="compare-row" key={name}>
          <span className="compare-name">{name}</span>
          {scoreCell(byName(a, name))}
          {scoreCell(byName(b, name))}
          {deltaCell(byName(a, name), byName(b, name))}
        </div>
      ))}
    </div>
  );
}
