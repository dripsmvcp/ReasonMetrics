// One slot of the compare view. EMPTY: the same InputPanel + GalleryStrip the
// single-trace flow uses, so paste / JSONL+mapping / drag-drop / gallery all
// work per slot with no new input logic. LOADED: collapses to a header (slot
// label + trace id/problem) with a Replace button, plus the AnatomyView for the
// slot's result. Purely presentational — ComparePanel owns analyzeTrace, the
// per-slot error, and the reset token; a slot never blanks its sibling.

import { AnatomyView } from "./AnatomyView";
import { GalleryStrip } from "./GalleryStrip";
import { InputPanel } from "./InputPanel";
import type { AnalysisResult, TraceInput } from "../lib/types";

interface TraceSlotProps {
  /** "A" / "B" — shown in the slot header and used as the CompareScores label. */
  slotLabel: string;
  loaded: { record: TraceInput; result: AnalysisResult } | null;
  /** Last analyze failure for this slot; shown above the still-usable loader. */
  error: string | null;
  onSelect: (record: TraceInput) => void;
  /** Clears the loaded trace, returning the slot to its loader. */
  onReplace: () => void;
  /** Bumped when this slot loads a trace: remounts the AnatomyView fresh (like
   * App's per-analysis key) and resets any open InputPanel mapping dialog. */
  resetToken: number;
}

function describeRecord(record: TraceInput): string {
  const problem = record.problem.trim().replace(/\s+/g, " ");
  const snippet = problem.length > 60 ? `${problem.slice(0, 60)}…` : problem;
  return record.id ? `#${record.id} — ${snippet}` : snippet;
}

export function TraceSlot({ slotLabel, loaded, error, onSelect, onReplace, resetToken }: TraceSlotProps) {
  if (loaded) {
    return (
      <div className="compare-slot">
        <div className="compare-slot-header">
          <span className="slot-tag">{slotLabel}</span>
          <span className="slot-desc" title={loaded.record.problem}>
            {describeRecord(loaded.record)}
          </span>
          <button type="button" className="slot-replace" onClick={onReplace}>
            Replace
          </button>
        </div>
        <AnatomyView key={resetToken} result={loaded.result} />
      </div>
    );
  }

  return (
    <div className="compare-slot">
      {error && <p className="slot-error" role="alert">{`analysis failed: ${error}`}</p>}
      <InputPanel onSelect={onSelect} resetToken={resetToken} />
      <GalleryStrip onSelect={onSelect} />
    </div>
  );
}
