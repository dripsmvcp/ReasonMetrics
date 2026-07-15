# ReasonMetrics Showcase Results

Scored with the shipped default config. **`quality` here is the calibrated
`quality_score`: a percentile against a reference corpus of real reasoning
traces** ("better than N% of real traces"), not a 0–100 grade. See
[docs/CALIBRATION.md](../docs/CALIBRATION.md) for how that scale is built and
what it does and doesn't mean.

> **Read the two blocks below differently.** LIMO, s1K, and Medical *are* the
> reference corpus the percentile scale is fitted on, so their `quality` numbers
> are partly circular — they describe where each sits *within* the reference.
> OpenThoughts and OpenCodeReasoning are **out-of-sample**: their numbers show
> where genuinely external datasets fall against that reference. The reference is
> math-heavy and long-form (median ~2,600 words), so short or non-math traces
> score low on `quality` even when they are perfectly good of their kind — that
> is the scale being honest about distance from the reference, not a quality
> verdict. The per-dimension scores below are *raw* (uncalibrated) and are the
> better lens for "is this trace well-formed".

## Reference corpus (the scale is fitted on these)

| Dataset | Traces | Avg quality (percentile) | quality ≥ 70 |
|---------|--------|--------------------------|--------------|
| **LIMO** (GAIR/LIMO) | 817 | **78.5** | 73.8% |
| **s1K** (simplescaling/s1K-1.1) | 1,000 | **50.0** | 20.7% |
| **Medical** (FreedomIntelligence/medical-o1-reasoning-SFT) | 700 | **21.6** | 0.0% |

LIMO — hand-curated competition math — sits highest *within* the reference, which
is the one genuinely validating signal here: curation the tool never saw still
lands it at the top. Medical sits lowest because its traces are short clinical
CoT, structurally sparse next to long math reasoning (see its raw dimensions
below — they are clean, just short).

## Out-of-sample (not in the reference)

| Dataset | Traces | Avg quality (percentile) | quality ≥ 70 |
|---------|--------|--------------------------|--------------|
| **OpenThoughts** (open-thoughts/OpenThoughts-114k) | 5,000 | **37.0** | 4.1% |
| **OpenCodeReasoning** (nvidia/OpenCodeReasoning) | 5,000 | **33.1** | 5.5% |

Both land below the reference median. Part of that is real (a filter *should*
separate these from curated LIMO), and part is the math-heavy reference — code
traces in particular are being measured against a distribution that contains no
code. Treat cross-dataset `quality` comparisons as reference-relative, and rank
*within* a dataset (`filter --top-percent N`) when you curate one.

Throughput is unchanged by calibration (it is a monotone remap); see
[COMPARISON.md](COMPARISON.md) for the speed/cost numbers.

## Per-Dimension Breakdown (raw scores)

These are the raw, uncalibrated dimension scores — the direct read on trace
*shape*. They are what the calibrated composite is built from.

### LIMO (817 traces) — Curated Math Reasoning
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 94.3 | 98.5% |
| Language Consistency | 99.2 | 98.9% |
| Answer Alignment | 89.3 | 92.9% |
| Structural Clarity | 89.4 | 77.2% |
| Repetition | 96.8 | 98.5% |
| Overthinking | 76.0 | 53.7% |
| Self-Verification | 95.6 | 94.1% |
| Length Calibration | 90.8 | 80.3% |

**Key findings**: LIMO is hand-curated, and it is strong on every dimension —
which is why it tops the reference-relative `quality` scale. Its one relative
weakness is overthinking (46.3% below the ≥80 bar).

### s1K-1.1 (1,000 traces) — Multi-Domain, `<think>`-tagged
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 96.1 | 99.7% |
| Language Consistency | 99.6 | 99.4% |
| Answer Alignment | 65.0 | 0.1% |
| Structural Clarity | 84.4 | 62.9% |
| Repetition | 98.7 | 100.0% |
| Overthinking | 63.0 | 31.4% |
| Self-Verification | 89.7 | 83.6% |
| Length Calibration | 83.2 | 56.0% |

**Key findings**: s1K's DeepSeek trajectories verify their work well (89.7) but
run long, so overthinking is its weakest dimension. Answer alignment clusters
just under the ≥80 bar (avg 65, almost nothing above 80) — the traces converge
without explicit convergence language. 10.1% show language mixing.

### Medical-o1 (700 traces) — Clinical Reasoning
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 99.8 | 100.0% |
| Language Consistency | 100.0 | 100.0% |
| Answer Alignment | 42.1 | 0.0% |
| Structural Clarity | 53.0 | 0.4% |
| Repetition | 100.0 | 100.0% |
| Overthinking | 100.0 | 100.0% |
| Self-Verification | 37.6 | 3.0% |
| Length Calibration | 99.6 | 99.0% |

**Key findings**: Medical traces are linguistically pristine (100% consistency,
zero repetition) and short, so they never overthink. But they rarely verify
explicitly (37.6) and read as narrative rather than stepwise (structural 53.0) —
the two dimensions that keep their calibrated `quality` low against a math-heavy
reference. This is the clearest example of "clean, but far from the reference".

### OpenThoughts-114k (5,000 traces) — Large Multi-Domain
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 99.8 | 100.0% |
| Language Consistency | 99.9 | 99.9% |
| Answer Alignment | 57.0 | 0.0% |
| Structural Clarity | 80.4 | 47.4% |
| Repetition | 92.9 | 95.2% |
| Overthinking | 85.0 | 68.3% |
| Self-Verification | 52.7 | 21.5% |
| Length Calibration | 80.8 | 58.0% |

**Key findings**: Strong structure (80.4) and moderate verification (52.7);
about half the traces lack explicit verification, which is the main thing pulling
their calibrated `quality` down.

### OpenCodeReasoning (5,000 traces) — Code Reasoning
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 99.8 | 100.0% |
| Language Consistency | 99.9 | 99.9% |
| Answer Alignment | 54.2 | 7.2% |
| Structural Clarity | 81.2 | 50.2% |
| Repetition | 93.4 | 95.0% |
| Overthinking | 76.8 | 52.3% |
| Self-Verification | 54.4 | 26.2% |
| Length Calibration | 83.2 | 56.5% |

**Key findings**: Well-organized (structural 81.2) with the lowest language
mixing of any dataset (0.1%). Remember the caveat: the reference contains no
code, so its calibrated `quality` (33.1) understates it relative to how it would
score against a code reference.

## What This Tells Us

1. **Curation shows up.** LIMO tops the reference-relative scale without the tool
   ever seeing the curators' choices — the one clean validating signal here.
2. **The scale is reference-relative and math-heavy.** Short and non-math
   datasets score low on calibrated `quality` even when their raw dimensions are
   clean. Compare *within* a dataset, not across, unless you accept the reference
   as your yardstick. ([Issue #31](https://github.com/dripsmvcp/ReasonMetrics/issues/31)
   tracks the related finding that some dimensions don't transfer across models.)
3. **Self-verification is the most consistently missing signal** — 46–97% of
   traces in the non-curated datasets lack it — and it is also the dimension that
   holds up best against objective correctness (see CALIBRATION.md).
4. **Language mixing is real** — s1K has 10.1% mixed-language traces, matching the
   DeepSeek-R1 finding.

## Reproduce

```bash
python scripts/convert_dataset.py limo          # or s1k / medical / openthoughts / opencoder
reasonmetrics report -i limo_traces.jsonl -o showcase/limo_report.html
reasonmetrics stats  -i limo_traces.jsonl
```
