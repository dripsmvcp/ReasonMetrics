// Trace-input panel: paste box, drag-drop zone, the field-mapping dialog,
// and the JSONL record list. Parsing/detection lives in ../lib/input.ts and
// ../lib/aliases.ts (pure, unit-tested there); this component only wires the
// DOM/React state around them.

import { useEffect, useRef, useState } from "react";
import {
  applyMapping,
  mapRecord,
  suggestMapping,
  type FieldAssignment,
  type LooseTraceInput,
} from "../lib/aliases";
import { detectAndParse } from "../lib/input";
import type { TraceInput } from "../lib/types";
import { MappingDialog } from "./MappingDialog";
import { RecordList, type RecordListNote } from "./RecordList";

interface MappingRequest {
  rawRecords: LooseTraceInput[];
  sample: LooseTraceInput;
  initialMapping: Record<string, FieldAssignment>;
}

interface InputPanelProps {
  /** Fires with a fully-mapped TraceInput whenever a record is ready to
   * analyze: a single record from the paste/JSON/raw paths fires
   * immediately, a JSONL batch renders a list and fires once per row click. */
  onSelect: (record: TraceInput) => void;
  /** Bumped by the parent on every successful analysis from ANY source
   * (paste, gallery, live, hash-restore). A mapping dialog opened here for
   * an earlier paste must not survive an analysis that happened elsewhere —
   * without this, a stale dialog's Apply button could re-fire onSelect with
   * long-outdated data even though a different trace is already on screen. */
  resetToken: number;
}

const EMPTY_NOTE: RecordListNote = { hidden: true, text: "" };

export function InputPanel({ onSelect, resetToken }: InputPanelProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [note, setNote] = useState<RecordListNote>(EMPTY_NOTE);
  const [records, setRecords] = useState<TraceInput[]>([]);
  const [mapping, setMapping] = useState<MappingRequest | null>(null);

  useEffect(() => {
    setMapping(null);
  }, [resetToken]);

  function renderRecords(recs: TraceInput[]): void {
    if (recs.length === 1) {
      setRecords([]);
      onSelect(recs[0]);
      return;
    }
    setRecords(recs);
  }

  function processRecords(rawRecords: LooseTraceInput[]): void {
    const mapped = rawRecords.map((record, i) => mapRecord(record, String(i + 1)));
    const needsMapping = mapped.some((result) => result.missing.length > 0);

    if (needsMapping) {
      const sample = rawRecords[0] ?? {};
      setMapping({ rawRecords, sample, initialMapping: suggestMapping(sample) });
      setRecords([]);
      return;
    }

    renderRecords(mapped.map((result) => result.input!));
  }

  function handleText(text: string): void {
    if (text.trim().length === 0) return;
    const detected = detectAndParse(text);

    setNote(
      detected.capped
        ? {
            hidden: false,
            text: `Showing first ${detected.records.length.toLocaleString()} of ${detected.totalCount.toLocaleString()} records`,
          }
        : EMPTY_NOTE,
    );

    processRecords(detected.records);
  }

  function handleApplyMapping(chosenMapping: Record<string, FieldAssignment>): void {
    if (!mapping) return;
    const inputs = mapping.rawRecords.map((record, i) =>
      applyMapping(record, chosenMapping, String(i + 1)),
    );
    setMapping(null);
    renderRecords(inputs);
  }

  function handleDrop(event: React.DragEvent<HTMLDivElement>): void {
    event.preventDefault();
    setDragOver(false);
    const file = event.dataTransfer?.files?.[0];
    if (!file) return;
    void file.text().then((text) => {
      if (textareaRef.current) textareaRef.current.value = text;
      handleText(text);
    });
  }

  return (
    <div className="input-panel">
      <div
        className={dragOver ? "drop-zone drag-over" : "drop-zone"}
        tabIndex={0}
        onDragOver={(event) => {
          event.preventDefault();
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={handleDrop}
      >
        Drop a .jsonl, .json, or .txt file here
      </div>

      <textarea
        ref={textareaRef}
        className="paste-box"
        placeholder="...or paste a trace (JSON, JSONL, or raw text)"
        defaultValue=""
      />

      <button
        type="button"
        className="analyze-btn"
        onClick={() => handleText(textareaRef.current?.value ?? "")}
      >
        Analyze
      </button>

      <RecordList records={records} note={note} onSelect={onSelect} />

      {mapping && (
        <MappingDialog
          sample={mapping.sample}
          initialMapping={mapping.initialMapping}
          onApply={handleApplyMapping}
        />
      )}
    </div>
  );
}
