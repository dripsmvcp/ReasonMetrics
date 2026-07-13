import { describe, expect, it } from "vitest";
import { detectAndParse, JSONL_RECORD_CAP } from "./input";

describe("detectAndParse", () => {
  it("detects a single whole-text JSON object", () => {
    const result = detectAndParse('{"problem":"2+2?","thinking":"4","answer":"4"}');

    expect(result.format).toBe("json");
    expect(result.records).toEqual([{ problem: "2+2?", thinking: "4", answer: "4" }]);
    expect(result.totalCount).toBe(1);
    expect(result.capped).toBe(false);
  });

  it("detects JSONL, one record per non-empty line", () => {
    const text = [
      '{"id":"1","thinking":"a"}',
      "",
      '{"id":"2","thinking":"b"}',
      '{"id":"3","thinking":"c"}',
    ].join("\n");

    const result = detectAndParse(text);

    expect(result.format).toBe("jsonl");
    expect(result.records).toHaveLength(3);
    expect(result.records[1]).toEqual({ id: "2", thinking: "b" });
    expect(result.totalCount).toBe(3);
    expect(result.capped).toBe(false);
  });

  it("falls back to raw text when the input is neither JSON nor JSONL", () => {
    const result = detectAndParse("Just thinking out loud about the problem.");

    expect(result.format).toBe("raw");
    expect(result.records).toEqual([
      { problem: "", thinking: "Just thinking out loud about the problem.", answer: "" },
    ]);
    expect(result.totalCount).toBe(1);
    expect(result.capped).toBe(false);
  });

  it("does not require a literal <think> tag to accept raw text", () => {
    const result = detectAndParse("no tags here, just prose");

    expect(result.format).toBe("raw");
    expect(result.records[0].thinking).toBe("no tags here, just prose");
  });

  it("caps JSONL at 1,000 records and reports the true total", () => {
    const lines = Array.from({ length: 1200 }, (_, i) =>
      JSON.stringify({ id: String(i), thinking: `t${i}` }),
    );

    const result = detectAndParse(lines.join("\n"));

    expect(result.format).toBe("jsonl");
    expect(result.records).toHaveLength(JSONL_RECORD_CAP);
    expect(result.totalCount).toBe(1200);
    expect(result.capped).toBe(true);
  });

  it("does not cap when the record count is exactly at the limit", () => {
    const lines = Array.from({ length: JSONL_RECORD_CAP }, (_, i) =>
      JSON.stringify({ id: String(i), thinking: `t${i}` }),
    );

    const result = detectAndParse(lines.join("\n"));

    expect(result.records).toHaveLength(JSONL_RECORD_CAP);
    expect(result.totalCount).toBe(JSONL_RECORD_CAP);
    expect(result.capped).toBe(false);
  });
});
