// Anatomy header: token count, cost at a configurable rate, and the
// composite-score dial.

import { useState } from "react";
import type { AnalysisResult } from "../lib/types";
import { scoreClass } from "./scoreClass";

const DEFAULT_RATE_PER_MILLION = 3;
const DIAL_RADIUS = 42;
const DIAL_CIRCUMFERENCE = 2 * Math.PI * DIAL_RADIUS;

export function AnatomyHeader({ result }: { result: AnalysisResult }) {
  const [rate, setRate] = useState(DEFAULT_RATE_PER_MILLION);
  const cost = (result.tokenCount / 1_000_000) * rate;

  return (
    <header className="anatomy-header">
      <div className="token-count">{result.tokenCount} tokens</div>

      <div className="cost-block">
        <label className="rate-label">
          $ / 1M tokens{" "}
          <input
            type="number"
            className="rate-input"
            min={0}
            step={0.5}
            value={rate}
            onChange={(event) => setRate(Number(event.target.value) || 0)}
          />
        </label>
        <div className="cost-value">${cost.toFixed(4)}</div>
      </div>

      <Dial score={result.composite} />
    </header>
  );
}

function Dial({ score }: { score: number }) {
  const clamped = Math.max(0, Math.min(100, score));
  const dashArray = `${(clamped / 100) * DIAL_CIRCUMFERENCE} ${DIAL_CIRCUMFERENCE}`;

  return (
    <svg
      className="dial"
      viewBox="0 0 100 100"
      role="img"
      aria-label={`composite score ${score.toFixed(1)} of 100`}
    >
      <circle className="dial-track" cx="50" cy="50" r={DIAL_RADIUS} />
      <circle
        className={`dial-arc ${scoreClass(score)}`}
        cx="50"
        cy="50"
        r={DIAL_RADIUS}
        strokeDasharray={dashArray}
        transform="rotate(-90 50 50)"
      />
      <text className="dial-value" x="50" y="50" textAnchor="middle" dy="0.35em">
        {score.toFixed(1)}
      </text>
    </svg>
  );
}
