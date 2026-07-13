// Demo gallery: a strip of one-click example cards shown next to the input
// panel. Index/fixture fetching and the fixture → TraceInput mapping live in
// ../lib/gallery.ts (pure, unit-tested there); this component only renders
// buttons and forwards the mapped record to the same onSelect callback the
// paste/live paths use, so a gallery trace renders through the existing
// analyze pipeline rather than a second one. All fixture-derived text goes
// through JSX text children, never dangerouslySetInnerHTML.

import { useEffect, useState } from "react";
import { fixtureToTraceInput, loadGalleryFixture, loadGalleryIndex, type GalleryEntry } from "../lib/gallery";
import type { TraceInput } from "../lib/types";

interface GalleryStripProps {
  onSelect: (record: TraceInput) => void;
}

export function GalleryStrip({ onSelect }: GalleryStripProps) {
  const [entries, setEntries] = useState<GalleryEntry[] | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadGalleryIndex(import.meta.env.BASE_URL)
      .then((index) => {
        if (!cancelled) setEntries(index);
      })
      .catch((err: unknown) => {
        console.warn("reasonmetrics: gallery index unavailable", err);
        if (!cancelled) setUnavailable(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Index fetch failed (e.g. fixtures missing from the deploy): hide the
  // strip entirely, same as the old module's `root.hidden = true`. The rest
  // of the app is unaffected.
  if (unavailable || !entries) return null;

  function handleCardClick(entry: GalleryEntry): void {
    setError(null);
    loadGalleryFixture(import.meta.env.BASE_URL, entry.file)
      .then((fixture) => onSelect(fixtureToTraceInput(fixture)))
      .catch((err: unknown) => {
        console.warn("reasonmetrics: gallery fixture unavailable", err);
        setError(`Couldn't load example "${entry.label}".`);
      });
  }

  return (
    <div className="gallery-strip">
      <p className="gallery-heading">Or load an example trace:</p>
      <div className="gallery-cards">
        {entries.map((entry) => (
          <button
            type="button"
            className="gallery-card"
            key={entry.id}
            onClick={() => handleCardClick(entry)}
          >
            <span className="gallery-card-label">{entry.label}</span>
            <span className="gallery-card-desc">{entry.description}</span>
          </button>
        ))}
      </div>
      <p className="gallery-error" hidden={!error}>
        {error ?? ""}
      </p>
    </div>
  );
}
