#!/usr/bin/env python3
"""Correlate ReasonMetrics structural scores with LLM-judge semantic scores.

Joins one or more scored JSONL files (output of `reasonmetrics score`) with an
LLM-judge results file (output of scripts/llm_judge.py) and reports tie-aware
Spearman rank correlations per dimension pair, as a markdown table.

Usage:
    python scripts/calibrate.py --judged judge_results.jsonl \
        limo_scored.jsonl s1k_scored.jsonl medical_scored.jsonl

No third-party dependencies.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from typing import Any

STRUCT_DIMS = [
    "quality_score",
    "structural_score",
    "verification_score",
    "answer_alignment_score",
    "repetition_score",
    "overthinking_score",
    "efficiency_score",
    "length_score",
    "language_score",
]

JUDGE_DIMS = [
    "semantic_composite",
    "logical_validity",
    "factual_correctness",
    "answer_correctness",
    "reasoning_completeness",
]


def load_jsonl(path: str) -> list[dict[str, Any]]:
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def rank_with_ties(values: list[float]) -> list[float]:
    """Average ranks for ties (standard Spearman treatment)."""
    order = sorted(range(len(values)), key=lambda i: values[i])
    ranks = [0.0] * len(values)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and values[order[j + 1]] == values[order[i]]:
            j += 1
        avg_rank = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[order[k]] = avg_rank
        i = j + 1
    return ranks


def pearson(x: list[float], y: list[float]) -> float:
    n = len(x)
    mx = sum(x) / n
    my = sum(y) / n
    cov = sum((a - mx) * (b - my) for a, b in zip(x, y))
    vx = sum((a - mx) ** 2 for a in x)
    vy = sum((b - my) ** 2 for b in y)
    if vx == 0.0 or vy == 0.0:
        return float("nan")
    return cov / math.sqrt(vx * vy)


def spearman(x: list[float], y: list[float]) -> float:
    return pearson(rank_with_ties(x), rank_with_ties(y))


def dataset_of(trace_id: str) -> str:
    return trace_id.rsplit("_", 1)[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scored", nargs="+", help="scored JSONL file(s)")
    parser.add_argument("--judged", required=True, help="LLM-judge results JSONL")
    args = parser.parse_args()

    scored_by_id: dict[str, dict[str, Any]] = {}
    for path in args.scored:
        for rec in load_jsonl(path):
            scored_by_id[rec["id"]] = rec

    judged = [r for r in load_jsonl(args.judged) if not r.get("error")]
    pairs = [(scored_by_id[j["trace_id"]], j) for j in judged
             if j["trace_id"] in scored_by_id]
    dropped = len(judged) - len(pairs)
    if dropped:
        print(f"warning: {dropped} judged traces had no scored match",
              file=sys.stderr)
    n = len(pairs)
    if n < 10:
        sys.exit(f"only {n} joined pairs — not enough to correlate")

    datasets = sorted({dataset_of(j["trace_id"]) for _, j in pairs})
    print(f"joined pairs: n={n} across {datasets}\n")

    # Full Spearman matrix: structural dims x judge dims
    print("## Spearman rank correlation (structural vs judge)\n")
    header = "| structural \\ judge | " + " | ".join(JUDGE_DIMS) + " |"
    print(header)
    print("|" + "---|" * (len(JUDGE_DIMS) + 1))
    for sd in STRUCT_DIMS:
        xs = [float(s[sd]) for s, _ in pairs]
        row = [f"**{sd}**"]
        for jd in JUDGE_DIMS:
            ys = [float(j[jd]) for _, j in pairs]
            rho = spearman(xs, ys)
            row.append("n/a" if math.isnan(rho) else f"{rho:+.2f}")
        print("| " + " | ".join(row) + " |")

    # Composite correlation per dataset (guards against pooled-dataset artifacts)
    print("\n## quality_score vs semantic_composite, per dataset\n")
    print("| dataset | n | spearman |")
    print("|---|---|---|")
    for ds in datasets + ["ALL"]:
        sel = pairs if ds == "ALL" else [
            (s, j) for s, j in pairs if dataset_of(j["trace_id"]) == ds]
        xs = [float(s["quality_score"]) for s, _ in sel]
        ys = [float(j["semantic_composite"]) for _, j in sel]
        rho = spearman(xs, ys)
        cell = "n/a" if math.isnan(rho) else f"{rho:+.2f}"
        print(f"| {ds} | {len(sel)} | {cell} |")

    # Judge internal consistency for context
    print("\n## judge dimension intercorrelations\n")
    print("| pair | spearman |")
    print("|---|---|")
    for i in range(1, len(JUDGE_DIMS)):
        for k in range(i + 1, len(JUDGE_DIMS)):
            a, b = JUDGE_DIMS[i], JUDGE_DIMS[k]
            rho = spearman([float(j[a]) for _, j in pairs],
                           [float(j[b]) for _, j in pairs])
            cell = "n/a" if math.isnan(rho) else f"{rho:+.2f}"
            print(f"| {a} vs {b} | {cell} |")

    thresh = 1.96 / math.sqrt(n - 1)
    print(f"\nFor n={n}, |rho| > {thresh:.2f} is significant at ~p<0.05 "
          "(normal approximation).")


if __name__ == "__main__":
    main()
