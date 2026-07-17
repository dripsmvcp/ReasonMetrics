# Compare two traces side-by-side — design

**Status:** approved 2026-07-17. Roadmap: R8 (web polish, top of value order — "directly
serves the curation use case"). Zone: 🟡 web components (maintainer-owned).

## Problem

A data curator wants to compare two reasoning traces to decide which to keep — two models'
answers to the same problem, or a known-good vs known-bad trace. Today the app scores one trace
at a time (Paste), or races two *live* models (Live). There is no way to load two *arbitrary*
static traces (gallery / paste / upload) and see them side by side.

## Approach

Compose existing, already-tested primitives. No new parsing, scoring, or input logic.

- `InputPanel` and `GalleryStrip` already forward a `TraceInput` through an `onSelect` callback.
- `analyzeTrace` (`lib/wasm`) is the single analyze path every source converges on.
- `CompareScores` already renders a composite + per-dimension delta table with a `Δ` column and
  renders a missing side as `–` (built for Live's compare mode).
- `AnatomyView` renders one analyzed trace (header, legend, annotated thinking, score card).

## Placement

A third mode tab, `Compare`, alongside Paste/Live. A self-contained `ComparePanel` stays mounted
(`hidden` toggled, never conditionally unmounted) so slot state survives tab switches — matching
App's existing "panels never unmount" rule.

## Components (new)

### `TraceSlot.tsx` — one comparison slot (presentational)
- **Empty:** renders `InputPanel` + `GalleryStrip`, both wired to the slot's `onSelect`.
- **Loaded:** renders a header (slot label `A`/`B` + trace id / truncated problem) + a **Replace**
  button (clears back to Empty) + `AnatomyView` for the slot's result.
- Owns no analyze; ComparePanel passes down `loaded`, `error`, `onSelect`, `onReplace`, `resetToken`.

Props:
```ts
interface TraceSlotProps {
  slotLabel: string;                                   // "A" | "B"
  loaded: { record: TraceInput; result: AnalysisResult } | null;
  error: string | null;
  onSelect: (record: TraceInput) => void;
  onReplace: () => void;
  resetToken: number;                                  // InputPanel mapping-dialog reset
}
```

### `ComparePanel.tsx` — owns the two slots
- State: `slotA`, `slotB` (each `{record, result} | null`), `errA`/`errB`, `genA`/`genB`.
- `analyzeInto(slot, record)`: `try { analyzeTrace(record) } catch` → set that slot's error only.
  A bad trace in one slot never blanks the other (mirrors App's error handling).
- Bumps the slot's own generation on success (drives `AnatomyView key` + `InputPanel resetToken`).
- Renders `CompareScores` (labels `A`/`B`) above the two `TraceSlot` columns, shown once ≥1 side
  is loaded (CompareScores renders the empty side as `–`).

### App change
Add the `Compare` tab button + a `hidden={mode !== "compare"}` container mounting `<ComparePanel />`.
`Mode` type becomes `"paste" | "live" | "compare"`.

### CSS
`.compare-panel`, `.compare-slots` (responsive grid: two columns wide, stacked narrow),
`.compare-slot`, `.compare-slot-header`. Reuse existing `.compare-scores`/`.compare-row`.

## Out of scope (YAGNI — follow-ups)

Compare-share-link (two traces in the URL fragment), PNG export of the pair, token-level text
diff. A trivial **Swap A↔B** button included only if it stays trivial.

## Testing (TDD, mirrors `App.test.tsx` / `LivePanel.test.tsx`, mocks `analyzeTrace`)

- `TraceSlot`: Empty renders input surface; Loaded renders header + Replace + anatomy; Replace fires
  `onReplace`.
- `ComparePanel`: load into A renders A's anatomy; then load B → delta table shows correct `Δ`
  (`B − A`) for composite + a dimension; Replace A returns A to the loader while B stays loaded;
  a per-slot analyze error is isolated and announced (`role="alert"`) without blanking the other.
- `App`: the Compare tab switches mode and mounts `ComparePanel`; slot state survives a tab switch.

## Verify

Run the built app, open Compare, load two gallery traces into A and B, confirm the delta table and
the two side-by-side anatomies render and Replace works.
