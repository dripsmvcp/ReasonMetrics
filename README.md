<div align="center">

# ReasonMetrics

**Measure and filter reasoning quality before it costs you.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Longer reasoning traces ≠ better training data ([r = −0.59 correlation](https://arxiv.org/abs/2602.13517)).
<br>
1 million traces scored in **~90 seconds**. No GPU required.

</div>

---

## What It Is

ReasonMetrics analyzes LLM reasoning traces (`<think>...</think>`) and scores them on nine quality dimensions. One production-grade Rust scoring engine, three interfaces:

- **Web analyzer** — paste a trace (or live-stream one from local Ollama) and see its anatomy: restart loops, repetition, self-verification, token/cost meter, and a composite quality score. Runs 100 % client-side via WebAssembly — traces never leave your machine.
- **CLI** — batch-score millions of traces to JSONL/Parquet for training-data filtering and quality gating.
- **Core library** — embed the scoring engine (`reasonmetrics-core`) anywhere Rust or WASM runs.

## Why It Exists

Every lab training a reasoning model processes millions of thinking traces. The typical pipeline generates traces, verifies answers, then trains without examining **trace quality itself**:

| Quality Issue | Impact |
|---------|---------|
| Language mixing | English/Chinese switching mid-trace — confuses decoder |
| Overthinking | 27,000-token trace for "2+3=?" — wastes compute, adds noise |
| No self-verification | Model never checks work — trains poor habits |
| Answer buried mid-trace | Breaks extraction, splits supervision signal |
| Wasteful restart loops | 40 % of tokens are "wait, let me restart" |

ReasonMetrics makes reasoning quality measurable, and wasteful traces filterable.

## How It Compares

General-purpose curation pipelines ([Data-Juicer](https://github.com/datajuicer/data-juicer), [datatrove](https://github.com/huggingface/datatrove), [NeMo Curator](https://github.com/NVIDIA/NeMo-Curator)) filter text with generic operators — dedup, length, perplexity. LLM-as-judge tools score quality but cost ~$1,000 per 100K traces, run at 2–5 traces/sec, send your data to a third party, and give different scores on every run. ReasonMetrics is the only tool purpose-built for reasoning-trace anatomy:

| | ReasonMetrics | Generic pipelines | LLM-as-judge |
|---|---|---|---|
| Trace-specific dimensions (restarts, language mixing, overthinking…) | ✅ 9, research-backed | ❌ | ⚠️ prompt-dependent |
| Throughput (traces/sec) | 2,650–45,000 | high | 2–5 |
| Cost per 100K traces | $0 | $0 | ~$1,000 |
| Deterministic | ✅ | ✅ | ❌ |
| Data stays local | ✅ (even the web app) | ✅ | ❌ |
| Inspect a single trace visually | ✅ | ❌ | ❌ |
| Semantic correctness | ❌ (see [LLM judge](#llm-as-judge-optional-semantic-scoring)) | ❌ | ✅ |

Full analysis with performance evidence: [showcase/COMPARISON.md](showcase/COMPARISON.md).

## Prerequisites

- [Rust](https://rustup.rs) ≥ 1.80 (CLI and scoring engine)
- [Node.js](https://nodejs.org) ≥ 20 with npm (web analyzer)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) ≥ 0.13 (web analyzer's wasm build)

## Web Analyzer

Runs 100 % client-side in the browser (deployed to GitHub Pages on every `web-v*` tag):

- **Paste / drop** a `.jsonl`, `.json`, or `.txt` file — field names auto-map with optional manual mapping dialog
- **Anatomy view** — restarts, verification, and repetition highlighted span-by-span; shows token count, configurable cost meter ($/1M-token), and composite quality score
- **Live mode** — stream a reasoning prompt against a local [Ollama](https://ollama.com) model and watch the analysis update in real-time
- **Share** — export a PNG scorecard or generate a shareable link that packs the entire trace into the URL fragment (no server, no upload)
- **Gallery** — five reference traces from production models demonstrating rambling, restart loops, language mixing, and verified reasoning

Run it locally:

```bash
cd web
npm install
npm run build:wasm   # compiles the rust engine to wasm — first run takes a minute or two
npm run dev          # then open the URL vite prints (default http://localhost:5173)
```

## CLI

Install the batch scoring tool. Three ways, fastest first:

```bash
# 1. Prebuilt binary — no toolchain needed. Pick your platform's asset from
#    https://github.com/dripsmvcp/ReasonMetrics/releases, e.g. on Linux x86_64:
curl -L https://github.com/dripsmvcp/ReasonMetrics/releases/latest/download/reasonmetrics-x86_64-unknown-linux-gnu.tar.gz | tar xz
./reasonmetrics-x86_64-unknown-linux-gnu/reasonmetrics --help

# 2. From crates.io (needs a Rust toolchain):
cargo install reasonmetrics-cli               # installs the `reasonmetrics` binary

# 3. From source:
cargo build --release                          # first build takes a minute or two
export PATH="$PWD/target/release:$PATH"        # or invoke as ./target/release/reasonmetrics
reasonmetrics --help                           # sanity check
```

> Prebuilt binaries (Linux/macOS/Windows) are published on every `cli-v*` release
> tag. `cargo install` works once the crates are published to crates.io — both are
> ready to go and gated only on the maintainer cutting the first release.

```bash
reasonmetrics score  -i traces.jsonl -o scored.parquet   # score all traces
reasonmetrics filter -i traces.jsonl -o clean.jsonl --min-score 70    # keep traces better than 70% of real ones
reasonmetrics filter -i traces.jsonl -o clean.jsonl --top-percent 30  # keep the best 30% of THIS file
reasonmetrics report -i traces.jsonl -o report.html      # generate HTML report
reasonmetrics stats  -i traces.jsonl                     # summary statistics
reasonmetrics explain                                    # score dimension reference
reasonmetrics init-config > my-scoring.toml              # dump default weights/thresholds
```

**`quality_score` is a percentile, not a grade.** It says "this trace out-reasons N% of
real reasoning traces", measured against a reference corpus of 2,517 of them. So
`--min-score 70` keeps the top ~30%, and a trivially short trace scoring 3 is the tool
working, not failing. `--top-percent` instead keeps an exact share of your own file,
for when the output size has to be fixed. The underlying weighted average of the nine
dimensions is still reported as `raw_score`.

On 938 traces with objective correct/incorrect labels, `--min-score 70` keeps traces
that reach the right answer **69.0%** of the time, versus **42.4%** for the ones it
drops (unfiltered baseline: 48.0%). Details, controls, and the ways this can mislead
you: [docs/CALIBRATION.md](docs/CALIBRATION.md).

Every command takes `--config <file>` (default: `reasonmetrics.toml` in the current directory — the repo root ships a ready-made one). To customize scoring, edit that file in place or pass a generated one: `reasonmetrics score --config my-scoring.toml -i traces.jsonl`. (Careful: `init-config > reasonmetrics.toml` at the repo root overwrites the tracked file.)

## Python

The same engine as a Python package (built with PyO3; releases the GIL and scores batches in parallel).

> **Not on PyPI yet** — the first release is still pending, so `pip install reasonmetrics` will not find
> anything. Install from source in the meantime: pip builds the extension for you, so you need a
> [Rust toolchain](https://rustup.rs) but nothing else.

```bash
pip install "git+https://github.com/dripsmvcp/ReasonMetrics.git#subdirectory=crates/reasonmetrics-py"
```

```python
import reasonmetrics as rm

result = rm.score({"problem": "2+2?", "thinking": "<think>4. Let me verify: 2+2=4.</think>", "answer": "4", "id": "1"})
result["scored"]["quality_score"]        # percentile vs real traces (0-100)
result["scored"]["raw_score"]            # the weighted dimension average behind it
result["annotations"]                    # restart/verification/repetition spans

scored = rm.score_many(records)          # parallel batch, list of the same dicts
kept = [r for r in scored if r["scored"]["quality_score"] >= 70]   # top ~30%

rm.score(record, config={"weights": {"efficiency": 0.5}})   # reasonmetrics.toml-shaped overrides
rm.registry()                            # embedded model-family registry
```

**Worked example:** [`examples/curate_a_reasoning_dataset.ipynb`](examples/curate_a_reasoning_dataset.ipynb) — the whole loop on a real dataset (HuggingFace → score → filter → inspect what you dropped → JSONL), including how to read the score without misreading it.

## Input Format

JSONL with one trace per line:

```jsonl
{"id": "001", "problem": "Find x...", "thinking": "Let me...", "answer": "x = 3"}
```

Accepts aliases: `question`/`prompt`/`query`/`input` for problem; `reasoning`/`chain_of_thought`/`cot`/`thought` for thinking; `solution`/`response`/`output`/`result` for answer; optional `expected_answer` (`ground_truth`/`label`/`target`) enables the accuracy-efficiency scorer. Auto-detects `<think>` tags. Supports `.jsonl.gz`.

## The 9 Quality Dimensions

| # | Dimension | Weight | Detects |
|---|-----------|--------|---------|
| 1 | Efficiency | 20 % | Restart/backtracking phrases |
| 2 | Language Consistency | 12 % | Mid-trace language switching |
| 3 | Answer Alignment | 18 % | Convergence to a conclusion |
| 4 | Structural Clarity | 10 % | Step markers, paragraph breaks |
| 5 | Repetition | 15 % | Repeated paragraphs/sentences |
| 6 | Overthinking | 10 % | Trace length vs problem complexity |
| 7 | Self-Verification | 8 % | Explicit + implicit verification |
| 8 | Length Calibration | 7 % | Appropriate word-count range |
| 9 | Accuracy-Efficiency | 0 % (opt-in) | Correct-but-bloated traces — harmonic mean of accuracy and token efficiency, per [LLMThinkBench](https://arxiv.org/abs/2507.04023); needs `expected_answer` |

Scores are heuristics — a lens, not ground truth. The failure modes are documented and pinned by adversarial tests: see [docs/LIMITATIONS.md](docs/LIMITATIONS.md).

**Does the score predict anything real?** Measured against **938 traces with objective
correct/incorrect labels** (s1K-1.1, symbolic answer verification, no LLM in the loop): the
composite reaches **AUC 0.714** at predicting whether a trace arrives at the right answer, and
it holds up when you control for problem difficulty (0.632 on hard problems, 0.701 on easy) —
so it is not just an elaborate difficulty detector. `--min-score 70` keeps traces that are right
**69.0%** of the time vs **42.4%** for those it drops.

Two things we are *not* claiming. On a second model answering the same problems, the composite
weakens (AUC 0.574) and the "shorter is better" signal that carries much of it on DeepSeek
largely evaporates (0.710 → 0.540) — only `verification_score` transfers cleanly. And structural
scores say nothing about whether the reasoning is *sound*; a confident, tidy, wrong trace scores
well by design. Full tables, the controls, a confound we caught in our own analysis, and the
failure modes: [docs/CALIBRATION.md](docs/CALIBRATION.md).

## Validated Datasets

Tested against 5 open-source reasoning-trace datasets (~940K total traces):

| Dataset | Rows | Domain | Link |
|---------|------|--------|------|
| GAIR/LIMO | 817 | Math | [HuggingFace](https://huggingface.co/datasets/GAIR/LIMO) |
| simplescaling/s1K-1.1 | 1,000 | Multi | [HuggingFace](https://huggingface.co/datasets/simplescaling/s1K-1.1) |
| OpenThoughts-114k | 114,000 | Math/Code/Science | [HuggingFace](https://huggingface.co/datasets/open-thoughts/OpenThoughts-114k) |
| nvidia/OpenCodeReasoning | 735,000 | Code | [HuggingFace](https://huggingface.co/datasets/nvidia/OpenCodeReasoning) |
| medical-o1-reasoning-SFT | 90,000 | Medical | [HuggingFace](https://huggingface.co/datasets/FreedomIntelligence/medical-o1-reasoning-SFT) |

Conversion script included — see [Dataset Conversion](#dataset-conversion).

## Research Basis

Every scorer is grounded in published research:

| Scorer | Paper | Venue |
|--------|-------|-------|
| Efficiency | [Think Deep, Not Just Long](https://arxiv.org/abs/2602.13517) | arXiv, Feb 2026 |
| Overthinking | [Do NOT Think That Much for 2+3=?](https://arxiv.org/abs/2412.21187) | arXiv, Dec 2024 |
| Accuracy-Efficiency | [LLMThinkBench](https://arxiv.org/abs/2507.04023) | ACL 2026 Findings |
| Quality signals | [LIMO: Less is More](https://arxiv.org/abs/2502.03387) | COLM 2025 |
| Language mixing | [DeepSeek-R1](https://arxiv.org/abs/2501.12948) | arXiv, Jan 2025 |
| Answer alignment | [Sky-T1](https://novasky-ai.github.io/posts/sky-t1/) | NovaSky, Jan 2025 |
| Length calibration | [Between Underthinking and Overthinking](https://arxiv.org/abs/2505.00127) | arXiv, May 2025 |
| Data recipes | [OpenThoughts](https://arxiv.org/abs/2506.04178) | arXiv, Jun 2025 |

## LLM-as-Judge (Optional Semantic Scoring)

ReasonMetrics evaluates **structural** quality at native speed. For **semantic** correctness (e.g., "is the math actually right?"), use the included LLM judge with any provider:

```bash
export GROQ_API_KEY=gsk_...        # or OPENROUTER_API_KEY / OPENAI_API_KEY, or none for Ollama
python3 scripts/llm_judge.py --provider groq traces.jsonl -n 50
python3 scripts/llm_judge.py --provider ollama traces.jsonl   # local, no auth needed
```

Evaluates logical validity, factual correctness, answer correctness, and reasoning completeness. Requires `pip install httpx`.

## Dataset Conversion

Convert any supported HuggingFace dataset to the JSONL input format:

```bash
pip install datasets
python3 scripts/convert_dataset.py limo                          # 817 math traces
python3 scripts/convert_dataset.py medical --max-rows 5000       # medical reasoning
python3 scripts/convert_dataset.py openthoughts --max-rows 5000  # multi-domain
python3 scripts/convert_dataset.py opencoder --max-rows 5000     # code reasoning
python3 scripts/convert_dataset.py s1k                           # 1K multi-domain
```

## Development

Cargo workspace:

```
crates/
├── reasonmetrics-core/ # pure scoring engine — no I/O, compiles to wasm32
├── reasonmetrics-cli/  # batch CLI (rayon, parquet, gzip, HTML reports)
└── reasonmetrics-wasm/ # wasm-bindgen wrapper consumed by web/
web/                   # Vite + React (TypeScript) profiler app
```

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo check -p reasonmetrics-core --no-default-features --target wasm32-unknown-unknown
wasm-pack test --node crates/reasonmetrics-wasm
cd web && npm test
```

CI runs formatting, clippy, tests, the wasm32 check, and a wasm-pack build on every push/PR. Tagging `web-v*` deploys the web app to GitHub Pages.

## Contribute & Earn — via Gittensor

We're bringing ReasonMetrics to [Gittensor](https://gittensor.io), a Bittensor subnet that pays contributors in TAO for merged pull requests — once the listing is live, every merged contribution here earns.

This isn't ordinary open contribution — it moves surprisingly fast. Gittensor supplies a standing pool of contributors with a direct incentive to ship mergeable work, and ReasonMetrics is built to absorb them: model support, language lexicons, and dataset converters are independent data-plus-fixture PRs that CI verifies mechanically and that merge within a 48-hour review SLA. Rewarded contributors → quick objective merges → broader coverage → more contributors. And every new reasoning-model release creates fresh, well-defined work, so the pipeline never runs dry.

**Getting started:** pick a `good first issue`, follow [CONTRIBUTING.md](CONTRIBUTING.md), open your PR — and once the listing is live, merged PRs earn through Gittensor automatically.

---

<div align="center">

MIT Licensed · Built with Rust

</div>
