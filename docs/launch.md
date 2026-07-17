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
model behavior, not as a benchmark. The headline score is a percentile
against 2,517 real reasoning traces, not a grade. If you want receipts
before trusting any of it: we measured the composite against objective
answer-correctness — it ranks a correct R1 trace above an incorrect one with
AUC 0.714 (n=938, symbolic verification, no LLM judge), survives a difficulty
control, and transfers imperfectly to other models and domains — all
documented with the negative results in
[docs/CALIBRATION.md](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/CALIBRATION.md),
and [docs/LIMITATIONS.md](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/LIMITATIONS.md)
demonstrates seven ways to fool individual scorers, each pinned by a CI
fixture. I'd love feedback on which dimensions are actually useful signals,
what's missing, and traces where the span detection
(restarts/verification/repetition) gets it wrong.

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
as you'd expect (it lands at the 40th percentile of real traces, in red).

Everything runs client-side: the scoring engine is Rust compiled to WASM
(with an overthinking metric ported from LLMThinkBench).
The scores are heuristics, so treat them as a lens, not a leaderboard — the
big dial is literally "better than N% of 2,517 real reasoning traces", so a
two-line trace scoring near zero is the scale working, not a bug. There's a
[calibration study](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/CALIBRATION.md)
against objective answer-correctness and a
[limitations doc](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/LIMITATIONS.md)
showing exactly how to fool each scorer, if you want to know where the sharp
edges are. There's also a Compare tab for putting two traces side by side —
handy for "same prompt, two models" arguments.
Would genuinely like to hear what your models' traces look like — especially
weird cases where the annotations misfire.

---

## 3. Asset checklist

Screenshots live in `docs/assets/gallery/` (PNG, captured from the app with
each gallery fixture loaded — **regenerated 2026-07-17 on the calibrated
percentile scale**, web-v0.4.0 UI). Suggested captions for posts/README:

| # | File | Score shown | Caption |
|---|------|------------|---------|
| 1 | `r1-rambling.png` | **40.4, red** | deepseek-r1:1.5b grinding trial division on "Is 3599 a prime number?" — 770+ tokens and it never reaches an answer |
| 2 | `concise-qwen.png` | 0.6, red | qwen3:1.7b dispatching "What is 2 + 2?" in a few clean sentences — near-zero percentile because there's almost no reasoning to score, which is the scale working |
| 3 | `language-mixing.png` | 0.9, red | qwen3:1.7b flipping between Chinese and English mid-thought on a translation task |
| 4 | `restart-loop.png` | 81.1, green | The bat-and-ball trick question sends qwen3:1.7b into wait-and-double-check loops — restarts, repetitions, and self-checks flagged inline |
| 5 | `verified-tidy.png` | 58.5, amber | Structured reasoning with an explicit self-check — verification spans highlighted green |

Caption discipline: the two ~1-percentile examples read as "the tool hates
short answers" unless the caption says why (percentile of *reasoning* traces;
a 2+2 trace is near-worthless as reasoning training data). Never crop the
dial's "better than N% of real reasoning traces" caption out of a screenshot —
it is what makes the number honest.

GIFs: none captured (optional per plan; static PNGs cover the launch posts).
If wanted later: a short capture of live mode streaming from Ollama with the
scorecard updating is the highest-value clip.

**Claims checklist (keep posts honest):**

- 100% client-side; no accounts; traces never leave the machine.
- Scoring is *heuristic* (9 dimensions) — do not present as a benchmark or ground truth.
- The headline score is a **percentile against a reference corpus of 2,517 real
  traces** (math-heavy, long — a trivially short trace scoring ~1 is correct
  behavior). Say "percentile", never "grade". ([CALIBRATION.md — the calibrated scale](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/CALIBRATION.md#the-calibrated-scale))
- Calibration claims must travel with their caveats, as a package:
  composite AUC **0.714** vs objective answer-correctness (938 DeepSeek-R1
  traces, symbolic verification, no LLM judge); survives a difficulty control
  (0.632 hard / 0.701 easy strata); **but** 0.574 on Gemini traces, 0.629 on
  code (where length alone scores 0.737). Quote the first number without the
  others and the claim is dishonest.
- Filtering claim: on the calibrated scale `--min-score 70` keeps ~21% of R1
  traces at **69.0% correct vs 42.4%** for what it drops; **0 of 7**
  adversarial fixtures survive it. Individual scorers are still gameable —
  [LIMITATIONS.md](https://github.com/dripsmvcp/ReasonMetrics/blob/main/docs/LIMITATIONS.md)
  is the honest framing ("removes the worst traces, doesn't certify survivors").
- Overthinking/accuracy-efficiency metric ported from [LLMThinkBench](https://arxiv.org/abs/2507.04023).
- Gallery traces were genuinely generated with local Ollama models (deepseek-r1:1.5b, qwen3:1.7b); any trimmed fixture is marked `"curated": true` in its JSON.
- Screenshots/scores in posts must come from the **calibrated** build
  (web-v0.4.0+). Anything showing r1-rambling at 82.9/green is from the
  pre-calibration scale — do not use it.

---

## 4. Posting plan

**Order and timing** (each post is a separate, manual decision):

1. **Show HN** — mid-week, morning US time. Lead with the anatomy view;
   the calibration + limitations docs are the credibility hook for the
   inevitable "it's just regexes" thread — link them, don't argue.
2. **r/LocalLLaMA** — the following week, different angle: the live-Ollama
   streaming scorecard leads, the scoring evidence trails.
3. **X thread + HF community post** — trailing, recycling whichever angle
   performed better.

**First-48h protocol** (per post):

- Answer everything, fast — first two days decide the thread.
- Every "your score is wrong on this trace" comment is gold: convert it into
  a miscalibration issue with the trace as a fixture, thank the reporter.
- Ship one small, visible fix during the window and say so in the thread.
- Never present the score as a benchmark; when pressed, the honest sentence
  is "it's a percentile against real traces, calibrated against objective
  answer-correctness, with the failure modes documented."
