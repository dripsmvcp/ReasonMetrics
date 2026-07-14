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

      <div className="score-block">
        <Dial score={result.composite} />
        <p className="score-meaning">
          better than <strong>{result.composite.toFixed(0)}%</strong> of real
          reasoning traces
        </p>
      </div>
    </header>
  );
}

// The score is a PERCENTILE against a reference corpus of real reasoning traces,
// not an absolute grade — so "12" means "12% of real traces are worse than this
// one", and a trivially short trace landing near zero is correct, not a bug.
// Rendering a bare "12 / 100" invited exactly that misreading, which is why the
// dial is captioned. See docs/CALIBRATION.md and issue #30.
function Dial({ score }: { score: number }) {
  const clamped = Math.max(0, Math.min(100, score));
  const dashArray = `${(clamped / 100) * DIAL_CIRCUMFERENCE} ${DIAL_CIRCUMFERENCE}`;

  return (
    <svg
      className="dial"
      viewBox="0 0 100 100"
      role="img"
      aria-label={`quality score ${score.toFixed(1)}: better than ${score.toFixed(0)}% of real reasoning traces`}
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
