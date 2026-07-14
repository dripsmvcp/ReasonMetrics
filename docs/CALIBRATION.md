# Calibration: structural scores vs. LLM judge

ReasonMetrics scores traces **structurally** — it never reads the math, only the
shape of the reasoning (verification phrases, repetition, restarts, length
ratios, language mixing). A fair question is whether those structural signals
track anything *semantic*. This study measures that.

**TL;DR:** structural scores are a useful cheap prefilter, not a semantic
oracle. See the correlation table below and [LIMITATIONS.md](LIMITATIONS.md)
for the ways the scorer can be fooled.

## Method

1. **Corpora** — three public reasoning-trace datasets, converted to
   ReasonMetrics JSONL (`problem`, `thinking`, `answer`):

   | dataset | traces | thinking source | domain |
   |---|---|---|---|
   | [GAIR/LIMO](https://huggingface.co/datasets/GAIR/LIMO) | 817 | `solution` (native CoT) | competition math |
   | [simplescaling/s1K-1.1](https://huggingface.co/datasets/simplescaling/s1K-1.1) | 1000 | `deepseek_thinking_trajectory` | math/science |
   | [FreedomIntelligence/medical-o1-reasoning-SFT](https://huggingface.co/datasets/FreedomIntelligence/medical-o1-reasoning-SFT) | 700 | `Complex_CoT` | clinical reasoning |

2. **Structural scoring** — all 2,517 traces scored with
   `reasonmetrics score` (default config, v0.2.0).

3. **Stratified sample** — per dataset, traces sorted by `quality_score` and
   every k-th taken (40/dataset = 120; judged subsample = every 2nd = 60,
   20/dataset) so the sample spans each dataset's full score range instead of
   clustering at the mean.

4. **LLM judge** — `scripts/llm_judge.py` asks an independent model to grade
   each trace 0–100 on logical validity, factual correctness, answer
   correctness, and reasoning completeness (`semantic_composite` = weighted
   mean). Judge: **qwen2.5-coder:7b via local Ollama** (8192-token context;
   thinking truncated head+tail beyond 12k chars; temperature 0).

5. **Correlation** — tie-aware Spearman rank correlation per
   structural-dimension × judge-dimension pair (`scripts/calibrate.py`,
   no dependencies).

## Results

Judged sample: **n = 60** (20/dataset), zero judge errors, run 2026-07-14.

### Composite vs judge, per dataset

| dataset | n | quality_score vs semantic_composite (ρ) |
|---|---|---|
| limo | 20 | +0.09 |
| s1k | 20 | **+0.53** |
| medical | 20 | +0.31 |
| pooled | 60 | +0.02 |

The pooled value is *lower than every per-dataset value*. That is a pooling
artifact (Simpson-style): datasets differ in both structural baseline and
judge baseline, so pooling across them cancels real within-dataset signal.
Compare within a corpus; don't compare scores across corpora.

### Structural dimensions vs `semantic_composite` (within-dataset)

| dimension | limo | s1k | medical | pooled |
|---|---|---|---|---|
| quality_score | +0.09 | +0.53 | +0.31 | +0.02 |
| verification_score | **+0.48** | **+0.36** | **+0.42** | +0.07 |
| length_score | +0.34 | **+0.70** | −0.08 | +0.38 |
| overthinking_score | +0.18 | +0.45 | n/a¹ | +0.31 |
| efficiency_score | −0.26 | +0.17 | n/a¹ | +0.10 |
| answer_alignment_score | −0.19 | +0.01 | +0.40 | −0.11 |
| repetition_score | −0.06 | −0.22 | n/a¹ | +0.01 |
| structural_score | −0.05 | +0.05 | **−0.40** | −0.20 |
| language_score | −0.24 | n/a¹ | n/a¹ | −0.12 |

¹ constant on that dataset (short conversational medical traces max these
dimensions out; every s1k/medical trace is pure English), so rank correlation
is undefined.

### What we take from this

- **`verification_score` is the most consistent semantic signal** — positive
  (+0.36 to +0.48) in all three corpora. Traces that actually check their work
  are judged better. This matches the intuition the scorer was built on.
- **`length_score` and `overthinking_score` carry real signal on long
  math/science traces** (ρ up to +0.70 on s1K) and none on short clinical
  ones. Signal strength tracks trace length — the scorer has more to look at.
- **The composite tracks the judge within every corpus** (+0.09 / +0.31 /
  +0.53), strongest exactly where traces are long R1-style reasoning, which
  is the tool's target input.
- **`structural_score` is miscalibrated**: zero-to-negative everywhere,
  −0.40 on medical. Rigid markdown/enumeration structure does not predict
  judged quality — conversational traces judged best score worst on it. Filed
  as [#13](https://github.com/dripsmvcp/ReasonMetrics/issues/13); treat this
  dimension as formatting description, not quality, until it is reworked.
- **The judge itself is a blunt instrument**: it graded 95% of traces ≥80
  and its four dimensions intercorrelate +0.36..+0.70 (halo effect). Judge
  leniency compresses ranks and attenuates every ρ above.

## How to read this honestly

- **Weak judge.** A local 7B model is the judge — it grades leniently and
  noisily compared to a frontier model. Correlations here are a *lower-bound
  sanity check*, not a validation. Re-running with a stronger judge
  (`--provider groq/openrouter/openai`) is one command.
- **Range restriction.** These are curated SFT datasets: almost everything in
  them is decent (structural 65–98, no true garbage). Correlations over a
  narrow quality band are attenuated; the scorer's job in practice —
  separating garbage from decent — is easier than what is measured here.
- **n = 60.** |ρ| below ~0.26 is indistinguishable from zero at p < 0.05.
- **Structural ≠ semantic, by design.** A trace can be structurally clean and
  logically wrong; [the adversarial suite](LIMITATIONS.md) constructs exactly
  such traces. Calibration tells you how often that matters on *natural*
  data, not whether it is possible (it is).

## Reproduce

```bash
# convert your traces to {"id", "problem", "thinking", "answer"} JSONL, then:
reasonmetrics score -i traces.jsonl -o scored.jsonl

python scripts/llm_judge.py traces.jsonl --provider ollama \
    --model qwen2.5-coder:7b --timeout 900 -n 60 -o judged.jsonl

python scripts/calibrate.py scored.jsonl --judged judged.jsonl
```

Ollama gotcha: the default loaded context is 4096 tokens and long prompts get
silently truncated. Create a judge variant first:

```bash
printf 'FROM qwen2.5-coder:7b\nPARAMETER num_ctx 8192\n' | ollama create qwen-judge -f -
```
