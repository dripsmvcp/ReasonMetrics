#!/usr/bin/env python3
"""Curate a reasoning-trace dataset with ReasonMetrics: HF -> score -> filter -> JSONL.

This is the notebook's source of truth. `examples/curate_a_reasoning_dataset.ipynb`
is generated from it (see examples/README.md), so the code in the notebook is code
that has actually run.

    pip install reasonmetrics datasets polars
    python examples/curate_a_reasoning_dataset.py
"""

# %% [markdown]
# # Curating a reasoning dataset in five minutes
#
# Reasoning traces are expensive to generate and uneven in quality. Some are tight
# and verified; some ramble for 4,000 words and never answer. If you are fine-tuning
# on them, the bad ones cost you twice — once to generate, once to train on.
#
# ReasonMetrics scores a trace's **structure** — verification, repetition, restarts,
# length ratios, language mixing — at roughly 10k traces/sec on a laptop, with no
# GPU and no API calls. This walks the whole loop: load a real dataset, score it,
# filter it, and check that the filter did something real.
#
# **Does the filter actually work?** Measured on 938 traces with objective
# correct/incorrect labels (symbolic answer verification, no LLM judge in the loop):
#
# | | traces that reach the correct answer |
# |---|---|
# | unfiltered | 48.0% |
# | **kept by `min_score >= 70`** | **69.0%** |
# | dropped by it | 42.4% |
#
# The composite reaches AUC 0.714 at predicting whether a trace arrives at the right
# answer, and it holds up when you control for problem difficulty. It is not a
# semantic oracle — see the honesty section at the bottom, which is where the
# interesting caveats live.

# %%
import json

import polars as pl
import reasonmetrics as rm
from datasets import load_dataset

print("reasonmetrics", rm.__version__)

# %% [markdown]
# ## 1. Load real traces
#
# Any dataset works; ReasonMetrics wants `{id, problem, thinking, answer}`. Here we
# use s1K-1.1, whose `deepseek_thinking_trajectory` column holds R1's real reasoning.
#
# Be careful which column you take. s1K also ships `solution` — the *human*
# ground-truth answer. Feeding that in as `thinking` scores the answer key instead
# of the model's reasoning, which silently poisons everything downstream. (We shipped
# that bug once; see `scripts/convert_dataset.py`.)

# %%
ds = load_dataset("simplescaling/s1K-1.1", split="train")

records = [
    {
        "id": f"s1k_{i}",
        "problem": row["question"] or "",
        "thinking": row["deepseek_thinking_trajectory"] or "",  # the MODEL's reasoning
        "answer": row["deepseek_attempt"] or "",
    }
    for i, row in enumerate(ds)
]
records = [r for r in records if r["thinking"] and r["answer"]]
print(f"{len(records)} traces")

# %% [markdown]
# ## 2. Score them
#
# `score_many` releases the GIL and scores in parallel across cores.

# %%
scored = rm.score_many(records)

df = pl.DataFrame(
    [
        {
            "id": s["scored"]["id"],
            "quality": s["scored"]["quality_score"],
            "raw": s["scored"]["raw_score"],
            "verification": s["scored"]["verification_score"],
            "repetition": s["scored"]["repetition_score"],
            "restarts": s["scored"]["restart_count"],
            "words": s["scored"]["thinking_word_count"],
        }
        for s in scored
    ]
)
print(df.select("quality", "verification", "words", "restarts").describe())

# %% [markdown]
# ## 3. Read the score correctly
#
# **`quality_score` is a percentile, not a grade.** It says "this trace out-reasons
# N% of real reasoning traces", measured against a reference corpus of 2,517 of them.
# So a score of 12 does not mean "12% good" — it means 88% of real traces are better.
#
# This matters because the raw composite (`raw_score`, still reported) is crushed into
# the top of its range: **99.9% of real traces score above 70 on it**. An absolute cut
# on the raw scale keeps everything, which is exactly the trap the percentile scale
# exists to avoid.

# %%
print(df.select(
    pl.col("raw").quantile(0.5).alias("raw_median"),
    (pl.col("raw") >= 70).mean().alias("raw_frac_above_70"),      # ~0.999 — useless
    pl.col("quality").quantile(0.5).alias("quality_median"),
    (pl.col("quality") >= 70).mean().alias("quality_frac_above_70"),  # ~0.2 — selective
))

# %% [markdown]
# One caveat about *this* demo specifically, since it flatters us: s1K is one of the
# three corpora the reference distribution was fitted on, so of course its scores
# spread neatly across the range. Your data will not be so obliging.
#
# The percentile is relative to **our** reference corpus, which is long-form and
# math-heavy (median 2,578 words). Score a corpus of short chat traces and everything
# will land near zero — correctly, in the sense that they contain little reasoning,
# but not usefully, because they will not be separated from *each other*. When your
# data is far from the reference distribution, rank within your own corpus
# (`top_percent` below) instead of trusting the absolute number.

# %% [markdown]
# ## 4. Filter
#
# Two ways, and they answer different questions.

# %%
# (a) Absolute: keep traces better than 70% of real reasoning traces.
kept = df.filter(pl.col("quality") >= 70)

# (b) Size-exact: keep the best 30% of THIS dataset, whatever it contains. Use this
#     when the output size has to be fixed — e.g. comparing a filtered training run
#     against a random-drop control of equal size.
cutoff = df.select(pl.col("quality").quantile(0.70)).item()
top30 = df.filter(pl.col("quality") >= cutoff)

print(f"absolute  (quality >= 70): {len(kept):5d} / {len(df)}  ({len(kept)/len(df):.1%})")
print(f"top 30%   (quality >= {cutoff:.1f}): {len(top30):5d} / {len(df)}  ({len(top30)/len(df):.1%})")

# %% [markdown]
# ## 5. Look at what you dropped
#
# Always do this. A filter you have not inspected is a filter you do not understand.

# %%
by_id = {r["id"]: r for r in records}
worst = df.sort("quality").head(3)

for row in worst.iter_rows(named=True):
    trace = by_id[row["id"]]
    print(f"\n--- {row['id']}: quality {row['quality']:.1f} "
          f"({row['words']} words, {row['restarts']} restarts) ---")
    print(trace["thinking"][:280].replace("\n", " ") + " ...")

# %% [markdown]
# ## 6. Save the curated set

# %%
with open("curated.jsonl", "w", encoding="utf-8") as f:
    for tid in kept["id"]:
        f.write(json.dumps(by_id[tid], ensure_ascii=False) + "\n")

print(f"wrote curated.jsonl: {len(kept)} traces")

# %% [markdown]
# ## What this does not tell you
#
# The scores are structural heuristics. They are a lens, not ground truth, and the
# honest limits are worth more to you than the headline number:
#
# - **A confident, tidy, well-verified trace that is completely wrong scores well.**
#   Structure is not semantics. If correctness matters, pair this with an LLM judge
#   (`scripts/llm_judge.py`) or with execution/symbolic verification.
# - **Each individual scorer is gameable**, and we ship adversarial fixtures proving
#   it — see `docs/LIMITATIONS.md`. Filtering removes the worst traces; it does not
#   certify the survivors.
# - **Much of the composite's power on DeepSeek traces is "shorter is better", and
#   that does not transfer.** On Gemini traces answering the *same* problems, the
#   length signal falls from AUC 0.710 to 0.540. `verification_score` is the only
#   dimension that holds up across both models.
# - **Filtering preferentially drops hard problems.** Harder problem → longer, messier
#   trace → lower score. If your training set needs hard examples, filter *within*
#   difficulty strata rather than globally, or you will quietly curate them away.
#
# Full method, controls, and a confound we caught in our own analysis:
# [docs/CALIBRATION.md](../docs/CALIBRATION.md).
