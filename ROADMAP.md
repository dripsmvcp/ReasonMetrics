# Roadmap

Where ReasonMetrics is headed. Sequenced by dependency, not dated — each stage
ships when its quality gate passes, and every public number stays reproducible
from a committed command line.

## Shipped

- [x] Nine-dimension scoring engine (Rust, wasm-safe core shared by every surface)
- [x] Batch CLI: score / filter / report / stats over JSONL and Parquet
- [x] Client-side web analyzer: anatomy view, live Ollama streaming, share links, gallery
- [x] Python bindings: `score`, parallel `score_many`, `annotate`, config overrides
- [x] Model-family registry: fixture-gated TOML entries (`reasonmetrics models`)
- [x] Adversarial limitations suite: documented, CI-pinned failure modes — [docs/LIMITATIONS.md](docs/LIMITATIONS.md)
- [x] Calibration study: per-dimension rank correlation against LLM-judge labels,
  including the dimensions that came out **badly** — [docs/CALIBRATION.md](docs/CALIBRATION.md).
  This is now the evidence bar any scorer or weight change has to clear.
- [x] Percentile score scale: the raw composite is mapped to its percentile
  against a 2,517-trace reference corpus, so a "top 20%" filter actually cuts
  and the score means the same thing across models (fixed #30) —
  [docs/CALIBRATION.md](docs/CALIBRATION.md).
- [x] Benchmark harness: `reasonmetrics bench` runs a fixed, content-hashed task set
  against any OpenAI-compatible endpoint and scores each returned trace into a
  committed result JSON (quality, accuracy, tokens- and cost-per-correct), with
  the exact command and task-set hash embedded so a leaderboard row is a
  reviewable PR. Feature-gated so the curation binary stays lean —
  [docs/BENCH.md](docs/BENCH.md).
- [x] Benchmark depth: a 100-task `overthinking-v2` set (deterministic generator),
  multi-sample **pass@k**, and cross-run **leaderboard assembly**
  (`reasonmetrics leaderboard`) that dedups and groups committed result JSONs —
  [docs/BENCH.md](docs/BENCH.md).
- [x] Leaderboard site + submission shape: `--site` renders a self-contained
  static [leaderboard/](leaderboard) page; a leaderboard entry is a PR adding a
  result JSON, gated by `--strict` validation in CI (a bundled task set must
  carry its frozen sha256) — [results/README.md](results/README.md).
- [x] Tiered LLM judge: opt-in, escalates only the uncertain heuristic band
  (default 40–70) to a judge model; advisory, never blended into the score.
- [x] [SPEC.md](SPEC.md) v1.0.0 — trace schema and scoring semantics, frozen and
  semver'd, with a CI guard tying the default weights to the spec version.

## Now

- [ ] **Filtering validation** — does score-filtering actually improve a fine-tune?
  Pre-registered three-arm experiment (unfiltered / score-filtered / random-drop),
  published whichever way it comes out → `docs/VALIDATION.md`.
- [ ] **Registry growth** — more model families and non-English restart/verification
  lexicons. These are the best first contributions: one TOML + one fixture
  (see the `good first issue` label and CONTRIBUTING.md).
- [x] **Prebuilt CLI binaries** — release workflow builds and attaches native
  binaries on `cli-v*` tags (`.github/workflows/release.yml`, `--features bench`).
- [ ] **PyPI release** of the Python package — wheel-build workflow is ready
  (`.github/workflows/wheels.yml`); the publish itself is gated on account setup.

## Next

- [ ] **Populate the leaderboard** — the harness, the site, and the submission
  gate all ship (above); what remains is operational: run notable model releases
  on `overthinking-v2`, commit the result JSONs, and keep the page current. The
  machinery is done; the entries accrue over time.
- [ ] **Larger, licensed task sets** — grow beyond `overthinking-v2` (100 tasks)
  to bigger, more varied reasoning sets, and richer answer extraction so verbose
  answers grade correctly. Design context:
  [docs/superpowers/specs/2026-07-15-reasonmetrics-bench-design.md](docs/superpowers/specs/2026-07-15-reasonmetrics-bench-design.md).

## Later

- [ ] **Thinking-budget enforcement** — a proxy that applies reasoning budgets at
  inference time (streaming incremental scoring is the groundwork).
- [ ] **Distillation tooling** — filter teacher traces → fine-tune → before/after
  report card.
- [ ] **Deeper scorers** — semantic near-duplicate detection, calibration-fitted
  weights, per-domain profiles (the cross-domain study in [docs/CALIBRATION.md](docs/CALIBRATION.md)
  is the evidence groundwork for per-domain profiles).

## Principles

- Reproducible or unpublished.
- Private by architecture — no server, no accounts, no telemetry, ever.
- Scores are a lens, not ground truth: failure modes stay documented and
  CI-pinned in [docs/LIMITATIONS.md](docs/LIMITATIONS.md).
- Scorer and weight changes require calibration evidence; registry changes
  require fixtures.
