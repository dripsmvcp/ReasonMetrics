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
