## What

<!-- One-paragraph summary. Every PR should address an existing issue. -->

Closes #

## Checklist

- [ ] Does **not** modify `crates/reasonmetrics-core/`, `crates/reasonmetrics-wasm/src/`, `.github/`, the workspace `Cargo.toml`, or `docs/launch.md` (maintainer-only — see [CONTRIBUTING.md](../CONTRIBUTING.md))
- [ ] Data contributions (registry entry, lexicon, converter, gallery trace) include a fixture that CI can verify
- [ ] The gates covering this change pass locally (`cargo test --workspace` / `cd web && npm test`)
- [ ] Scoped to one change — no unrelated refactors, renames, or formatting sweeps
