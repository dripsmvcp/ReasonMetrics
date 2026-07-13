// @vitest-environment happy-dom

// Component tests for the gallery strip: renders one card per index entry,
// clicking a card loads its fixture and fires the same onSelect callback
// the input panel uses, and a failed index fetch hides the strip instead of
// throwing. Fetch is stubbed — fixture parsing/mapping is covered in
// ../lib/gallery.test.ts.

import { fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TraceInput } from "../lib/types";
import { GalleryStrip } from "./GalleryStrip";

const INDEX = [
  { id: "r1-rambling", label: "R1 rambling", description: "overthinks 15% of 80", file: "r1-rambling.json" },
  { id: "concise-qwen", label: "Concise Qwen", description: "short and clean", file: "concise-qwen.json" },
  { id: "language-mixing", label: "Language mixing", description: "thinking switches languages", file: "language-mixing.json" },
  { id: "restart-loop", label: "Restart loop", description: "wait, let me try again", file: "restart-loop.json" },
  { id: "verified-tidy", label: "Verified & tidy", description: "checks its own answer", file: "verified-tidy.json" },
];

const FIXTURE = {
  id: "concise-qwen",
  label: "Concise Qwen",
  description: "short and clean",
  model: "qwen3:1.7b",
  problem: "What is 12 + 30?",
  thinking: "12 plus 30 is 42.",
  answer: "42",
  generated_with: { prompt: "What is 12 + 30?", options: {} },
  curated: false,
};

function stubFetchRouting(): ReturnType<typeof vi.fn> {
  const fn = vi.fn((url: string) => {
    if (url.endsWith("/gallery/index.json")) {
      return Promise.resolve(new Response(JSON.stringify(INDEX), { status: 200 }));
    }
    if (url.endsWith("/gallery/concise-qwen.json")) {
      return Promise.resolve(new Response(JSON.stringify(FIXTURE), { status: 200 }));
    }
    return Promise.resolve(new Response("not found", { status: 404 }));
  });
  vi.stubGlobal("fetch", fn);
  return fn;
}

function setup() {
  const onSelect = vi.fn<(record: TraceInput) => void>();
  const { container } = render(<GalleryStrip onSelect={onSelect} />);
  return { container, onSelect };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("GalleryStrip", () => {
  it("renders one card per index entry with label and description text", async () => {
    stubFetchRouting();
    const { container } = setup();

    const cards = await vi.waitFor(() => {
      const found = container.querySelectorAll("button.gallery-card");
      expect(found).toHaveLength(5);
      return found;
    });

    expect(cards[0].textContent).toContain("R1 rambling");
    expect(cards[0].textContent).toContain("overthinks 15% of 80");
    expect(cards[4].textContent).toContain("Verified & tidy");
  });

  it("loads the clicked card's fixture and fires onSelect with the mapped TraceInput", async () => {
    stubFetchRouting();
    const { container, onSelect } = setup();

    const cards = await vi.waitFor(() => {
      const found = container.querySelectorAll("button.gallery-card");
      expect(found).toHaveLength(5);
      return found;
    });

    fireEvent.click(cards[1]);

    await vi.waitFor(() => {
      expect(onSelect).toHaveBeenCalledTimes(1);
    });
    expect(onSelect).toHaveBeenCalledWith({
      id: "concise-qwen",
      problem: "What is 12 + 30?",
      thinking: "12 plus 30 is 42.",
      answer: "42",
    });
  });

  it("hides the strip without throwing when the index fetch fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("nope", { status: 404 })));

    const { container, onSelect } = setup();

    await vi.waitFor(() => {
      expect(container.querySelector(".gallery-strip")).toBeNull();
    });
    expect(onSelect).not.toHaveBeenCalled();
  });
});
