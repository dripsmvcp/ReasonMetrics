// @vitest-environment happy-dom

import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TraceInput } from "../lib/types";
import { InputPanel } from "./InputPanel";

function setup(resetToken = 0) {
  const onSelect = vi.fn<(record: TraceInput) => void>();
  const { container, rerender } = render(<InputPanel onSelect={onSelect} resetToken={resetToken} />);

  const textarea = container.querySelector<HTMLTextAreaElement>("textarea.paste-box")!;
  const button = container.querySelector<HTMLButtonElement>("button.analyze-btn")!;

  function paste(text: string) {
    fireEvent.change(textarea, { target: { value: text } });
    fireEvent.click(button);
  }

  function selectByKey(key: string): HTMLSelectElement {
    const rows = Array.from(container.querySelectorAll<HTMLElement>(".mapping-row"));
    const row = rows.find((r) => r.textContent?.startsWith(key));
    if (!row) throw new Error(`no mapping row for key "${key}"`);
    return row.querySelector("select")!;
  }

  function applyButton(): HTMLButtonElement {
    return container.querySelector<HTMLButtonElement>("button.mapping-apply")!;
  }

  return { container, onSelect, paste, selectByKey, applyButton, rerender };
}

describe("InputPanel: field-mapping dialog", () => {
  it("opens when a record has no resolvable thinking field, pre-filled via the alias table", () => {
    const { container, onSelect, paste } = setup();

    paste(JSON.stringify({ question: "2+2?", steps: "add them", output: "4" }));

    expect(onSelect).not.toHaveBeenCalled();
    const dialog = container.querySelector(".mapping-dialog");
    expect(dialog).not.toBeNull();

    const rows = Array.from(container.querySelectorAll<HTMLElement>(".mapping-row"));
    expect(rows.map((r) => r.textContent)).toEqual(
      expect.arrayContaining([
        expect.stringContaining("question"),
        expect.stringContaining("steps"),
        expect.stringContaining("output"),
      ]),
    );

    const values = Object.fromEntries(
      rows.map((r) => [r.querySelector("select")!.dataset.key, r.querySelector("select")!.value]),
    );
    expect(values.question).toBe("problem");
    expect(values.output).toBe("answer");
    expect(values.steps).toBe("ignore");
  });

  it("applies a user-corrected mapping and analyzes the single resulting record", () => {
    const { onSelect, paste, selectByKey, applyButton } = setup();

    paste(JSON.stringify({ question: "2+2?", steps: "add them", output: "4" }));

    fireEvent.change(selectByKey("steps"), { target: { value: "thinking" } });
    fireEvent.click(applyButton());

    expect(onSelect).toHaveBeenCalledWith({
      id: "1",
      problem: "2+2?",
      thinking: "add them",
      answer: "4",
    });
  });

  it("refuses to apply when no key is mapped to thinking", () => {
    const { container, onSelect, paste, selectByKey, applyButton } = setup();

    // No key matches any alias, so every dropdown pre-fills to "(ignore)" --
    // exactly the state the dialog opens in.
    paste(JSON.stringify({ foo: "2+2?", bar: "add them" }));

    fireEvent.click(applyButton());

    expect(onSelect).not.toHaveBeenCalled();
    expect(container.querySelector(".mapping-dialog")).not.toBeNull();
    const error = container.querySelector<HTMLElement>(".mapping-error")!;
    expect(error.hidden).toBe(false);
    expect(error.textContent).toContain("thinking");

    // Correcting the mapping recovers: the same dialog applies cleanly.
    fireEvent.change(selectByKey("bar"), { target: { value: "thinking" } });
    fireEvent.click(applyButton());

    expect(container.querySelector(".mapping-dialog")).toBeNull();
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ thinking: "add them" }));
  });

  it("applies the chosen mapping to every record in a JSONL batch", () => {
    const { container, onSelect, paste, selectByKey, applyButton } = setup();

    paste(
      [
        JSON.stringify({ id: "r1", steps: "first record steps" }),
        JSON.stringify({ id: "r2", steps: "second record steps" }),
      ].join("\n"),
    );

    fireEvent.change(selectByKey("steps"), { target: { value: "thinking" } });
    fireEvent.click(applyButton());

    expect(container.querySelector(".mapping-dialog")).toBeNull();
    const rows = container.querySelectorAll("ul.record-list li.record-row");
    expect(rows).toHaveLength(2);
    expect(onSelect).not.toHaveBeenCalled();

    fireEvent.click(rows[1]);
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: "r2", thinking: "second record steps" }),
    );
  });

  it("closes a stale dialog when an analysis happens elsewhere (resetToken bump)", () => {
    // Regression test for the old vanilla-DOM stale-dialog bug: opening the
    // mapping dialog from an ambiguous paste, then having some OTHER source
    // (gallery card, live stream, hash restore) complete an analysis, must
    // not leave this dialog's Apply button wired to long-outdated data.
    const { container, onSelect, paste, rerender } = setup(0);

    paste(JSON.stringify({ foo: "2+2?", bar: "add them" }));
    expect(container.querySelector(".mapping-dialog")).not.toBeNull();

    // Parent bumps resetToken because a different source just analyzed.
    rerender(<InputPanel onSelect={onSelect} resetToken={1} />);

    expect(container.querySelector(".mapping-dialog")).toBeNull();
  });
});
