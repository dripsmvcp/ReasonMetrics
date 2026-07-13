# ReasonMetrics launch kit

> **DRAFTS — not to be auto-posted. Launch is a manual, human decision.**
> Everything below is prepared material only. Review, edit, and post by hand
> when (and if) the decision is made.

---

## 1. Show HN

**Title** (78 chars):

> Show HN: ReasonMetrics – score and dissect LLM reasoning traces in your browser

**Body** (two paragraphs):

ReasonMetrics takes a reasoning trace — the `<think>...</think>` text a model
like DeepSeek-R1 or Qwen3 produces before its answer — and shows you its
anatomy: restarts ("wait, let me try again"), self-verification passes, and
repeated passages are highlighted inline as annotated spans, next to a
scorecard of 9 heuristic quality dimensions (efficiency, language
consistency, answer alignment, structural clarity, repetition, overthinking,
self-verification, length calibration, and an accuracy-efficiency score
ported from LLMThinkBench). You can paste a trace or a JSONL batch, load one
of five pre-baked examples from small local models, or point it at a local
Ollama server and watch the anatomy update live while the model is still
thinking. The scoring core is a Rust workspace shared by the CLI, the
library, and the browser build.

Technically it's a Rust scoring engine compiled to WebAssembly and driven by
a small React (TypeScript) app — there is no backend, no accounts, and no
telemetry; traces never leave your machine (share links pack the trace into
the URL fragment, so even sharing is client-side). The scores are heuristics,
not ground truth — they're meant for triaging training data and eyeballing
model behavior, not as a benchmark. I'd love feedback on which dimensions
are actually useful signals, what's missing, and traces where the span
detection (restarts/verification/repetition) gets it wrong.

---

## 2. r/LocalLLaMA post

**Suggested title:**

> I made a browser tool that scores your local model's reasoning traces live while it streams from Ollama

**Body:**

If you run reasoning models locally you've probably stared at a wall of
`<think>` text wondering whether the model is actually getting anywhere or
just going "wait, actually, let me reconsider" for 2,000 tokens. I built
ReasonMetrics to make that visible.

Point it at your local Ollama (it talks to `localhost:11434` straight from
the browser — nothing is uploaded anywhere), pick a model, give it a prompt,
and it re-scores the trace live as tokens stream in. Restarts get red wavy
underlines, self-verification gets green highlights, repeated passages get
collapsed into pills, and a 9-dimension scorecard updates as it goes. You
can also paste traces or JSONL dumps, and there's a gallery of five
pre-baked examples generated with deepseek-r1:1.5b and qwen3:1.7b — the
r1-rambling one (screenshot: `docs/assets/gallery/r1-rambling.png`) is a
1.5B R1 distill grinding trial division on "Is 3599 a prime number?" for
770+ tokens without ever reaching an answer, and it's exactly as painful
as you'd expect.

Everything runs client-side: the scoring engine is Rust compiled to WASM
(with an overthinking metric ported from LLMThinkBench).
The scores are heuristics, so treat them as a lens, not a leaderboard.
Would genuinely like to hear what your models' traces look like — especially
weird cases where the annotations misfire.

---

## 3. Asset checklist

Screenshots live in `docs/assets/gallery/` (PNG, captured from the app with
each gallery fixture loaded). Suggested captions for posts/README:

| # | File | Caption |
|---|------|---------|
| 1 | `r1-rambling.png` | deepseek-r1:1.5b grinding trial division on "Is 3599 a prime number?" — 770+ tokens and it never reaches an answer |
| 2 | `concise-qwen.png` | qwen3:1.7b dispatching "What is 2 + 2?" in a few clean sentences |
| 3 | `language-mixing.png` | qwen3:1.7b flipping between Chinese and English mid-thought on a translation task |
| 4 | `restart-loop.png` | The bat-and-ball trick question sends qwen3:1.7b into wait-and-double-check loops — restarts, repetitions, and self-checks flagged inline |
| 5 | `verified-tidy.png` | Structured reasoning with an explicit self-check — verification spans highlighted green |

GIFs: none captured (optional per plan; static PNGs cover the launch posts).
If wanted later: a short capture of live mode streaming from Ollama with the
scorecard updating is the highest-value clip.

**Claims checklist (keep posts honest):**

- 100% client-side; no accounts; traces never leave the machine.
- Scoring is *heuristic* (9 dimensions) — do not present as a benchmark or ground truth.
- Overthinking/accuracy-efficiency metric ported from [LLMThinkBench](https://arxiv.org/abs/2507.04023).
- Gallery traces were genuinely generated with local Ollama models (deepseek-r1:1.5b, qwen3:1.7b); any trimmed fixture is marked `"curated": true` in its JSON.
