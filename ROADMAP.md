# Roadmap

Where ReasonMetrics is headed. Sequenced by dependency, not dated — each stage
ships when its quality gate passes, and every public number stays reproducible
from a committed command line.

## Shipped

- Nine-dimension scoring engine (Rust, wasm-safe core shared by every surface)
- Batch CLI: score / filter / report / stats over JSONL and Parquet
- Client-side web analyzer: anatomy view, live Ollama streaming, share links, gallery
- Python bindings: `score`, parallel `score_many`, `annotate`, config overrides
- Model-family registry: fixture-gated TOML entries (`reasonmetrics models`)
- Adversarial limitations suite: documented, CI-pinned failure modes — [docs/LIMITATIONS.md](docs/LIMITATIONS.md)

## Now

- **Calibration study** — per-dimension correlation against LLM-judge labels on
  the validated datasets → `docs/CALIBRATION.md`. Also becomes the evidence bar
  for any scorer/weight change.
- **Filtering validation** — does score-filtering actually improve a fine-tune?
  Pre-registered three-arm experiment (unfiltered / score-filtered / random-drop),
  published whichever way it comes out → `docs/VALIDATION.md`.
- **Registry growth** — more model families and non-English restart/verification
  lexicons. These are the best first contributions: one TOML + one fixture
  (see the `good first issue` label and CONTRIBUTING.md).
- **PyPI release** of the Python package; prebuilt CLI binaries.

## Next

- **`reasonmetrics bench`** — a fixed task set runnable against any
  OpenAI-compatible endpoint.
- **Public overthinking leaderboard** — accuracy, tokens per correct answer, and
  cost per 1,000 correct answers for notable model releases; every entry a
  committed JSON + the exact command that produced it, so third-party
  submissions are reviewable PRs.
- **SPEC.md v1** — the trace schema and scoring semantics, frozen and semver'd,
  so other tools can implement compatibly.

## Later

- **Thinking-budget enforcement** — a proxy that applies reasoning budgets at
  inference time (streaming incremental scoring is the groundwork).
- **Distillation tooling** — filter teacher traces → fine-tune → before/after
  report card.
- **Deeper scorers** — semantic near-duplicate detection, calibration-fitted
  weights, per-domain profiles.

## Principles

- Reproducible or unpublished.
- Private by architecture — no server, no accounts, no telemetry, ever.
- Scores are a lens, not ground truth: failure modes stay documented and
  CI-pinned in [docs/LIMITATIONS.md](docs/LIMITATIONS.md).
- Scorer and weight changes require calibration evidence; registry changes
  require fixtures.
