// Field-mapping dialog: shown when a pasted/dropped record's keys can't be
// resolved onto the canonical TraceInput fields via the alias table.
// Pre-filled from the caller's suggested mapping; Apply refuses (inline
// error, dialog stays open) until some key maps to thinking, the only
// required field. Alias resolution and validation are pure logic in
// ../lib/aliases.ts; this component only renders the selects and local
// in-progress choices.

import { useState } from "react";
import { hasThinkingAssignment, type FieldAssignment, type LooseTraceInput } from "../lib/aliases";

const CANONICAL_OPTIONS: { value: FieldAssignment; label: string }[] = [
  { value: "id", label: "id" },
  { value: "problem", label: "problem" },
  { value: "thinking", label: "thinking" },
  { value: "answer", label: "answer" },
  { value: "expected_answer", label: "expected_answer" },
  { value: "ignore", label: "(ignore)" },
];

interface MappingDialogProps {
  sample: LooseTraceInput;
  initialMapping: Record<string, FieldAssignment>;
  onApply: (mapping: Record<string, FieldAssignment>) => void;
}

export function MappingDialog({ sample, initialMapping, onApply }: MappingDialogProps) {
  const keys = Object.keys(sample);
  const [selections, setSelections] = useState<Record<string, FieldAssignment>>(() => {
    const initial: Record<string, FieldAssignment> = {};
    for (const key of keys) initial[key] = initialMapping[key] ?? "ignore";
    return initial;
  });
  const [showError, setShowError] = useState(false);

  function handleApply(): void {
    if (!hasThinkingAssignment(selections)) {
      setShowError(true);
      return;
    }
    onApply(selections);
  }

  return (
    <div className="mapping-dialog">
      <p>Some fields couldn't be matched automatically. Map them below:</p>
      {keys.map((key) => (
        <label className="mapping-row" key={key}>
          <span>{key}</span>
          <select
            data-key={key}
            value={selections[key]}
            onChange={(event) => {
              const value = event.target.value as FieldAssignment;
              setSelections((prev) => ({ ...prev, [key]: value }));
            }}
          >
            {CANONICAL_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
      ))}
      <p className="mapping-error" hidden={!showError}>
        Assign one field to thinking — it's the only required field.
      </p>
      <button type="button" className="mapping-apply" onClick={handleApply}>
        Apply mapping
      </button>
    </div>
  );
}
