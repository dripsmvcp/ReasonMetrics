# `reasonmetrics bench`

Run a fixed, version-pinned task set against any OpenAI-compatible
`/v1/chat/completions` endpoint, score each returned reasoning trace, and write
a commit-friendly result JSON plus a leaderboard.

Requires a build with the `bench` feature:

    cargo build --release --features bench

## Usage

    reasonmetrics bench \
      --endpoint http://localhost:8000/v1 \
      --model deepseek-r1:8b \
      --task-set overthinking-v1 \
      --temperature 0 \
      --cost-per-mtok 0.40 \
      --api-key-env OPENAI_API_KEY

The API key is read from the named env var, never a flag — so it never lands in
your shell history or the committed command.

## Task sets

Sets are frozen and content-hashed; the sha256 is embedded in every result so a
leaderboard row names exactly which problems produced it.

- **`overthinking-v1`** — 12 hand-authored arithmetic problems. A fast smoke set
  for wiring up an endpoint.
- **`overthinking-v2`** — 100 problems across ten categories (arithmetic,
  comparison, parity, counting, ordering, remainder, percentage). Simple enough
  that a competent model answers in a sentence, but exactly the kind of task that
  tempts weak reasoning models into long detours — which is what the quality
  score is meant to catch. This is the set to report.

`overthinking-v2` is generated deterministically by
[`scripts/gen_benchset.py`](../scripts/gen_benchset.py) (fixed seed, fixed
category order), so anyone can regenerate the `.jsonl` byte-for-byte and confirm
the embedded hash. The problems are our own authored instances; their shape is
inspired by [LLMThinkBench](https://arxiv.org/abs/2507.04023) (MIT), not copied
from it. Every answer is a single integer or word so grading needs no parser.

## Metrics

- **quality** — mean ReasonMetrics composite (percentile vs real traces).
- **accuracy** — `n_correct / n_scored`.
- **tokens/correct** — total completion tokens over all attempted tasks divided
  by the number correct (so wasted tokens on wrong answers count against it).
- **cost/1k correct** — `(total_tokens/1e6 * cost_per_mtok) / n_correct * 1000`;
  shown only with `--cost-per-mtok`.

`n_attempted`, `n_scored`, and `n_errored` are all reported: an errored task is
never silently dropped from the denominator.

## Reproducibility & caveats

Each run writes a result JSON embedding the exact command, the task-set sha256,
and the tool version — so a leaderboard entry is a reviewable PR. `--temperature
0` reduces variance but does not guarantee bit-identical re-runs, and hosted
models change over time; the committed JSON is the record of what happened.

Correctness is a normalized answer match (numeric or string), not a proof — a
confident, tidy, wrong trace still scores its quality. See docs/LIMITATIONS.md.
