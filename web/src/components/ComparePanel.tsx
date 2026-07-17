// The compare view: two TraceSlots side by side, with the reused CompareScores
// delta table above them. Each slot analyzes through the same analyzeTrace the
// single-trace flow uses; a per-slot try/catch keeps one bad trace from blanking
// the other (and, like App, a failed re-analyze leaves the slot's prior trace on
// screen). This panel stays mounted while the Compare tab is hidden, so slot
// state survives switching tabs — matching App's other panels.

import { useCallback, useState } from "react";
import { CompareScores } from "./CompareScores";
import { TraceSlot } from "./TraceSlot";
import { analyzeTrace } from "../lib/wasm";
import type { AnalysisResult, TraceInput } from "../lib/types";

interface SlotState {
  loaded: { record: TraceInput; result: AnalysisResult } | null;
  error: string | null;
  /** Bumped on every successful load — drives the slot's AnatomyView remount
   * and InputPanel mapping-dialog reset (its resetToken). */
  gen: number;
}

const EMPTY: SlotState = { loaded: null, error: null, gen: 0 };

function useSlot() {
  const [state, setState] = useState<SlotState>(EMPTY);

  const onSelect = useCallback((record: TraceInput) => {
    try {
      const result = analyzeTrace(record);
      setState((prev) => ({ loaded: { record, result }, error: null, gen: prev.gen + 1 }));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      // Keep any previously loaded trace; only surface the error.
      setState((prev) => ({ ...prev, error: message }));
    }
  }, []);

  const onReplace = useCallback(() => {
    setState((prev) => ({ loaded: null, error: null, gen: prev.gen }));
  }, []);

  return { state, onSelect, onReplace };
}

export function ComparePanel() {
  const a = useSlot();
  const b = useSlot();

  const bothEmpty = !a.state.loaded && !b.state.loaded;

  return (
    <div className="compare-panel">
      {!bothEmpty && (
        <CompareScores
          labelA="A"
          labelB="B"
          a={a.state.loaded?.result ?? null}
          b={b.state.loaded?.result ?? null}
        />
      )}
      <div className="compare-slots">
        <TraceSlot
          slotLabel="A"
          loaded={a.state.loaded}
          error={a.state.error}
          onSelect={a.onSelect}
          onReplace={a.onReplace}
          resetToken={a.state.gen}
        />
        <TraceSlot
          slotLabel="B"
          loaded={b.state.loaded}
          error={b.state.error}
          onSelect={b.onSelect}
          onReplace={b.onReplace}
          resetToken={b.state.gen}
        />
      </div>
    </div>
  );
}
