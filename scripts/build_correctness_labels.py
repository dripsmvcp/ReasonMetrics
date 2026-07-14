#!/usr/bin/env python3
"""Build an OBJECTIVE correctness label set — no LLM judge in the loop.

The calibration study (docs/CALIBRATION.md) rests on 60 traces graded by a
lenient local 7B judge. This produces a much stronger evidence base for free:
s1K-1.1 ships, per problem, R1's real reasoning trace
(`deepseek_thinking_trajectory`), R1's final answer (`deepseek_attempt`), and the
human ground-truth `solution`. Comparing the two answers labels each trace
correct/incorrect.

Answer comparison is SYMBOLIC (`math_verify`, sympy-backed), not string equality:
\\dfrac{27}{2} and 13.5 are the same answer, and a string comparison marks them
different — which silently poisons the labels.

    pip install datasets math-verify
    python scripts/build_correctness_labels.py -o s1k_labelled.jsonl

Output is ReasonMetrics trace JSONL plus a `correct` boolean, ready for:

    reasonmetrics score -i s1k_labelled.jsonl -o s1k_scored.jsonl
"""

import argparse
import json
import sys


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-o", "--out", default="s1k_labelled.jsonl")
    ap.add_argument("--max-rows", type=int, default=None)
    args = ap.parse_args()

    try:
        from datasets import load_dataset
        from math_verify import parse, verify
    except ImportError:
        sys.exit("needs: pip install datasets math-verify")

    ds = load_dataset("simplescaling/s1K-1.1", split="train")
    if args.max_rows:
        ds = ds.select(range(min(args.max_rows, len(ds))))

    rows, skipped = [], 0
    for i, r in enumerate(ds):
        thinking = (r.get("deepseek_thinking_trajectory") or "").strip()
        attempt = (r.get("deepseek_attempt") or "").strip()
        truth = (r.get("solution") or "").strip()
        if not thinking or not attempt or not truth:
            skipped += 1
            continue

        gold, pred = parse(truth), parse(attempt)
        if not gold or not pred:
            skipped += 1
            continue
        try:
            correct = bool(verify(gold, pred))
        except Exception:
            skipped += 1  # unverifiable (proofs, set-valued answers, timeouts)
            continue

        rows.append({
            "id": f"s1k_{i}",
            "problem": r.get("question") or "",
            "thinking": thinking,
            "answer": attempt,
            "domain": r.get("cot_type") or "unknown",
            "source": "simplescaling/s1K-1.1",
            "correct": correct,
        })

    if not rows:
        sys.exit("no labelled rows produced")

    with open(args.out, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    n = len(rows)
    ncorr = sum(r["correct"] for r in rows)
    print(f"wrote {args.out}: n={n} (skipped {skipped} unverifiable)", file=sys.stderr)
    print(f"  correct   {ncorr:4d} ({100 * ncorr / n:.1f}%)", file=sys.stderr)
    print(f"  incorrect {n - ncorr:4d} ({100 * (n - ncorr) / n:.1f}%)", file=sys.stderr)


if __name__ == "__main__":
    main()
