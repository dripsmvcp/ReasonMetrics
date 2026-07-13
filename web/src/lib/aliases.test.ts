import { describe, expect, it } from "vitest";
import { applyMapping, hasThinkingAssignment, mapRecord, suggestMapping } from "./aliases";

describe("mapRecord", () => {
  it("resolves canonical field names directly", () => {
    const result = mapRecord({ id: "7", problem: "P", thinking: "T", answer: "A" });

    expect(result.input).toEqual({ id: "7", problem: "P", thinking: "T", answer: "A" });
    expect(result.unknownKeys).toEqual([]);
    expect(result.missing).toEqual([]);
  });

  it("resolves aliased field names (including numeric id)", () => {
    const result = mapRecord({ idx: 7, question: "P", reasoning: "T", solution: "A" });

    expect(result.input).toEqual({ id: "7", problem: "P", thinking: "T", answer: "A" });
  });

  it("reports keys that match no canonical name or alias", () => {
    const result = mapRecord({ thinking: "T", mystery_field: 42 });

    expect(result.input).toBeDefined();
    expect(result.unknownKeys).toEqual(["mystery_field"]);
  });

  it("flags a record with no resolvable thinking field as unmappable", () => {
    const result = mapRecord({ foo: "bar" });

    expect(result.input).toBeUndefined();
    expect(result.missing).toEqual(["thinking"]);
  });

  it("defaults missing problem/answer to empty string and id to the fallback", () => {
    const result = mapRecord({ thinking: "only this" }, "3");

    expect(result.input).toEqual({ id: "3", problem: "", thinking: "only this", answer: "" });
  });

  it("carries expected_answer through its alias", () => {
    const result = mapRecord({ thinking: "T", ground_truth: "42" });

    expect(result.input?.expected_answer).toBe("42");
  });
});

describe("suggestMapping", () => {
  it("pre-selects canonical fields via the alias table, ignoring unmatched keys", () => {
    const mapping = suggestMapping({ question: "P", weird: "?", reasoning: "T" });

    expect(mapping).toEqual({ question: "problem", weird: "ignore", reasoning: "thinking" });
  });
});

describe("applyMapping", () => {
  it("applies a user-chosen mapping across arbitrary keys", () => {
    const input = applyMapping(
      { a: "P", b: "T", c: "A" },
      { a: "problem", b: "thinking", c: "answer" },
      "2",
    );

    expect(input).toEqual({ id: "2", problem: "P", thinking: "T", answer: "A" });
  });

  it("ignores keys mapped to 'ignore' and defaults a missing id", () => {
    const input = applyMapping(
      { a: "P", b: "T", junk: "drop me" },
      { a: "problem", b: "thinking", junk: "ignore" },
    );

    expect(input).toEqual({ id: "1", problem: "P", thinking: "T", answer: "" });
  });
});

describe("hasThinkingAssignment", () => {
  it("flags a mapping with no key assigned to thinking as invalid", () => {
    expect(hasThinkingAssignment({ a: "problem", b: "ignore", c: "answer" })).toBe(false);
    expect(hasThinkingAssignment({})).toBe(false);
  });

  it("accepts a mapping where some key is assigned to thinking", () => {
    expect(hasThinkingAssignment({ a: "ignore", b: "thinking" })).toBe(true);
  });
});
