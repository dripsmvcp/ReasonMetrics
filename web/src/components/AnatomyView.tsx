// Composes the anatomy view for one analyzed trace: header (tokens / cost /
// dial), legend, annotated thinking text, and the score card.

import { AnatomyHeader } from "./AnatomyHeader";
import { AnnotatedText } from "./AnnotatedText";
import { Legend } from "./Legend";
import { ScoreCard } from "./ScoreCard";
import type { AnalysisResult } from "../lib/types";

export function AnatomyView({ result }: { result: AnalysisResult }) {
  return (
    <div className="anatomy">
      <AnatomyHeader result={result} />
      <Legend />
      <AnnotatedText result={result} />
      <ScoreCard result={result} />
    </div>
  );
}
