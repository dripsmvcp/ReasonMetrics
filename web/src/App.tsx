// App shell: mode tabs (paste/live), the single analyze pipeline every
// input source converges on, and hash-restore on load. Mirrors the old
// main.ts wiring, now as React state instead of DOM containers whose
// `hidden` attribute main.ts toggled by hand — the four panels below stay
// mounted the whole session (never conditionally unmounted by mode), so
// e.g. Live settings/prompt state and the input panel's paste text survive
// switching tabs, exactly like before.

import { useCallback, useRef, useState } from "react";
import { AnatomyView } from "./components/AnatomyView";
import { GalleryStrip } from "./components/GalleryStrip";
import { InputPanel } from "./components/InputPanel";
import { LivePanel } from "./components/LivePanel";
import { ShareBar } from "./components/ShareBar";
import { useHashRestore } from "./hooks/useHashRestore";
import { analyzeTrace } from "./lib/wasm";
import type { AnalysisResult, TraceInput } from "./lib/types";

type Mode = "paste" | "live";

export default function App() {
  const [mode, setMode] = useState<Mode>("paste");
  const [record, setRecord] = useState<TraceInput | null>(null);
  const [result, setResult] = useState<AnalysisResult | null>(null);
  // Bumped on every successful analysis from ANY source (paste, JSONL row,
  // mapping-dialog apply, gallery card, live stream tick, hash restore).
  // Two things key off it: the anatomy view remounts fresh each time
  // (matching the old renderAnatomy's full DOM rebuild — default cost rate,
  // all repetition toggles collapsed), and the input panel closes any
  // mapping dialog left open by an earlier, now-superseded paste.
  const [generation, setGeneration] = useState(0);
  // Set when analyzeTrace throws (a wasm/parse failure); cleared on the
  // next successful analysis. The previous record/result are left in place
  // so a bad trace never blanks an already-rendered analysis.
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const detailRef = useRef<HTMLDivElement>(null);

  const renderAnalysis = useCallback((rec: TraceInput) => {
    try {
      const analyzed = analyzeTrace(rec);
      setRecord(rec);
      setResult(analyzed);
      setAnalysisError(null);
      setGeneration((n) => n + 1);
    } catch (err) {
      setAnalysisError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // Any freshly-analyzed trace (paste/live/gallery) invalidates a leftover
  // `#t=...` share-link fragment from an earlier restore or copy — without
  // this, the address bar keeps pointing at the old trace after a new one
  // renders. `useHashRestore` below calls `renderAnalysis` directly instead
  // of `onAnalyze`, so a freshly-loaded share link keeps its hash and stays
  // shareable; "copy share link" sets the hash afterward as it always has.
  const onAnalyze = useCallback(
    (rec: TraceInput) => {
      renderAnalysis(rec);
      if (location.hash.startsWith("#t=")) {
        history.replaceState(null, "", location.pathname + location.search);
      }
    },
    [renderAnalysis],
  );

  useHashRestore(renderAnalysis);

  return (
    <>
      <div className="mode-tabs">
        <button
          type="button"
          className={mode === "paste" ? "mode-tab active" : "mode-tab"}
          onClick={() => setMode("paste")}
        >
          Paste
        </button>
        <button
          type="button"
          className={mode === "live" ? "mode-tab active" : "mode-tab"}
          onClick={() => setMode("live")}
        >
          Live
        </button>
      </div>

      <div id="input-panel" hidden={mode !== "paste"}>
        <InputPanel onSelect={onAnalyze} resetToken={generation} />
      </div>
      <div id="gallery-strip-container" hidden={mode !== "paste"}>
        <GalleryStrip onSelect={onAnalyze} />
      </div>
      <div id="live-panel-container" hidden={mode !== "live"}>
        <LivePanel onAnalyze={onAnalyze} active={mode === "live"} />
      </div>
      {analysisError && <p className="analysis-error">{`analysis failed: ${analysisError}`}</p>}
      <div id="share-bar-container" hidden={!result}>
        {record && result && (
          <ShareBar trace={record} result={result} captureTargetRef={detailRef} />
        )}
      </div>
      <div id="detail" ref={detailRef}>
        {result && <AnatomyView key={generation} result={result} />}
      </div>
    </>
  );
}
