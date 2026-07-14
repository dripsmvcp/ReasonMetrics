// @vitest-environment happy-dom

import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TraceInput } from "../lib/types";
import { InputPanel } from "./InputPanel";

function setup() {
  const onSelect = vi.fn<(record: TraceInput) => void>();
  const { container } = render(<InputPanel onSelect={onSelect} resetToken={0} />);

  const textarea = container.querySelector<HTMLTextAreaElement>("textarea.paste-box")!;
  const button = container.querySelector<HTMLButtonElement>("button.analyze-btn")!;

  function paste(text: string) {
    fireEvent.change(textarea, { target: { value: text } });
    fireEvent.click(button);
  }

  return { container, onSelect, paste };
}

describe("InputPanel: paste box", () => {
  it("analyzes a single pasted JSON object immediately, without a list", () => {
    const { container, onSelect, paste } = setup();

    paste('{"problem":"2+2?","thinking":"4","answer":"4"}');

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ id: "1", problem: "2+2?", thinking: "4", answer: "4" });
    expect(container.querySelector("ul.record-list")?.hasAttribute("hidden")).toBe(true);
  });

  it("renders one row per record for a 3-record JSONL paste", () => {
    const { container, onSelect, paste } = setup();

    paste(
      [
        '{"id":"a","thinking":"first thought"}',
        '{"id":"b","thinking":"second thought"}',
        '{"id":"c","thinking":"third thought"}',
      ].join("\n"),
    );

    const rows = container.querySelectorAll("ul.record-list li.record-row");
    expect(rows).toHaveLength(3);
    expect(onSelect).not.toHaveBeenCalled();

    fireEvent.click(rows[1]);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: "b", thinking: "second thought" }),
    );
  });

  it("shows a capped note when a JSONL batch exceeds 1,000 records", () => {
    const { container, paste } = setup();
    const lines = Array.from({ length: 1200 }, (_, i) =>
      JSON.stringify({ id: String(i), thinking: `t${i}` }),
    );

    paste(lines.join("\n"));

    const note = container.querySelector("p.input-note")!;
    expect(note.hasAttribute("hidden")).toBe(false);
    expect(note.textContent).toContain("1,000");
    expect(note.textContent).toContain("1,200");
    expect(container.querySelectorAll("ul.record-list li.record-row")).toHaveLength(1000);
  });

  it("does not show the capped note for a batch under the limit", () => {
    const { container, paste } = setup();

    paste(['{"id":"1","thinking":"a"}', '{"id":"2","thinking":"b"}'].join("\n"));

    const note = container.querySelector("p.input-note")!;
    expect(note.hasAttribute("hidden")).toBe(true);
  });
});

describe("InputPanel: drag-and-drop", () => {
  it("reads a dropped file's text and feeds it through the same detection path", async () => {
    const { container, onSelect } = setup();
    const dropZone = container.querySelector(".drop-zone")!;
    const file = new File(['{"problem":"P","thinking":"T","answer":"A"}'], "trace.json", {
      type: "application/json",
    });

    fireEvent.drop(dropZone, { dataTransfer: { files: [file] } });

    await vi.waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith({ id: "1", problem: "P", thinking: "T", answer: "A" });
    });
  });
});

describe("InputPanel: click-to-browse", () => {
  // The zone has always shown `cursor: pointer` while clicking it did
  // nothing — and a keyboard user could focus it with no way to act on it.
  it("opens the file picker when the drop zone is clicked", () => {
    const { container } = setup();
    const dropZone = container.querySelector<HTMLElement>(".drop-zone")!;
    const picker = container.querySelector<HTMLInputElement>("input.file-picker")!;
    const click = vi.spyOn(picker, "click");

    fireEvent.click(dropZone);

    expect(click).toHaveBeenCalledTimes(1);
  });

  it.each(["Enter", " "])("opens the file picker on %s from the keyboard", (key) => {
    const { container } = setup();
    const dropZone = container.querySelector<HTMLElement>(".drop-zone")!;
    const picker = container.querySelector<HTMLInputElement>("input.file-picker")!;
    const click = vi.spyOn(picker, "click");

    fireEvent.keyDown(dropZone, { key });

    expect(click).toHaveBeenCalledTimes(1);
  });

  it("routes a picked file through the same detection path as a dropped one", async () => {
    const { container, onSelect } = setup();
    const picker = container.querySelector<HTMLInputElement>("input.file-picker")!;
    const file = new File(['{"problem":"P","thinking":"T","answer":"A"}'], "trace.json", {
      type: "application/json",
    });

    fireEvent.change(picker, { target: { files: [file] } });

    await vi.waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith({ id: "1", problem: "P", thinking: "T", answer: "A" });
    });
  });

  it("exposes the zone to assistive tech as an activatable control", () => {
    const { container } = setup();
    const dropZone = container.querySelector<HTMLElement>(".drop-zone")!;

    expect(dropZone.getAttribute("role")).toBe("button");
    expect(dropZone.tabIndex).toBe(0);
    expect(dropZone.getAttribute("aria-label")).toBeTruthy();
  });
});
