// Restores the detail view from a `#t=...` share-link fragment on load.
// Decoding is delegated to ../lib/share.ts (pure, unit-tested there); this hook
// only reads location.hash once on mount and drives the same render
// callback the input/live/gallery paths use, so a restored trace renders
// through the existing pipeline rather than a second one.

import { useEffect } from "react";
import { decodeShareFragment } from "../lib/share";
import type { TraceInput } from "../lib/types";

/**
 * On mount, if `location.hash` decodes to a trace, call `renderAnalysis`
 * with it exactly as a pasted or live record would be. Any invalid/corrupt
 * fragment is ignored silently — a warning is logged, but the caller sees
 * no error and the normal input screen stays in place.
 *
 * `renderAnalysis` must be the render-only path (it must NOT clear
 * `location.hash`) — a freshly-loaded share link should keep its hash and
 * stay shareable. Callers pass the same stable callback every render (e.g.
 * one created with `useCallback(..., [])`); this hook only ever runs its
 * effect once, on mount.
 */
export function useHashRestore(renderAnalysis: (record: TraceInput) => void): void {
  useEffect(() => {
    const trace = decodeShareFragment(location.hash);
    if (!trace) {
      if (location.hash.length > 0) {
        console.warn("reasonmetrics: ignoring invalid share-link fragment");
      }
      return;
    }
    renderAnalysis(trace);
    // Intentionally mount-only: restoring from the URL is a one-time,
    // load-time concern, not something that should re-run if the parent
    // re-creates its callback.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
