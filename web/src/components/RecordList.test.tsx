// @vitest-environment happy-dom

// Component tests for the record list: rows are keyed by position so
// duplicate ids in malformed JSONL never collide (React's dev-mode
// duplicate-key warning is the regression signal), and each row is
// keyboard-activatable (Enter/Space) in addition to click, matching its
// tabIndex={0} + role="button" semantics.

import { fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TraceInput } from "../lib/types";
import { RecordList, type RecordListNote } from "./RecordList";

const NOTE: RecordListNote = { hidden: true, text: "" };

function records(): TraceInput[] {
  return [
    { id: "dup", problem: "p1", thinking: "first thought", answer: "a1" },
    { id: "dup", problem: "p2", thinking: "second thought", answer: "a2" },
  ];
}

describe("RecordList: keying", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("renders both rows for duplicate-id records without a React duplicate-key warning", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <RecordList records={records()} note={NOTE} onSelect={onSelect} />,
    );

    const rows = container.querySelectorAll("li.record-row");
    expect(rows).toHaveLength(2);

    const keyWarning = consoleErrorSpy.mock.calls.some((args: unknown[]) =>
      args.some((arg: unknown) => typeof arg === "string" && arg.includes("same key")),
    );
    expect(keyWarning).toBe(false);
  });
});

describe("RecordList: keyboard activation", () => {
  it("exposes role=button and fires onSelect on Enter", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <RecordList records={records()} note={NOTE} onSelect={onSelect} />,
    );

    const row = container.querySelectorAll("li.record-row")[0];
    expect(row.getAttribute("role")).toBe("button");

    fireEvent.keyDown(row, { key: "Enter" });

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ thinking: "first thought" }),
    );
  });

  it("fires onSelect on Space and prevents the page from scrolling", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <RecordList records={records()} note={NOTE} onSelect={onSelect} />,
    );

    const row = container.querySelectorAll("li.record-row")[1];
    const event = new KeyboardEvent("keydown", { key: " ", bubbles: true, cancelable: true });
    const preventDefaultSpy = vi.spyOn(event, "preventDefault");
    row.dispatchEvent(event);

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ thinking: "second thought" }),
    );
    expect(preventDefaultSpy).toHaveBeenCalled();
  });

  it("does not fire onSelect for other keys", () => {
    const onSelect = vi.fn();
    const { container } = render(
      <RecordList records={records()} note={NOTE} onSelect={onSelect} />,
    );

    const row = container.querySelectorAll("li.record-row")[0];
    fireEvent.keyDown(row, { key: "Tab" });

    expect(onSelect).not.toHaveBeenCalled();
  });
});
