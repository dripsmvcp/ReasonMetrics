# Design — `reasonmetrics bench`

Status: draft for review · Date: 2026-07-15 · Roadmap stage: **Next**

## Goal

Add a `reasonmetrics bench` subcommand that runs a **fixed, version-pinned task
set** against **any OpenAI-compatible `/v1/chat/completions` endpoint**, scores
each returned reasoning trace with the existing core engine, and emits a
**commit-friendly result JSON** plus a printed leaderboard. This is the
roadmap's "Next" item, built as specced there — not the producer/distillation
work, which stays in "Later".

## Roadmap alignment

Maps 1:1 to `ROADMAP.md` → **Next**:

- `reasonmetrics bench` — fixed task set, any OpenAI-compatible endpoint. ✓
- Overthinking leaderboard metrics — reasoning-quality score, accuracy, tokens
  per correct answer, cost per 1,000 correct answers. ✓
- Reproducible-or-unpublished — every run's JSON embeds the exact command +
  task-set hash + tool version, so a leaderboard entry is a reviewable PR. ✓
- Private-by-architecture — endpoint is user-supplied (localhost by default);
  no telemetry; API keys never touch a flag or the committed command. ✓

Deliberate addition, now recorded in the roadmap: a **reasoning-quality
column** alongside the four named metrics. It is the tool's core differentiator;
a reasoning-quality benchmark without it has no reason to exist.

## Scope

**In scope (v1):** the subcommand, one bundled task set, the HTTP endpoint
client, correctness + quality scoring reuse, the aggregator, the result JSON,
and a printed/markdown/HTML leaderboard for a single run.

**Out of scope (v1, deferred):** multi-sample / pass@k (`--samples` > 1);
cross-run public-leaderboard assembly (combining many committed JSONs into one
table — a thin follow-on); the CLI escape-hatch backend (dropped — endpoint is
OpenAI-compatible only); `SPEC.md` freezing of the result schema (tracked
separately in the roadmap).

## Architecture

Six units. The **endpoint client** is the only unit that touches the network;
the **scorer adapter** is the only unit that touches the evaluator. Everything
between them is pure data, so the whole loop is testable against a mock model.

```
 benchset (fixed, committed, hashed) ─┐  { id, problem, expected_answer }
                                      ▼
   --endpoint,--model,--task-set ► Runner ──► POST /v1/chat/completions ──► model
                                  (rayon cap) ◄── completion text ─────────────┘
                                      │ TraceRecord (extract_thinking)
                                      ▼
                              Scorer + correctness
                              quality = core::score_one
                              correct = normalized match vs expected_answer
                                      │ per-task rows
                                      ▼
                                 Aggregator ──► metrics
                                      │
                                      ▼
        results/<taskset>-<model>-<hash>.json  +  leaderboard (stdout/md/html)
```

1. **Task set** — `benchsets/<name>-vN.jsonl`, committed in-repo, each row
   `{id, problem, expected_answer}`. Selected via `--task-set <name>`.
   Content-hashed (sha256 over the canonical bytes); the hash is recorded in
   every result so a leaderboard row is bound to an exact set.
2. **Endpoint client** — one blocking `ureq` client that POSTs to
   `{base_url}/chat/completions`. Behind a `trait Model { fn complete(&self,
   prompt: &str) -> Result<Completion> }` so tests substitute a mock. Reads the
   API key from the env var named by `--api-key-env`. Returns completion text +
   `usage` token counts when the endpoint provides them.
3. **Runner** — for each task, calls `Model::complete` under a `rayon`
   concurrency cap (`--concurrency`, default 8), then builds a `TraceRecord`
   from the completion using the existing `extract_thinking` to split
   `<think>`/answer.
4. **Scorer + correctness** — `reasonmetrics-core::score_one` for the quality
   score; correctness via `answer_matches(extracted, expected)` (normalization
   mirroring the accuracy-efficiency scorer). Only unit touching the evaluator.
5. **Aggregator** — computes the leaderboard metrics (below) over the per-task
   rows.
6. **Result writer** — serializes one result JSON per run and renders the
   leaderboard table (stdout by default; markdown/HTML via the existing
   `minijinja`).

## CLI interface

The invocation *is* the reproducible artifact:

```bash
reasonmetrics bench \
  --endpoint http://localhost:8000/v1 \   # base URL (required)
  --model deepseek-r1:8b \                # model id sent to the endpoint (required)
  --task-set overthinking-v1 \            # bundled, hashed set (default: overthinking-v1)
  --temperature 0 \                       # default 0 → deterministic as far as the endpoint allows
  --max-tokens 8192 \                     # default 8192
  --concurrency 8 \                       # default 8; rayon cap
  --cost-per-mtok 0.40 \                  # optional; enables the cost column
  --api-key-env OPENAI_API_KEY \          # env var NAME; key never on the command line
  --out results/overthinking-v1-deepseek.json \  # default: results/<taskset>-<model>-<hash>.json
  --format table                          # table | md | html | json
```

Flags also readable from `--config` for the quality scorer's weights/thresholds
(same file the other subcommands take).

## Result JSON schema

```json
{
  "schema_version": "1",
  "tool_version": "0.2.0",
  "generated_at": 1752570600,
  "command": "reasonmetrics bench --endpoint … --model deepseek-r1:8b --task-set overthinking-v1 --temperature 0",
  "task_set": { "name": "overthinking-v1", "sha256": "9c…", "n": 200 },
  "model": "deepseek-r1:8b",
  "endpoint_host": "localhost:8000",
  "sampling": { "temperature": 0.0, "max_tokens": 8192, "samples": 1 },
  "tokens_estimated": false,
  "metrics": {
    "n_attempted": 200, "n_scored": 198, "n_errored": 2,
    "accuracy": 0.68, "mean_quality": 71.2,
    "tokens_per_correct": 1240.5, "cost_per_1k_correct": 3.72
  },
  "results": [
    { "id": "001", "correct": true, "quality": 74.1, "tokens": 1180, "error": null }
  ]
}
```

- `endpoint_host` stores **host only** — never a URL that could carry a key.
- `n_attempted` / `n_scored` / `n_errored` are all reported, so errored tasks are
  visible rather than silently dropped from the accuracy denominator.
- `tokens_estimated` is `true` when the endpoint returned no `usage` and token
  counts fell back to `estimated_token_count`.
- `generated_at` is Unix epoch seconds, wall-clock metadata only; it is
  **excluded** from the reproducibility comparison (re-running the same command
  matches on `metrics` and `task_set.sha256`, not on this timestamp).

## Metric definitions

Precise, because "tokens per correct answer" is ambiguous otherwise:

- **accuracy** = `n_correct / n_scored`.
- **mean_quality** = mean of the core quality score over scored traces.
- **tokens_per_correct** = `total_completion_tokens(all attempted) / n_correct`
  — the token cost of *buying one correct answer*, so wasted tokens on wrong
  answers count against it. Undefined (`null`) when `n_correct == 0`.
- **cost_per_1k_correct** = `(total_completion_tokens / 1e6 * cost_per_mtok) /
  n_correct * 1000`. `null` when `--cost-per-mtok` is omitted or `n_correct == 0`.

Token counts come from the endpoint's `usage.completion_tokens` when present,
else from `estimated_token_count` (flagged via `tokens_estimated`).

## Correctness semantics

`answer_matches(extracted, expected)`:

1. Extract the model's final answer (reuse the answer-extraction path the
   accuracy-efficiency scorer uses; fall back to the text after the last
   `</think>`).
2. Normalize: trim, lowercase, strip surrounding LaTeX/`$`/`\boxed{}`, collapse
   whitespace, drop trailing punctuation.
3. Match: exact normalized-string equality **or** numeric equality within a
   small relative tolerance when both sides parse as numbers.

This is a **lens, not a proof** — consistent with the project principle. A task
whose answer cannot be extracted counts as **incorrect** (not errored) and is
flagged. Correctness limits are documented alongside the existing LIMITATIONS.

## Error handling & reproducibility edge cases

- **Endpoint failure** (timeout / non-2xx / malformed body): bounded retries
  with backoff (default 2 retries); on final failure the task's `error` is set,
  it is counted in `n_errored`, excluded from `n_scored`, and the run continues.
  Partial results are always written.
- **No `usage` in response**: fall back to estimated tokens; set
  `tokens_estimated: true`.
- **Remote model drift**: temperature 0 reduces variance but does not guarantee
  bit-identical re-runs, and hosted models change over time. The committed JSON
  is the record of what happened; the embedded command documents how to re-run.
  This is stated in the leaderboard docs so entries are not over-trusted.
- **Empty / unparseable completion**: scored as a trace of whatever text
  returned (quality will be low); counts as incorrect if no answer extracts.

## Dependencies & feature-gating

- New dependencies, **both** under an opt-in `bench` cargo feature on
  `reasonmetrics-cli`: **`ureq`** (blocking HTTP client) and **`sha2`**
  (task-set content hash). Default `cargo install reasonmetrics-cli` stays lean
  and TLS-free for curation users; prebuilt release binaries build with
  `--features bench`. `generated_at` uses `std::time` (no dependency).
- Reuses existing deps: `rayon` (concurrency), `indicatif` (progress),
  `minijinja` (md/html leaderboard), `serde`/`serde_json`, `anyhow`, `clap`.
- No new dependency in `reasonmetrics-core`.

## Testing

- **Mock `Model`** returning canned completions → unit-tests Runner +
  Aggregator + Writer end-to-end with no network.
- **Correctness matcher** unit tests: numeric equality, LaTeX/`\boxed{}`
  stripping, whitespace, and true negatives (confident wrong answer).
- **Metric math** unit tests: `tokens_per_correct` / `cost_per_1k_correct`,
  including the `n_correct == 0` → `null` paths.
- **Result-JSON golden test**: serialize a fixed mock run and diff against a
  committed fixture, with volatile fields (timestamps) normalized.
- **CLI integration** (`assert_cmd`): `reasonmetrics bench` against an in-test
  `tiny_http` stub endpoint (dev-dependency) that serves canned completions —
  exercises flag parsing, the real HTTP path, and file output.
- **Feature isolation**: `cargo build` (no `bench` feature) still compiles and
  the binary carries no `ureq`/TLS; `cargo test --features bench` covers the
  above.

## Definition of done (the promised deliverable)

All must hold:

1. `cargo build --release --features bench` produces a `reasonmetrics` binary
   whose `bench` subcommand is present; a plain `cargo build` compiles without
   `ureq` and without the subcommand.
2. Against a real local endpoint (Ollama's OpenAI shim or a local vLLM),
   `reasonmetrics bench --endpoint <url> --model <m> --task-set overthinking-v1`
   prints a leaderboard row **and** writes a `results/*.json` matching the schema
   above.
3. Running the *same command* twice at `--temperature 0` produces the same
   metrics up to endpoint nondeterminism, and both JSONs carry the identical
   `task_set.sha256`.
4. All tests above pass under `cargo test --features bench`; the existing suite
   still passes with default features.
5. `docs/` documents the subcommand, the metric definitions, and the
   correctness caveat.

## Open implementation details (resolve in the plan)

- **v1 task-set sourcing**: assemble `overthinking-v1.jsonl` (~100–200 problems
  with `expected_answer`) from a permissively licensed reasoning set, license
  and attribution checked before committing. Candidate: a fixed subset of an
  existing validated dataset that ships answers.
- Exact `ureq` TLS backend choice (rustls vs native-tls) for release binaries.
- Whether `--format html` reuses the existing report template or gets a small
  dedicated leaderboard template.
