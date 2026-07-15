#!/usr/bin/env python3
"""Is there a weighting of the dimensions that transfers across models? (issue #31)

The hand-set composite leans on signals that are DeepSeek-specific: length alone
scores AUC 0.710 on DeepSeek and 0.540 on Gemini answering the SAME problems.
This asks whether a better weighting exists — and answers it honestly, by
**fitting on one model and evaluating on the held-out one**. A weighting that
only works because it memorised DeepSeek's verbosity cannot survive that test, so
cross-model held-out AUC is the metric that matters, not in-sample fit.

    reasonmetrics score -i s1k_paired.jsonl -o s1k_paired_scored.jsonl
    python scripts/reweight_study.py s1k_paired_scored.jsonl --labels s1k_paired.jsonl

This measures whether a fix is possible. It does NOT ship one: every corpus here
is math, so a weighting fitted on it still has to be checked for DOMAIN transfer
(code/clinical) before it can become a default. No third-party dependencies.
"""
from __future__ import annotations

import argparse
import json
import math
import random
import sys
from typing import Any, Callable

DIMS = [
    "efficiency_score",
    "language_score",
    "answer_alignment_score",
    "structural_score",
    "repetition_score",
    "overthinking_score",
    "verification_score",
    "length_score",
]
SEED = 0


def load_jsonl(path: str) -> list[dict[str, Any]]:
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def auc(scores: list[float], labels: list[bool]) -> float:
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
        for k in range(i, j + 1):
            ranks[order[k]] = (i + j) / 2.0 + 1.0
        i = j + 1
    rank_sum_pos = sum(r for r, lab in zip(ranks, labels) if lab)
    return (rank_sum_pos - pos * (pos + 1) / 2.0) / (pos * neg)


def standardize_params(rows: list[dict]) -> dict[str, tuple[float, float]]:
    params = {}
    for d in DIMS:
        vals = [r[d] for r in rows]
        mean = sum(vals) / len(vals)
        var = sum((v - mean) ** 2 for v in vals) / len(vals)
        params[d] = (mean, math.sqrt(var) or 1.0)
    return params


def featurize(row: dict, params: dict[str, tuple[float, float]]) -> list[float]:
    return [(row[d] - params[d][0]) / params[d][1] for d in DIMS]


def fit_logreg(
    X: list[list[float]], y: list[int], l2: float = 1.0, lr: float = 0.3, iters: int = 2000
) -> tuple[list[float], float]:
    """Plain full-batch gradient descent on standardized features. 8 features,
    ~940 points, so no need for anything fancier."""
    n, m = len(X), len(X[0])
    w = [0.0] * m
    b = 0.0
    for _ in range(iters):
        gw = [0.0] * m
        gb = 0.0
        for xi, yi in zip(X, y):
            z = b + sum(wj * xij for wj, xij in zip(w, xi))
            p = 1.0 / (1.0 + math.exp(-max(-35.0, min(35.0, z))))
            err = p - yi
            for j in range(m):
                gw[j] += err * xi[j]
            gb += err
        for j in range(m):
            w[j] -= lr * (gw[j] / n + l2 * w[j] / n)
        b -= lr * gb / n
    return w, b


def lr_score(row: dict, w: list[float], b: float, params: dict) -> float:
    x = featurize(row, params)
    return b + sum(wj * xj for wj, xj in zip(w, x))


def held_out_pair(a: list[dict], b: list[dict], get: Callable) -> tuple[float, float]:
    """AUC of a scoring function on each model's traces."""
    return (
        auc([get(r) for r in a], [r["correct"] for r in a]),
        auc([get(r) for r in b], [r["correct"] for r in b]),
    )


def paired_bootstrap_diff(
    test_rows: list[dict], f1: Callable, f2: Callable, rnd: random.Random, n_boot=1000
) -> tuple[float, float]:
    """95% CI of AUC(f1) - AUC(f2) on the same resampled test set."""
    n = len(test_rows)
    diffs = []
    for _ in range(n_boot):
        s = [test_rows[rnd.randrange(n)] for _ in range(n)]
        labs = [r["correct"] for r in s]
        d = auc([f1(r) for r in s], labs) - auc([f2(r) for r in s], labs)
        if d == d:
            diffs.append(d)
    diffs.sort()
    return diffs[int(0.025 * len(diffs))], diffs[int(0.975 * len(diffs)) - 1]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("scored")
    ap.add_argument("--labels", required=True)
    ap.add_argument(
        "--group-field",
        default="model",
        help="label field that splits the two groups to fit/evaluate across. "
        "Default `model` (cross-model, the original #31 test); set `domain` to "
        "fit on one domain and evaluate on the other (cross-DOMAIN transfer).",
    )
    args = ap.parse_args()

    gf = args.group_field
    labels = {r["id"]: r for r in load_jsonl(args.labels) if "correct" in r}
    rows = []
    for r in load_jsonl(args.scored):
        lab = labels.get(r["id"])
        if lab and gf in lab:
            r["correct"] = bool(lab["correct"])
            r["group"] = str(lab[gf])
            rows.append(r)
    if not rows:
        sys.exit(f"no rows joined — need labels with `correct` and `{gf}`")

    groups = sorted({r["group"] for r in rows})
    if len(groups) != 2:
        sys.exit(f"need exactly 2 {gf} groups, got {groups}")
    m_a, m_b = groups
    A = [r for r in rows if r["group"] == m_a]
    B = [r for r in rows if r["group"] == m_b]

    print(f"# Reweighting study: does a fix for #31 exist? (n={len(A)}+{len(B)})\n")
    same = " (same problems)" if gf == "model" else ""
    print(f"Split by `{gf}`: **{m_a}** (n={len(A)}) and **{m_b}** (n={len(B)}){same}.")
    print("Every weighting is judged by AUC on **each** group. The current composite's")
    print("problem is the *spread* between the two columns, not either number alone.\n")

    # --- fixed reference scorers ---
    scorers: list[tuple[str, Callable]] = [
        ("current composite (hand-set)", lambda r: r["raw_score"]),
        ("verification_score alone", lambda r: r["verification_score"]),
        ("length alone (shorter=better)", lambda r: -r["thinking_word_count"]),
    ]

    print("| weighting | " + f"{m_a} AUC | {m_b} AUC | spread |")
    print("|---|---|---|---|")
    for name, get in scorers:
        aa, ab = held_out_pair(A, B, get)
        print(f"| {name} | {aa:.3f} | {ab:.3f} | {abs(aa - ab):.3f} |")

    # --- learned weights, fit on one group, evaluated on the OTHER ---
    # The honest transfer test: a weighting that only works via one group's
    # idiosyncrasies (DeepSeek's verbosity cross-model; math conventions
    # cross-domain) cannot score well on the held-out group.
    learned_rows = []
    fitted = {}
    for train, test, tn in ((A, B, m_a), (B, A, m_b)):
        params = standardize_params(train)
        w, b = fit_logreg([featurize(r, params) for r in train], [int(r["correct"]) for r in train])
        fitted[tn] = (w, params)
        get = lambda r, w=w, b=b, p=params: lr_score(r, w, b, p)
        test_auc = auc([get(r) for r in test], [r["correct"] for r in test])
        learned_rows.append((tn, test_auc, get, test))

    print(f"\n## Learned weighting — fit on one {gf}, scored on the held-out one\n")
    print("| trained on | evaluated on (held out) | AUC |")
    print("|---|---|---|")
    for tn, test_auc, _, test in learned_rows:
        other = m_b if tn == m_a else m_a
        print(f"| {tn} | {other} | **{test_auc:.3f}** |")

    # Does the learned weighting beat the current composite on the held-out group?
    rnd = random.Random(SEED)
    print(f"\n## Learned vs current composite, on the held-out {gf}\n")
    print(f"| held-out {gf} | current | learned | Δ (learned − current) 95% CI |")
    print("|---|---|---|---|")
    beats = 0
    for tn, test_auc, get, test in learned_rows:
        other = m_b if tn == m_a else m_a
        cur = auc([r["raw_score"] for r in test], [r["correct"] for r in test])
        lo, hi = paired_bootstrap_diff(test, get, lambda r: r["raw_score"], rnd)
        sig = "**" if lo > 0 else ""
        if lo > 0:
            beats += 1
        print(f"| {other} | {cur:.3f} | {test_auc:.3f} | {sig}[{lo:+.3f}, {hi:+.3f}]{sig} |")

    # --- the prescription: what did it learn? ---
    print("\n## What the fix looks like (standardized weights, averaged over both fits)\n")
    print("Positive = higher score → more likely correct. This is the reweighting #31 asks for.\n")
    print("| dimension | weight | direction |")
    print("|---|---|---|")
    avg_w = {d: 0.0 for d in DIMS}
    for tn, (w, _) in fitted.items():
        for d, wj in zip(DIMS, w):
            avg_w[d] += wj / len(fitted)
    for d, wj in sorted(avg_w.items(), key=lambda kv: -abs(kv[1])):
        arrow = "↑ rewards" if wj > 0 else "↓ penalises"
        print(f"| {d} | {wj:+.3f} | {arrow} |")

    print("\n---\n")
    axis = "model-transfer" if gf == "model" else f"{gf}-transfer"
    if beats == 2:
        print(f"**A transferable fix exists.** The learned weighting beats the hand-set "
              f"composite on BOTH held-out {gf}s (CI excludes zero), which it could not "
              f"do by memorising one {gf}. The {axis} half of #31 is solvable with this data.")
    elif beats == 1:
        print(f"**Partial.** The learned weighting beats the current composite on one "
              f"held-out {gf} but not decisively on both. A fix likely exists but the "
              f"evidence is not yet clean.")
    else:
        print(f"**No clean win.** The learned weighting does not beat the hand-set "
              f"composite out-of-{gf}. Either the current weights are already near the "
              f"achievable frontier, or no single weighting transfers across {gf}s here.")
    if gf == "model":
        print("\n**Caveat that still stands:** every trace here is math. This settles "
              "model transfer, not DOMAIN transfer — a weighting fitted here must still be "
              "checked on code/clinical ground truth before it becomes a default.")
    else:
        print(f"\n**Caveat:** the '{m_a}' and '{m_b}' corpora are each one dataset "
              f"(s1K math; SYNTHETIC-1 code, mostly output-prediction), so this is a "
              f"two-point read on {gf} transfer, not the whole space.")


if __name__ == "__main__":
    main()
