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
- **`structural_score` scores badly against this judge** (−0.05 limo / +0.05 s1k
  / −0.40 medical). We tried to explain that away as a length confound and
  **published the wrong explanation** — see the retraction below. Against ground
  truth it is one of the *better* dimensions. Treat the judge result here as
  weak evidence, not as a verdict; tracking in
  [#13](https://github.com/dripsmvcp/ReasonMetrics/issues/13).
- **The judge itself is a blunt instrument**: it graded 95% of traces ≥80
  and its four dimensions intercorrelate +0.36..+0.70 (halo effect). Judge
  leniency compresses ranks and attenuates every ρ above.

## Ground truth beats the judge (n=938)

The study above rests on 60 traces graded by a lenient 7B model. We replaced that
with an **objective** label: s1K-1.1 ships, per problem, R1's real reasoning trace,
R1's final answer, *and* the human ground-truth solution. Comparing the two answers
with symbolic verification (`math_verify`) labels **938 traces correct/incorrect**
with no LLM in the loop — a 15× larger, judge-free evidence base, balanced 48/52.

AUC = P(a correct trace outranks an incorrect one). 0.50 is a coin flip.

| dimension | AUC vs. answer-correctness |
|---|---|
| **quality_score (composite)** | **0.714** |
| structural_score | 0.685 |
| length_score | 0.680 |
| overthinking_score | 0.648 |
| verification_score | 0.623 |
| repetition_score | 0.551 |
| language_score | 0.492 |
| efficiency_score | 0.470 |
| answer_alignment_score | 0.469 |
| *(trace length alone, shorter=better)* | *0.710* |

**The composite really does predict whether reasoning reaches the right answer**
(0.714) — the strongest evidence this project has. Three caveats, stated plainly:

1. **`structural_score` is one of the better dimensions here (0.685)**, flatly
   contradicting the judge study. See the retraction below.
2. **Length alone scores 0.710** — nearly the whole composite. On this corpus most
   of our signal *is* "shorter is better". Nine dimensions are not yet earning
   their keep over one heuristic. *(This turns out to be a DeepSeek-specific
   artifact — see "Does it survive a second model?" below.)*
3. **The default filter was useless.** `filter --min-score 70` kept **99.9%** of
   real R1 traces (937 of 938). The ranking worked; the *threshold* did not
   discriminate, because scores were crushed into the top of the range. That, not
   any single dimension, is why a rambling trace showed 82.9 in green.
   **Fixed in [#30](https://github.com/dripsmvcp/ReasonMetrics/issues/30)** — see
   "The calibrated scale" below.

## Is the filter just picking easy problems? (n=938, 2 models)

An AUC of 0.714 against answer-correctness has an ugly alternative explanation:
**hard problem → long, messy trace → wrong answer.** If so, the scorer would be
an elaborate difficulty detector, and filtering by it would quietly strip the hard
problems out of a training set — the opposite of curation.

That worry is well-founded on its face: filtering *does* select easier problems.
Tighten to the top 30% and problems both models got wrong fall from 46.8% → 32.6%,
while problems both got right rise from 39.4% → 57.1%.

To separate the two effects, hold the **model** constant (score only DeepSeek's
traces) and split problems by an **independent** difficulty proxy — did *Gemini*
get the same problem right? Gemini's correctness says nothing about the DeepSeek
trace being scored, so it cannot leak. The difficulty effect it exposes is huge:
DeepSeek is 88.3% correct on the easy stratum and 15.4% on the hard one.

| dimension | pooled | easy-only | hard-only |
|---|---|---|---|
| **quality_score** | **0.713** | **0.701** | **0.632** |
| verification_score | 0.623 | 0.600 | 0.596 |
| structural_score | 0.688 | 0.665 | 0.607 |
| length_score | 0.680 | 0.693 | 0.541 *(n.s.)* |
| *(length alone, shorter=better)* | 0.710 | 0.702 | 0.590 |

**The composite survives inside both strata** (0.632–0.701). It is not merely a
difficulty detector: given two traces on problems of comparable difficulty, it
still ranks the one that reaches the right answer higher. This is the result that
justifies filtering at all.

## Does it survive a second model? (n=940)

s1K ships a Gemini trace for every problem too, so the whole study re-runs on a
different model answering the *same questions*. This is where the composite's
story gets less flattering.

| dimension | DeepSeek AUC | Gemini AUC |
|---|---|---|
| quality_score | 0.713 | 0.574 |
| **verification_score** | **0.623** | **0.662** |
| structural_score | 0.688 | 0.468 *(n.s.)* |
| length_score | 0.680 | 0.525 |
| *(length alone, shorter=better)* | 0.710 | **0.540** |

- **The length signal does not transfer.** "Shorter is better" scores 0.710 on
  DeepSeek and **0.540** on Gemini — on the same problems. Caveat 2 above is
  therefore a statement about DeepSeek's verbosity, *not* a general fact about
  reasoning traces, and the earlier phrasing ("most of our signal is shorter is
  better") overclaims. DeepSeek's traces are 4,658 words at the median; Gemini's
  are 1,980.
- **`verification_score` is the most transferable dimension *across models*** —
  positive and significant on both models and in both difficulty strata
  (0.596–0.680 throughout). *(This does not survive a change of **domain** — see
  the next section, where it inverts on code.)*
- **`structural_score` does not replicate** (0.688 → 0.468, and *below* chance on
  Gemini's easy stratum). [#13](https://github.com/dripsmvcp/ReasonMetrics/issues/13)
  stays open.

## Does it survive a second domain? (code, n=8000)

Everything above is math. To test **domain** transfer we built objective code
labels the same way — `scripts/build_code_labels.py` over
[PrimeIntellect/SYNTHETIC-1](https://huggingface.co/datasets/PrimeIntellect/SYNTHETIC-1),
the *raw* verifiable set (the SFT/distillation sets are curated to correct-only
and so useless here; the raw set keeps the ~20% rejected). 8,000 code-reasoning
traces (mostly "predict this code's output"), labelled by the dataset's own
verification — no code executed on our side, exactly as the math side trusts
`math_verify`.

| dimension | math AUC | **code AUC** |
|---|---|---|
| quality_score (composite) | 0.714 | **0.629** |
| **verification_score** | 0.623 | **0.470** *(below chance)* |
| structural_score | 0.688 | 0.568 |
| length_score | 0.680 | 0.669 |
| *(length alone, shorter=better)* | 0.710 | **0.737** |

Two findings that matter more than the headline:

- **`verification_score` inverts.** The one dimension that was stable across
  *models* on math falls to 0.470 — *below chance* — on code. Code-tracing
  reasoning is full of "let me check each step" whether or not it reaches the
  right answer, so the phrase-matching verification signal carries nothing here.
  The "just weight toward verification" fix that looked good on math would
  **hurt** on code.
- **No dimension is stable on both axes.** Length is domain-stable (0.74 code /
  0.71 math) but model-unstable (0.71 DeepSeek / 0.54 Gemini); verification is
  model-stable but domain-unstable. There is no single signal that survives both
  a model change and a domain change.

On code the composite is also **beaten by length alone** (0.629 vs 0.737, paired
bootstrap Δ = −0.108, CI excludes zero) — the opposite of "the dimensions add
value." The default filter still does *something* (`--min-score 70` keeps traces
that are 86.3% correct vs 78.7% for those it drops), but the composite is not the
best-available signal in this domain.

### So can the composite be reweighted? (the #31 answer)

No — not as a single global weighting. Fitting weights on one domain and
evaluating on the other (`reweight_study.py --group-field source`) is a **"no
clean win"**: a code-fitted weighting is *significantly worse* than the hand-set
composite on math (Δ [−0.045, −0.005]), and a math-fitted one is no better on
code. Because the useful signal depends on **both** the model and the domain, the
evidence points away from reweighting and toward **per-domain profiles** (a
math preset, a code preset) — the R9 direction — not one set of weights. This is
the decisive non-math evidence [#31](https://github.com/dripsmvcp/ReasonMetrics/issues/31)
was waiting on, and it argues against the reweight it proposed.

Weights remain **untouched.** The one caveat on this result: the code corpus is a
single dataset and mostly output-prediction, so it is a two-point read on domain
transfer, not the whole space.

### A confound we caught in our own work

The first attempt at the difficulty control used a **paired** design — same
problem, DeepSeek trace vs Gemini trace — and produced a dramatic result: length
"inverted", with the shorter trace *less* likely to be correct (39.5%, p=0.02).

**It was wrong.** Splitting the pairs by which model happened to be right showed
every signal flipping sign completely (`quality_score` won 88.8% of the pairs
DeepSeek got right, and 20.4% of the ones Gemini got right). Pairing holds the
*problem* constant but not the *model* — and since DeepSeek's traces are ~2.4×
longer and score ~7 points higher, the test was really measuring "the scorer
prefers DeepSeek, and DeepSeek is more often right." The finding was discarded
before it reached this page.

That is the third confound-driven false result in this project's calibration work
(after the pooled Simpson artifact and the retracted length-proxy claim). The
lesson is now a rule: **stratify before believing.**

## The calibrated scale

`quality_score` is no longer the raw weighted average of the dimensions. It is
that composite's **percentile against a reference distribution of 2,517 real
reasoning traces** (limo + s1K + medical) — "better than N% of real traces". The
raw composite is still reported, as `raw_score`.

The mapping is monotone, so **ranking is completely unchanged** — every AUC on
this page is identical before and after. Only the scale moved. What that buys:

| | before | after |
|---|---|---|
| `--min-score 70` keeps | 937/938 (99.9%) | **197/938 (21.0%)** |
| accuracy of kept traces | 48.0% (= unfiltered) | **69.0%** |
| accuracy of dropped traces | — | 42.4% |
| adversarial fixtures surviving | 5 of 7 | **0 of 7** |
| `r1-rambling` (never answers) | 82.9, green | **40.4, red** |

Two honest consequences:

- **The scale is corpus-relative.** The reference corpus is math-heavy (1,817 of
  2,517 traces) and long (median 2,578 words). A very short trace scores near
  zero — a "what is 2+2" trace lands at ~1 — which is *correct* (it contains
  almost no reasoning, and is near-worthless as reasoning training data) but will
  surprise you if you read the number as a grade. It is a percentile, not a grade.
- **Scores are not comparable across scorer versions** unless the curve is held
  fixed. Refitting it (`scripts/fit_calibration.py`) is a semver-relevant event,
  and the curve is fitted against the *default weights* — changing the weights
  without refitting makes the percentiles lie.

For size-exact curation, `filter --top-percent N` keeps the best N% of the input
file regardless of the reference distribution.

### Retraction (2026-07-15)

An earlier version of this document claimed `structural_score` "is a length proxy",
citing ρ = +0.50 between it and word count across all 2,517 traces. **That was
wrong, and it was wrong in the specific way this document warns about two sections
above: it was a Simpson artifact.** Pooled it is +0.50; *within* each corpus it is
**−0.21 (limo), −0.19 (s1k), +0.09 (medical)** — the correlation exists in no
dataset and was manufactured entirely by mixing corpora of different lengths.
Three "fixes" derived from that false mechanism were tried and refuted by
measurement; none shipped. The claim is withdrawn.

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

The ground-truth study (n=938), the difficulty stratification, and the
cross-model replication — no LLM judge, no API key:

```bash
pip install datasets math-verify

# Objective correct/incorrect labels by symbolic answer verification.
# --models deepseek,gemini gives two traces per problem, which is what the
# difficulty control and the second-model replication need.
python scripts/build_correctness_labels.py --models deepseek,gemini -o paired.jsonl

reasonmetrics score -i paired.jsonl -o paired_scored.jsonl
python scripts/filter_study.py paired_scored.jsonl --labels paired.jsonl --paired
```

`filter_study.py` reports the retention sweep, the random-drop null, the
length-alone control, per-dimension AUCs with bootstrap CIs, and the paired test.

The cross-domain (code) study — objective code labels, then the same tools plus
the fit-on-one/test-on-the-other transfer test:

```bash
python scripts/build_code_labels.py -o code.jsonl        # PrimeIntellect/SYNTHETIC-1
reasonmetrics score -i code.jsonl -o code_scored.jsonl
python scripts/filter_study.py code_scored.jsonl --labels code.jsonl   # code AUCs

# cross-domain reweight: group by dataset `source`, fit one, evaluate the other
cat code_scored.jsonl s1k_scored.jsonl > all_scored.jsonl
cat code.jsonl s1k_labelled.jsonl      > all_labels.jsonl
python scripts/reweight_study.py all_scored.jsonl --labels all_labels.jsonl \
    --group-field source
```

Refit the calibration curve (maintainer action — see the caveats above):

```bash
python scripts/fit_calibration.py limo_scored.jsonl s1k_scored.jsonl \
    medical_scored.jsonl > crates/reasonmetrics-core/src/calibration/table.rs
```

The original LLM-judge study:

```bash
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
