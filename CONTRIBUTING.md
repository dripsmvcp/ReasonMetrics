# Contributing to ReasonMetrics

Thanks for contributing! ReasonMetrics uses a zoned contribution model: **maintainers own logic,
contributors own data and edges.** This keeps merges fast and objective — most accepted PRs are
data-plus-fixture changes that CI can verify mechanically.

## Ownership zones

### 🟢 Green — open to PRs, no prior discussion needed

| Path | Contributions |
|------|--------------|
| `registry/` | Model families: think-tag formats, field aliases, cost tables, lexicon phrases (one TOML + one fixture per PR) |
| `scripts/` | Dataset converters, LLM-judge provider additions |
| `web/public/gallery/` | Gallery traces (must include honest provenance; trimmed fixtures marked `"curated": true`) |
| `crates/reasonmetrics-cli/tests/fixtures/`, `web/src/**/*.test.*` | Test fixtures and coverage — miscalibration repro cases especially welcome |
| `docs/` (except `launch.md`) | Guides, examples, corrections |

### 🟡 Yellow — open an issue first, PR after discussion

| Path | Contributions |
|------|--------------|
| `web/src/components/`, `web/src/hooks/` | UI polish, accessibility, bug fixes (architecture stays with maintainers) |
| `crates/reasonmetrics-cli/src/output/`, `src/parser.rs` | New output formats, input handling |
| Any dependency addition or bump | Supply-chain review required |

### 🔴 Red — maintainer-only (PRs closed, converted to issues)

`crates/reasonmetrics-core/` · `crates/reasonmetrics-wasm/src/` · `.github/` · workspace `Cargo.toml`
· `reasonmetrics.toml` (it carries `[scoring.weights]`) · `docs/launch.md` · releases and tags.

The scoring semantics are the product; external scorer or weight changes are never merged directly.
Found a scoring problem? **File a miscalibration issue** with a repro trace — those reports are gold
and typically become fixtures and fixes quickly. The `path-guard` CI check enforces these zones
automatically on external PRs.

## Add a model family in 30 minutes

The registry is the main contribution surface: one TOML file + one fixture, no Rust required.

1. Copy the worked example: `registry/deepseek-r1.toml` → `registry/<your-model-id>.toml`.
   The file name (minus `.toml`) must equal the `id` field.
2. Fill in `display_name`, `notes`, and the `[extraction]` section — the `[start, end]` think-tag
   pairs and/or the top-level JSON fields that carry the reasoning.
   ⚠️ TOML gotcha: keep the top-level keys (`id`, `display_name`, `notes`, `fixture`) **above** the
   first `[section]` header, or they'll be parsed as part of that section.
3. Add `registry/fixtures/<your-model-id>.json` with a **real, short** output sample:
   `input` + `expected_thinking` (for tag extraction) and/or `input_json` + `expected_field_thinking`
   (for field extraction). At least one pair is required.
4. Optional: a `[cost]` table (must cite `source` + date) and `[heuristics] tokens_per_char`.
   Lexicon phrases (`[lexicon.<lang>]`) are welcome as data — they don't change scoring yet
   (that switch is gated on calibration evidence).
5. Run `cargo test -p reasonmetrics-core registry` — the harness verifies your entry parses, the
   fixture exists, and your extraction config actually extracts it. If that passes, CI will too.
6. Open the PR with the "New model / lexicon" issue linked. That's it.

## Rules

1. **Every data contribution ships with a fixture CI can verify.** Registry entry → fixture trace;
   converter → sample row; gallery trace → provenance metadata. No fixture, no merge.
2. **PR size is not value.** Unsolicited refactors, mass renames, and formatting sweeps are closed
   without review.
3. **One change per PR**, linked to an existing issue.
4. Conventional commit summaries (`feat:`, `fix:`, `chore:` …); PRs are squash-merged, so only the
   PR title needs to be clean.

## Gates (all must pass)

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo check -p reasonmetrics-core --no-default-features --target wasm32-unknown-unknown
cd web && npm test
```

CI runs these plus a wasm-pack build on every PR. Run the ones covering your change locally first.

## Review

Target first response and merge decision within **48 hours**. If a PR sits longer, ping it — that's
a miss on our side, not yours.

<!-- path-guard verification: green-zone edit, safe to close unmerged -->
