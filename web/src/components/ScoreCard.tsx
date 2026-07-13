// The 9-row score card: one row per scorer, in registry order — a bar,
// 1-decimal score, and the first diagnostic as "key: value".

import type { AnalysisResult } from "../lib/types";
import { scoreClass } from "./scoreClass";

export function ScoreCard({ result }: { result: AnalysisResult }) {
  return (
    <div className="score-card">
      {result.scores.map((entry) => {
        const top = entry.diagnostics[0];
        const width = Math.max(0, Math.min(100, entry.score));
        return (
          <div className="score-row" key={entry.name}>
            <span className="score-name">{entry.name}</span>
            <div className="score-bar">
              <div
                className={`score-bar-fill ${scoreClass(entry.score)}`}
                style={{ width: `${width}%` }}
              />
            </div>
            <span className="score-value">{entry.score.toFixed(1)}</span>
            {top && (
              <span className="score-diag" title={`${top[0]}: ${top[1]}`}>
                {`${top[0]}: ${top[1]}`}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}
