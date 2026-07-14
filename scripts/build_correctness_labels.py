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


MODELS = {
    "deepseek": ("deepseek_thinking_trajectory", "deepseek_attempt", "deepseek_grade"),
    "gemini": ("gemini_thinking_trajectory", "gemini_attempt", "gemini_grade"),
}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-o", "--out", default="s1k_labelled.jsonl")
    ap.add_argument("--max-rows", type=int, default=None)
    ap.add_argument(
        "--models",
        default="deepseek",
        help="comma-separated: deepseek,gemini. Emitting both gives two traces "
        "per problem (same `problem_id`), which is what scripts/filter_study.py "
        "--paired needs to cancel problem difficulty.",
    )
    args = ap.parse_args()

    models = [m.strip() for m in args.models.split(",") if m.strip()]
    for m in models:
        if m not in MODELS:
            sys.exit(f"unknown model {m!r}; choose from {', '.join(MODELS)}")

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
        truth = (r.get("solution") or "").strip()
        if not truth:
            skipped += len(models)
            continue
        gold = parse(truth)
        if not gold:
            skipped += len(models)
            continue

        for model in models:
            think_col, attempt_col, grade_col = MODELS[model]
            thinking = (r.get(think_col) or "").strip()
            attempt = (r.get(attempt_col) or "").strip()
            if not thinking or not attempt:
                skipped += 1
                continue

            pred = parse(attempt)
            if not pred:
                skipped += 1
                continue
            try:
                correct = bool(verify(gold, pred))
            except Exception:
                skipped += 1  # unverifiable (proofs, set-valued answers, timeouts)
                continue

            # Single-model runs keep the original flat id, so the published
            # n=938 study reproduces byte-for-byte.
            trace_id = f"s1k_{i}" if len(models) == 1 else f"s1k_{i}_{model}"
            rows.append({
                "id": trace_id,
                "problem_id": f"s1k_{i}",
                "model": model,
                "problem": r.get("question") or "",
                "thinking": thinking,
                "answer": attempt,
                "domain": r.get("cot_type") or "unknown",
                "source": "simplescaling/s1K-1.1",
                "correct": correct,
                # s1K ships its own grade of the same attempt — carried through
                # so the symbolic labels can be cross-checked against it.
                "dataset_grade": (r.get(grade_col) or "").strip() or None,
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

    for model in models:
        sub = [r for r in rows if r["model"] == model]
        if sub:
            c = sum(r["correct"] for r in sub)
            print(f"  [{model}] n={len(sub)} correct={c} ({100 * c / len(sub):.1f}%)",
                  file=sys.stderr)

    if len(models) > 1:
        by_problem: dict[str, int] = {}
        for r in rows:
            by_problem[r["problem_id"]] = by_problem.get(r["problem_id"], 0) + 1
        complete = sum(1 for v in by_problem.values() if v == len(models))
        print(f"  complete pairs (all {len(models)} models labelled): {complete}",
              file=sys.stderr)

    # Cross-check the symbolic labels against the dataset's own grade, where it
    # has one. Disagreement here is a label-quality warning, not a hard error:
    # the two use different notions of "correct".
    graded = [r for r in rows if r["dataset_grade"]]
    if graded:
        def truthy(g: str) -> bool:
            return g.strip().lower() in {"yes", "true", "correct", "1"}

        agree = sum(1 for r in graded if truthy(r["dataset_grade"]) == r["correct"])
        print(f"  agreement with s1K's own grade: {agree}/{len(graded)} "
              f"({100 * agree / len(graded):.1f}%)", file=sys.stderr)


if __name__ == "__main__":
    main()
