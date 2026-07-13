// The annotated thinking text: restart = wavy red underline + ⟲ badge,
// verification = green highlight, repetition = collapsed ×N pill that
// toggles inline expansion. All text reaches the DOM via JSX children —
// never dangerouslySetInnerHTML — so the view is XSS-safe by construction.
// Pure span math (byte→UTF-16 conversion, the flat segment partition,
// repetition grouping) lives in ../lib/spans.ts.

import { useState, type ReactNode } from "react";
import { annotationsToUtf16, buildSegments, groupRepetitions, type Segment } from "../lib/spans";
import type { AnalysisResult } from "../lib/types";

/** Restart-annotation end offsets → the notes of the restart annotations
 * ending there (one ⟲ badge per note). */
type RestartEnds = Map<number, string[]>;

/** Positions of collapsed duplicates: segment start → group info. */
type RepetitionStarts = Map<number, { end: number; note: string; total: number }>;

/** Title text for a segment: the actual notes of the annotations covering
 * it — never a hardcoded per-kind constant. */
type TitleFor = (segment: Segment) => string;

export function AnnotatedText({ result }: { result: AnalysisResult }) {
  const text = result.extractedThinking;
  const annotations = annotationsToUtf16(text, result.annotations);
  const segments = buildSegments(text.length, annotations);

  const restartEnds: RestartEnds = new Map();
  for (const a of annotations) {
    if (a.kind === "restart" && a.start < a.end) {
      const notes = restartEnds.get(a.end) ?? [];
      notes.push(a.note);
      restartEnds.set(a.end, notes);
    }
  }

  // Titles carry the covering annotations' own notes verbatim; distinct
  // notes of overlapping annotations are joined with "; ".
  const titleFor: TitleFor = (segment) => {
    const notes: string[] = [];
    for (const a of annotations) {
      if (
        a.start < a.end &&
        a.start <= segment.start &&
        a.end >= segment.end &&
        a.note.length > 0 &&
        !notes.includes(a.note)
      ) {
        notes.push(a.note);
      }
    }
    return notes.join("; ");
  };

  const repetitionStarts: RepetitionStarts = new Map();
  for (const group of groupRepetitions(text, annotations)) {
    for (const span of group.spans) {
      repetitionStarts.set(span.start, { end: span.end, note: span.note, total: group.total });
    }
  }

  const blocks: ReactNode[] = [];
  let i = 0;
  let blockIndex = 0;
  while (i < segments.length) {
    const segment = segments[i];
    const repetition = repetitionStarts.get(segment.start);
    if (repetition && segment.kinds.includes("repetition")) {
      const inner: Segment[] = [];
      while (i < segments.length && segments[i].end <= repetition.end) {
        inner.push(segments[i]);
        i++;
      }
      blocks.push(
        <RepetitionToggle
          key={`rep-${blockIndex++}`}
          text={text}
          innerSegments={inner}
          repetition={repetition}
          restartEnds={restartEnds}
          titleFor={titleFor}
        />,
      );
    } else {
      blocks.push(...renderSegment(text, segment, restartEnds, titleFor, `seg-${blockIndex++}`));
      i++;
    }
  }

  return <div className="thinking-text">{blocks}</div>;
}

/** Collapsed duplicate: a ×N pill that toggles the full duplicated text
 * (rendered from its segments, so nested restart/verification styling
 * still applies when expanded). */
function RepetitionToggle({
  text,
  innerSegments,
  repetition,
  restartEnds,
  titleFor,
}: {
  text: string;
  innerSegments: Segment[];
  repetition: { end: number; note: string; total: number };
  restartEnds: RestartEnds;
  titleFor: TitleFor;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <span className="rep-group">
      <button
        type="button"
        className="rep-pill"
        title={repetition.note}
        aria-expanded={expanded}
        onClick={() => setExpanded((value) => !value)}
      >
        {`×${repetition.total} ↕`}
      </button>
      <span className="rep-content" hidden={!expanded}>
        {innerSegments.flatMap((segment, idx) =>
          renderSegment(text, segment, restartEnds, titleFor, `rep-inner-${idx}`),
        )}
      </span>
    </span>
  );
}

/** A restart annotation always ends on a segment boundary; its ⟲ badge goes
 * right after the segment that closes it, titled with that annotation's own
 * note. */
function renderSegment(
  text: string,
  segment: Segment,
  restartEnds: RestartEnds,
  titleFor: TitleFor,
  keyPrefix: string,
): ReactNode[] {
  const slice = text.slice(segment.start, segment.end);
  const nodes: ReactNode[] = [];

  if (segment.kinds.length === 0) {
    nodes.push(slice);
  } else {
    nodes.push(
      <span
        key={`${keyPrefix}-text`}
        className={segment.kinds.map((kind) => `ann-${kind}`).join(" ")}
        title={titleFor(segment)}
      >
        {slice}
      </span>,
    );
  }

  if (segment.kinds.includes("restart")) {
    (restartEnds.get(segment.end) ?? []).forEach((note, idx) => {
      nodes.push(
        <span key={`${keyPrefix}-badge-${idx}`} className="restart-badge" title={note}>
          ⟲
        </span>,
      );
    });
  }

  return nodes;
}
