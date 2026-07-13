// The 1,000-record cap note plus the JSONL record list: one row per parsed
// record, click to analyze. Purely presentational — InputPanel owns the
// parsing/selection logic and just hands this component what to show.

import type { TraceInput } from "../lib/types";

export interface RecordListNote {
  hidden: boolean;
  text: string;
}

interface RecordListProps {
  records: TraceInput[];
  note: RecordListNote;
  onSelect: (record: TraceInput) => void;
}

export function RecordList({ records, note, onSelect }: RecordListProps) {
  return (
    <>
      <p className="input-note" hidden={note.hidden}>
        {note.text}
      </p>
      <ul className="record-list" hidden={records.length === 0}>
        {records.map((record, i) => (
          <li
            key={i}
            className="record-row"
            tabIndex={0}
            role="button"
            onClick={() => onSelect(record)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSelect(record);
              }
            }}
          >
            {`${record.id} — ${record.thinking.slice(0, 80)}`}
          </li>
        ))}
      </ul>
    </>
  );
}
