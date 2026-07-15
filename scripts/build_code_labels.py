#!/usr/bin/env python3
"""Build an OBJECTIVE code-reasoning correctness label set — no code executed here.

The calibration/reweighting evidence (docs/CALIBRATION.md, issues #13/#31) is
entirely MATH: `build_correctness_labels.py` labels s1K traces by symbolic answer
verification. That leaves the open question of whether the scorer — and any
reweighting of it — transfers to a *different domain*. This produces the missing
input: objectively-labelled CODE reasoning traces.

Source: `PrimeIntellect/SYNTHETIC-1`, the raw (pre-SFT) verifiable-reasoning set.
Unlike the distillation/SFT sets (KodCode-SFT-R1, SYNTHETIC-1-SFT-Data), which are
curated to correct-only and so useless for an AUC study, the raw set KEEPS the
rejected samples: ~20% of code responses score 0.0. Each row carries the model's
response (reasoning), the task type, a `problem_id`, and a `score` that
PrimeIntellect computed by *verifying* the response (exact-match for
output-prediction, tests for code generation). We trust that score exactly as the
math pipeline trusts `math_verify` — no untrusted code is run here.

    pip install datasets
    python scripts/build_code_labels.py -o code_labelled.jsonl

Output is ReasonMetrics trace JSONL plus a `correct` boolean, ready for:

    reasonmetrics score -i code_labelled.jsonl -o code_scored.jsonl
    python scripts/filter_study.py code_scored.jsonl --labels code_labelled.jsonl
"""
from __future__ import annotations

import argparse
import json
import re
import sys

DATASET = "PrimeIntellect/SYNTHETIC-1"
THINK_RE = re.compile(r"<think>(.*?)</think>", re.S)


def extract_trace(response: str) -> tuple[str, str]:
    """Split a model response into (thinking, answer).

    R1-style responses wrap reasoning in <think>…</think> and put the final
    answer after. When there are no tags the whole response is the reasoning
    (there is no separate answer to peel off), which the scorer handles the same
    way as a tag-free trace.
    """
    m = THINK_RE.search(response)
    if m:
        thinking = m.group(1).strip()
        answer = response[m.end():].strip()
        return thinking, answer
    return response.strip(), ""


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-o", "--out", default="code_labelled.jsonl")
    ap.add_argument("--max-rows", type=int, default=None,
                    help="stop after scanning this many dataset rows")
    ap.add_argument("--limit", type=int, default=None,
                    help="stop after WRITING this many labelled traces")
    ap.add_argument("--task-substring", default="code",
                    help="keep rows whose task_type contains this (default: code)")
    args = ap.parse_args()

    try:
        from datasets import load_dataset
    except ImportError:
        sys.exit("needs: pip install datasets")

    ds = load_dataset(DATASET, split="train", streaming=True)

    rows, scanned, skipped = [], 0, 0
    for r in ds:
        scanned += 1
        if args.max_rows and scanned > args.max_rows:
            break

        if args.task_substring not in str(r.get("task_type") or ""):
            continue

        # The score is the verification result. Keep only the unambiguous ends:
        # 1.0 = correct, 0.0 = incorrect. Rare fractional scores (partial
        # test-pass, multi-part answers) are dropped rather than thresholded.
        score = str(r.get("score"))
        if score == "1.0":
            correct = True
        elif score == "0.0":
            correct = False
        else:
            skipped += 1
            continue

        thinking, answer = extract_trace(r.get("llm_response") or "")
        problem = (r.get("prompt") or "").strip()
        if not thinking or not problem:
            skipped += 1
            continue

        rows.append({
            "id": str(r.get("response_id") or f"syn1_{scanned}"),
            "problem_id": str(r.get("problem_id") or ""),
            "problem": problem,
            "thinking": thinking,
            "answer": answer,
            "domain": str(r.get("task_type") or "code"),
            "source": DATASET,
            "correct": correct,
        })
        if args.limit and len(rows) >= args.limit:
            break

    if not rows:
        sys.exit("no labelled rows produced")

    with open(args.out, "w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")

    n = len(rows)
    ncorr = sum(r["correct"] for r in rows)
    tagged = sum(1 for r in rows if r["answer"])  # had a <think> split
    tasks: dict[str, int] = {}
    for r in rows:
        tasks[r["domain"]] = tasks.get(r["domain"], 0) + 1

    print(f"wrote {args.out}: n={n} (scanned {scanned}, skipped {skipped})", file=sys.stderr)
    print(f"  correct   {ncorr:5d} ({100 * ncorr / n:.1f}%)", file=sys.stderr)
    print(f"  incorrect {n - ncorr:5d} ({100 * (n - ncorr) / n:.1f}%)", file=sys.stderr)
    print(f"  had <think> split: {tagged}/{n} ({100 * tagged / n:.1f}%)", file=sys.stderr)
    print(f"  task types: {tasks}", file=sys.stderr)


if __name__ == "__main__":
    main()
