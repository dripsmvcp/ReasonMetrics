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

Build and use the batch scoring tool:

```bash
cargo build --release                          # first build takes a minute or two
export PATH="$PWD/target/release:$PATH"        # or invoke as ./target/release/reasonmetrics
reasonmetrics --help                           # sanity check
```

```bash
reasonmetrics score  -i traces.jsonl -o scored.parquet   # score all traces
reasonmetrics filter -i traces.jsonl -o clean.jsonl --min-score 70
reasonmetrics report -i traces.jsonl -o report.html      # generate HTML report
reasonmetrics stats  -i traces.jsonl                     # summary statistics
reasonmetrics explain                                    # score dimension reference
reasonmetrics init-config > my-scoring.toml              # dump default weights/thresholds
```

Every command takes `--config <file>` (default: `reasonmetrics.toml` in the current directory — the repo root ships a ready-made one). To customize scoring, edit that file in place or pass a generated one: `reasonmetrics score --config my-scoring.toml -i traces.jsonl`. (Careful: `init-config > reasonmetrics.toml` at the repo root overwrites the tracked file.)

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

This repository is listed on [Gittensor](https://gittensor.io), a Bittensor subnet that pays contributors in TAO for merged pull requests.

This isn't ordinary open contribution — it moves surprisingly fast. Gittensor supplies a standing pool of contributors with a direct incentive to ship mergeable work, and ReasonMetrics is built to absorb them: model support, language lexicons, and dataset converters are independent data-plus-fixture PRs that CI verifies mechanically and that merge within a 48-hour review SLA. Rewarded contributors → quick objective merges → broader coverage → more contributors. And every new reasoning-model release creates fresh, well-defined work, so the pipeline never runs dry.

**Getting started:** pick a `good-first-issue`, follow [CONTRIBUTING.md](CONTRIBUTING.md), open your PR — merged PRs earn through Gittensor automatically.

---

<div align="center">

MIT Licensed · Built with Rust

</div>
