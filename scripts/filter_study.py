#!/usr/bin/env python3
"""Does filtering by score actually produce a better trace set?

Measures the thing issue #30 is about. Given traces that carry an OBJECTIVE
`correct` label (see scripts/build_correctness_labels.py — symbolic answer
verification, no LLM judge), this answers three questions in order:

1. **Does percentile filtering work?** Keep the top-N% by `quality_score` and
   compare the kept set's answer-correctness rate against the unfiltered
   baseline, against the traces it dropped, and against a random drop of the
   same size (the null).

2. **Does the composite beat a one-line heuristic?** Length alone ("keep the
   shortest") is the control. Nine scored dimensions have to earn their keep
   over `wc -w`; where they don't, we say so.

3. **Is the absolute threshold usable?** Reports what `--min-score X` actually
   retains, which is the defect in #30.

Every headline number is reported with a bootstrap CI, because a lift that is
inside the noise is not a finding — this repo has already retracted one result
that was (see the Simpson artifact retraction in docs/CALIBRATION.md).

Usage:
    reasonmetrics score -i s1k_labelled.jsonl -o s1k_labelled_scored.jsonl
    python scripts/filter_study.py s1k_labelled_scored.jsonl \
        --labels s1k_labelled.jsonl

No third-party dependencies.
"""
from __future__ import annotations

import argparse
import json
import random
import sys
from typing import Any

DIMS = [
    "quality_score",
    "structural_score",
    "length_score",
    "overthinking_score",
    "verification_score",
    "repetition_score",
    "language_score",
    "efficiency_score",
    "answer_alignment_score",
]

RETENTIONS = [90, 80, 70, 60, 50, 40, 30, 20, 10]
BOOTSTRAP_N = 2000
SEED = 0


def load_jsonl(path: str) -> list[dict[str, Any]]:
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def auc(scores: list[float], labels: list[bool]) -> float:
    """P(positive outranks negative), tie-aware. 0.5 = coin flip.

    Rank-based Mann-Whitney U; O(n log n).
    """
    pos = sum(labels)
    neg = len(labels) - pos
    if pos == 0 or neg == 0:
        return float("nan")

    order = sorted(range(len(scores)), key=lambda i: scores[i])
    ranks = [0.0] * len(scores)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and scores[order[j + 1]] == scores[order[i]]:
            j += 1
        avg = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[order[k]] = avg
        i = j + 1

    rank_sum_pos = sum(r for r, lab in zip(ranks, labels) if lab)
    return (rank_sum_pos - pos * (pos + 1) / 2.0) / (pos * neg)


def bootstrap_ci(
    fn, rows: list[dict], rnd: random.Random, n_boot: int = BOOTSTRAP_N
) -> tuple[float, float]:
    """Percentile bootstrap 95% CI of fn(resampled rows)."""
    n = len(rows)
    vals = []
    for _ in range(n_boot):
        sample = [rows[rnd.randrange(n)] for _ in range(n)]
        v = fn(sample)
        if v == v:  # skip NaN (degenerate resample)
            vals.append(v)
    vals.sort()
    lo = vals[int(0.025 * len(vals))]
    hi = vals[int(0.975 * len(vals)) - 1]
    return lo, hi


def accuracy(rows: list[dict]) -> float:
    return sum(r["correct"] for r in rows) / len(rows) if rows else float("nan")


def keep_top(rows: list[dict], key: str, pct: int, reverse: bool) -> tuple[list, list]:
    srt = sorted(rows, key=lambda r: r[key], reverse=reverse)
    k = int(round(len(rows) * pct / 100))
    return srt[:k], srt[k:]


def sweep_table(rows: list[dict], key: str, reverse: bool, rnd: random.Random) -> str:
    base = accuracy(rows)
    out = [
        "| keep | n | accuracy kept | accuracy dropped | lift vs unfiltered |",
        "|---|---|---|---|---|",
    ]
    for pct in RETENTIONS:
        kept, dropped = keep_top(rows, key, pct, reverse)
        ak, ad = accuracy(kept), accuracy(dropped)

        def lift(sample: list[dict], _pct: int = pct) -> float:
            k, _ = keep_top(sample, key, _pct, reverse)
            return accuracy(k) - accuracy(sample)

        lo, hi = bootstrap_ci(lift, rows, rnd, n_boot=400)
        sig = "" if lo <= 0 <= hi else "**"
        out.append(
            f"| top {pct}% | {len(kept)} | {ak:.1%} | {ad:.1%} | "
            f"{sig}{ak - base:+.1%}{sig} [{lo:+.1%}, {hi:+.1%}] |"
        )
    return "\n".join(out)


def binom_two_sided(k: int, n: int) -> float:
    """Exact two-sided binomial p-value against p=0.5."""
    if n == 0:
        return float("nan")
    from math import comb

    tail = min(k, n - k)
    one_sided = sum(comb(n, i) for i in range(tail + 1)) / (2**n)
    return min(1.0, 2 * one_sided)


def paired_report(rows: list[dict]) -> None:
    """Within-problem test: does the scorer rank the CORRECT trace higher?

    Every problem carries two traces (different models) answering the SAME
    question. Restricting to discordant pairs — exactly one of the two is
    correct — holds problem difficulty exactly constant. A between-problem
    result cannot distinguish "this trace is better" from "this problem was
    easier"; this can. Chance is 50%.
    """
    pairs: dict[str, list[dict]] = {}
    for r in rows:
        pairs.setdefault(r["problem_id"], []).append(r)

    discordant = [
        p for p in pairs.values()
        if len(p) == 2 and sum(x["correct"] for x in p) == 1
    ]
    concordant = len(pairs) - len(discordant)

    print("\n# Paired within-problem test (difficulty held constant)\n")
    print(f"{len(pairs)} problems with both traces labelled; "
          f"**{len(discordant)} discordant** (exactly one trace correct — the "
          f"informative ones), {concordant} concordant.\n")
    if not discordant:
        print("_No discordant pairs — nothing to test._")
        return

    print("Of the discordant pairs, how often does the signal score the "
          "correct trace above the incorrect one? Chance = 50%.\n")
    print("| signal | correct trace ranked higher | p (two-sided) |")
    print("|---|---|---|")

    signals: list[tuple[str, Any]] = [
        (d, (lambda r, d=d: r[d])) for d in DIMS if all(d in r for r in rows)
    ]
    signals.append(
        ("*(length alone, shorter=better)*", lambda r: -r["thinking_word_count"])
    )

    results = []
    for name, get in signals:
        wins = ties = 0
        for pair in discordant:
            good = next(x for x in pair if x["correct"])
            bad = next(x for x in pair if not x["correct"])
            if get(good) > get(bad):
                wins += 1
            elif get(good) == get(bad):
                ties += 1
        n_eff = len(discordant) - ties
        rate = wins / n_eff if n_eff else float("nan")
        p = binom_two_sided(wins, n_eff)
        results.append((rate, name, wins, n_eff, p))

    for rate, name, wins, n_eff, p in sorted(results, reverse=True):
        star = "**" if p < 0.05 else ""
        print(f"| {name} | {star}{rate:.1%}{star} ({wins}/{n_eff}) | {p:.3g} |")

    print(
        "\nA signal at 50% here carries **no** trace-quality information: its "
        "apparent skill in the pooled study would be problem difficulty "
        "leaking in (harder problem → longer trace → more likely wrong).\n"
    )


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("scored", help="output of `reasonmetrics score`")
    ap.add_argument(
        "--labels",
        required=True,
        help="JSONL with {id, correct} (scoring drops unknown fields)",
    )
    ap.add_argument("--min-score", type=float, default=70.0)
    ap.add_argument(
        "--paired",
        action="store_true",
        help="also run the within-problem paired test (needs `problem_id` on "
        "the labels — build them with --models deepseek,gemini)",
    )
    args = ap.parse_args()

    # `reasonmetrics score` drops unknown fields, so the label file is the only
    # carrier of `correct` / `problem_id`. Join on id.
    labels = {r["id"]: r for r in load_jsonl(args.labels) if "correct" in r}
    if not labels:
        sys.exit(f"{args.labels}: no rows carry a `correct` field")

    rows = []
    for r in load_jsonl(args.scored):
        lab = labels.get(r["id"])
        if lab is None:
            continue
        r["correct"] = bool(lab["correct"])
        for k in ("problem_id", "model"):
            if k in lab:
                r[k] = lab[k]
        rows.append(r)
    if not rows:
        sys.exit("no scored rows joined to a label — check that `id` survives scoring")

    if args.paired and not all("problem_id" in r for r in rows):
        sys.exit(
            "--paired needs `problem_id` on every label; rebuild them with "
            "`build_correctness_labels.py --models deepseek,gemini`"
        )

    rnd = random.Random(SEED)
    n = len(rows)
    base = accuracy(rows)

    print(f"# Filter study (n={n})\n")
    print(f"Baseline: **{base:.1%}** of traces reach the correct answer "
          f"({sum(r['correct'] for r in rows)}/{n}).\n")

    # --- 3. what does the absolute threshold actually retain?
    kept_abs = [r for r in rows if r["quality_score"] >= args.min_score]
    drop_abs = [r for r in rows if r["quality_score"] < args.min_score]
    retained = len(kept_abs) / n
    print("## The default threshold\n")
    print(f"`filter --min-score {args.min_score:g}` keeps **{len(kept_abs)}/{n} "
          f"({retained:.1%})** — accuracy {accuracy(kept_abs):.1%} vs "
          f"{base:.1%} unfiltered.")
    if retained > 0.95:
        print("\n**Broken (issue #30).** A threshold that keeps essentially "
              "everything discriminates nothing, whatever its ranking is worth.\n")
    else:
        print(f" It drops {len(drop_abs)} traces which are "
              f"{accuracy(drop_abs):.1%} correct — a gap of "
              f"**{accuracy(kept_abs) - accuracy(drop_abs):+.1%}** between kept "
              f"and dropped.\n")

    # --- 1. does percentile filtering work?
    print("## Percentile filter: top-N% by `quality_score`\n")
    print("Bootstrap 95% CI on the lift; `**` = CI excludes zero.\n")
    print(sweep_table(rows, "quality_score", True, rnd))

    print("\n### Null: random drop of the same size\n")
    print("| keep | mean accuracy kept | 95% CI |")
    print("|---|---|---|")
    idx = list(range(n))
    for pct in (70, 50, 30):
        k = int(round(n * pct / 100))
        accs = []
        for _ in range(BOOTSTRAP_N):
            rnd.shuffle(idx)
            accs.append(sum(rows[i]["correct"] for i in idx[:k]) / k)
        accs.sort()
        mean = sum(accs) / len(accs)
        lo = accs[int(0.025 * len(accs))]
        hi = accs[int(0.975 * len(accs)) - 1]
        print(f"| top {pct}% | {mean:.1%} | [{lo:.1%}, {hi:.1%}] |")

    # --- 2. the control: does the composite beat "keep the shortest"?
    print("\n## Control: keep the SHORTEST N% (word count alone)\n")
    print(sweep_table(rows, "thinking_word_count", False, rnd))

    print("\n## AUC per dimension (P(correct trace outranks incorrect one))\n")
    print("| dimension | AUC | 95% CI |")
    print("|---|---|---|")
    ranked = []
    for dim in DIMS:
        if not all(dim in r for r in rows):
            continue
        a = auc([r[dim] for r in rows], [r["correct"] for r in rows])
        lo, hi = bootstrap_ci(
            lambda s, d=dim: auc([r[d] for r in s], [r["correct"] for r in s]), rows, rnd
        )
        ranked.append((a, dim, lo, hi))
    # length as a negative-direction predictor: shorter = better
    a_len = auc([-r["thinking_word_count"] for r in rows], [r["correct"] for r in rows])
    lo_len, hi_len = bootstrap_ci(
        lambda s: auc([-r["thinking_word_count"] for r in s], [r["correct"] for r in s]),
        rows,
        rnd,
    )
    ranked.append((a_len, "*(length alone, shorter=better)*", lo_len, hi_len))
    for a, dim, lo, hi in sorted(ranked, reverse=True):
        print(f"| {dim} | {a:.3f} | [{lo:.3f}, {hi:.3f}] |")

    # Paired bootstrap on the AUC DIFFERENCE — the honest test of whether the
    # composite adds anything over the trivial heuristic. Resample rows once and
    # compute both AUCs on the same resample, so the CI accounts for correlation.
    def auc_diff(sample: list[dict]) -> float:
        labs = [r["correct"] for r in sample]
        return auc([r["quality_score"] for r in sample], labs) - auc(
            [-r["thinking_word_count"] for r in sample], labs
        )

    d = auc_diff(rows)
    lo, hi = bootstrap_ci(auc_diff, rows, rnd)
    verdict = (
        "**inside the noise — the composite does not beat length on this corpus**"
        if lo <= 0 <= hi
        else "**outside the noise**"
    )
    print(f"\n**composite AUC − length AUC = {d:+.3f}** (95% CI [{lo:+.3f}, {hi:+.3f}]) — {verdict}.\n")

    if args.paired:
        paired_report(rows)


if __name__ == "__main__":
    main()
