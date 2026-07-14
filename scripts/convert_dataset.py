#!/usr/bin/env python3
"""
Convert HuggingFace reasoning trace datasets to ReasonMetrics JSONL format.

Usage:
    python scripts/convert_dataset.py limo
    python scripts/convert_dataset.py s1k
    python scripts/convert_dataset.py openthoughts --max-rows 10000
    python scripts/convert_dataset.py opencoder --max-rows 5000
    python scripts/convert_dataset.py medical --max-rows 5000

Requires:
    pip install datasets
"""

import argparse
import json
import sys
import time
from pathlib import Path


def _progress(iterable, total: int, prefix: str = ""):
    """Simple inline progress bar — no extra dependencies."""
    start = time.time()
    for i, item in enumerate(iterable, 1):
        yield item
        if i % 50 == 0 or i == total:
            pct = i / total
            bar_len = 40
            filled = int(bar_len * pct)
            bar = "█" * filled + "░" * (bar_len - filled)
            elapsed = time.time() - start
            rate = i / elapsed if elapsed > 0 else 0
            sys.stderr.write(
                f"\r  {prefix} {bar} {i}/{total} ({rate:.0f}/s)"
            )
            sys.stderr.flush()
    sys.stderr.write("\n")


def _require(row: dict, column: str, dataset: str) -> str:
    """Read a column that MUST exist, or fail loudly.

    Silent `row.get(a, row.get(b, ""))` fallback chains are how a schema
    mismatch turns into a corrupt corpus instead of an error: the s1K
    converter used to fall back to `solution` — the GROUND-TRUTH answer — as
    the reasoning trace, so the scorers were fed polished reference prose that
    no model ever reasoned. A missing column is a bug in the converter, and
    must stop the run rather than quietly produce garbage.
    """
    value = row.get(column)
    if not isinstance(value, str) or not value.strip():
        raise KeyError(
            f"{dataset}: required column {column!r} is missing or empty — "
            f"the dataset schema has changed. Columns present: {sorted(row)}"
        )
    return value


def convert_limo(max_rows: int | None) -> tuple[str, list[dict]]:
    """GAIR/LIMO — 817 curated math reasoning traces."""
    from datasets import load_dataset
    ds = load_dataset("GAIR/LIMO", split="train")
    if max_rows:
        ds = ds.select(range(min(max_rows, len(ds))))

    total = len(ds)
    traces = []
    for i, row in enumerate(_progress(ds, total, "LIMO")):
        traces.append({
            "id": f"limo_{i}",
            "problem": row.get("question", row.get("problem", "")),
            "thinking": row.get("solution", row.get("reasoning", "")),
            "answer": row.get("answer", ""),
            "source": "GAIR/LIMO",
            "domain": "math",
        })
    return "limo_traces.jsonl", traces


def convert_s1k(max_rows: int | None) -> tuple[str, list[dict]]:
    """simplescaling/s1K-1.1 — 1K multi-domain traces.

    Column semantics (dataset card):
      question                     — the problem
      solution                     — the GROUND-TRUTH solution (NOT a trace)
      deepseek_thinking_trajectory — the reasoning trace R1 actually produced
      deepseek_attempt             — R1's final answer

    `solution` is reference prose written by humans/verifiers; scoring it as a
    reasoning trace measures nothing about a model's thinking and inflates
    every structure/verification signal. docs/CALIBRATION.md has always
    documented `deepseek_thinking_trajectory` as this corpus's thinking field —
    this converter is what disagreed.
    """
    from datasets import load_dataset
    ds = load_dataset("simplescaling/s1K-1.1", split="train")
    if max_rows:
        ds = ds.select(range(min(max_rows, len(ds))))

    total = len(ds)
    traces = []
    for i, row in enumerate(_progress(ds, total, "s1K")):
        traces.append({
            "id": f"s1k_{i}",
            "problem": _require(row, "question", "s1K-1.1"),
            "thinking": _require(row, "deepseek_thinking_trajectory", "s1K-1.1"),
            # R1's own answer, so problem/thinking/answer all come from the
            # same generation. `answer` was read from a column that does not
            # exist in this dataset, so it was silently always "".
            "answer": row.get("deepseek_attempt", ""),
            "source": "simplescaling/s1K-1.1",
            "domain": row.get("cot_type", "multi"),
        })
    return "s1k_traces.jsonl", traces


def convert_openthoughts(max_rows: int | None) -> tuple[str, list[dict]]:
    """open-thoughts/OpenThoughts-114k — large multi-domain dataset."""
    from datasets import load_dataset
    ds = load_dataset("open-thoughts/OpenThoughts-114k", split="train")
    if max_rows:
        ds = ds.select(range(min(max_rows, len(ds))))

    total = len(ds)
    traces = []
    for i, row in enumerate(_progress(ds, total, "OpenThoughts")):
        conversations = row.get("conversations", [])
        problem = ""
        thinking = ""
        for msg in conversations:
            role = msg.get("from", msg.get("role", ""))
            content = msg.get("value", msg.get("content", ""))
            if role in ("human", "user"):
                problem = content
            elif role in ("assistant", "gpt"):
                thinking = content

        traces.append({
            "id": f"openthoughts_{i}",
            "problem": problem,
            "thinking": thinking,
            "answer": row.get("chosen", row.get("answer", "")),
            "source": "open-thoughts/OpenThoughts-114k",
            "domain": row.get("domain", row.get("source", "unknown")),
        })
    return "openthoughts_traces.jsonl", traces


def convert_opencoder(max_rows: int | None) -> tuple[str, list[dict]]:
    """nvidia/OpenCodeReasoning — code reasoning traces.

    Uses streaming mode to avoid downloading the full 735K-row dataset.
    """
    from datasets import load_dataset
    # Stream to avoid multi-GB download; config='split_0', split='split_0'
    ds = load_dataset(
        "nvidia/OpenCodeReasoning", "split_0", split="split_0", streaming=True
    )

    limit = max_rows or 5000
    traces = []
    sys.stderr.write(f"  Streaming up to {limit} rows from OpenCodeReasoning...\n")
    start = time.time()
    for i, row in enumerate(ds):
        if i >= limit:
            break
        # Fields: input=problem, output=<think>reasoning</think>, solution=code
        problem = row.get("input", "")
        thinking = row.get("output", "")
        answer = row.get("solution", "")
        traces.append({
            "id": f"opencoder_{i}",
            "problem": problem,
            "thinking": thinking,
            "answer": answer,
            "source": "nvidia/OpenCodeReasoning",
            "domain": "code",
        })
        if (i + 1) % 100 == 0:
            elapsed = time.time() - start
            rate = (i + 1) / elapsed if elapsed > 0 else 0
            bar_len = 40
            pct = (i + 1) / limit
            filled = int(bar_len * pct)
            bar = "█" * filled + "░" * (bar_len - filled)
            sys.stderr.write(f"\r  OpenCodeReasoning {bar} {i+1}/{limit} ({rate:.0f}/s)")
            sys.stderr.flush()
    sys.stderr.write("\n")
    return "opencoder_traces.jsonl", traces


def convert_medical(max_rows: int | None) -> tuple[str, list[dict]]:
    """FreedomIntelligence/medical-o1-reasoning-SFT — medical reasoning."""
    from datasets import load_dataset
    ds = load_dataset("FreedomIntelligence/medical-o1-reasoning-SFT", "en", split="train")
    if max_rows:
        ds = ds.select(range(min(max_rows, len(ds))))

    total = len(ds)
    traces = []
    for i, row in enumerate(_progress(ds, total, "Medical")):
        problem = row.get("Question", row.get("question", ""))
        thinking = row.get("Complex_CoT", row.get("complex_cot", ""))
        answer = row.get("Response", row.get("response", ""))

        traces.append({
            "id": f"medical_{i}",
            "problem": problem,
            "thinking": thinking,
            "answer": answer,
            "source": "FreedomIntelligence/medical-o1-reasoning-SFT",
            "domain": "medical",
        })
    return "medical_traces.jsonl", traces


CONVERTERS = {
    "limo": convert_limo,
    "s1k": convert_s1k,
    "openthoughts": convert_openthoughts,
    "opencoder": convert_opencoder,
    "medical": convert_medical,
}


def main():
    parser = argparse.ArgumentParser(
        description="Convert HuggingFace datasets to ReasonMetrics JSONL format"
    )
    parser.add_argument(
        "dataset",
        choices=list(CONVERTERS.keys()),
        help="Dataset to convert",
    )
    parser.add_argument(
        "--max-rows",
        type=int,
        default=None,
        help="Maximum rows to convert (for large datasets)",
    )
    args = parser.parse_args()

    print(f"Loading {args.dataset}...", file=sys.stderr)
    converter = CONVERTERS[args.dataset]
    output_file, traces = converter(args.max_rows)

    # Filter out traces with empty thinking
    original_count = len(traces)
    traces = [t for t in traces if t["thinking"].strip()]
    skipped = original_count - len(traces)

    output_path = Path(output_file)
    # encoding is explicit because the payload is deliberately NOT ASCII-escaped
    # (ensure_ascii=False): under a C/POSIX locale Python picks latin-1 here and
    # the write dies on the first non-ASCII character — a `≠` in a math trace, or
    # any Chinese/Japanese reasoning, which is precisely the corpus this project
    # cares about.
    with open(output_path, "w", encoding="utf-8") as f:
        for trace in traces:
            f.write(json.dumps(trace, ensure_ascii=False) + "\n")

    print(f"Wrote {len(traces)} traces to {output_path}", file=sys.stderr)
    if skipped:
        print(f"  (skipped {skipped} traces with empty thinking)", file=sys.stderr)


if __name__ == "__main__":
    main()
