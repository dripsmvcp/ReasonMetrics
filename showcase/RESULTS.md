# ReasonMetrics Showcase Results

## Cross-Dataset Comparison

| Dataset | Traces | Avg Quality | High Quality % | Throughput |
|---------|--------|------------|----------------|------------|
| **LIMO** (GAIR/LIMO) | 817 | **91.9** | 98.7% | ~750/s |
| **OpenThoughts** (open-thoughts/OpenThoughts-114k) | 5,000 | **82.6** | 74.4% | ~2,650/s |
| **Medical** (FreedomIntelligence/medical-o1-reasoning-SFT) | 5,000 | **79.8** | 46.2% | ~24,300/s |
| **OpenCodeReasoning** (nvidia/OpenCodeReasoning) | 5,000 | **81.7** | 65.1% | ~1,200/s |
| **s1K** (simplescaling/s1K-1.1) | 1,000 | **75.5** | 28.9% | ~45,000/s |

> All benchmarks run on a single machine with `cargo build --release`. No GPU required.

## Per-Dimension Breakdown

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

**Key findings**: LIMO is a hand-curated dataset — ReasonMetrics confirms its quality is genuinely high. Answer alignment scores 89.3% thanks to convergence-based scoring detecting `\boxed{}`, conclusion phrases, and answer echo together. The main weakness is overthinking (18.7% flagged). Only 3.4% have language mixing.

### s1K-1.1 (1,000 traces) — Multi-Domain with `<think>` Tags
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 100.0 | 100.0% |
| Language Consistency | 96.6 | 93.3% |
| Answer Alignment | 38.8 | 0.0% |
| Structural Clarity | 53.8 | 11.0% |
| Repetition | 99.9 | 99.8% |
| Overthinking | 100.0 | 100.0% |
| Self-Verification | 38.7 | 0.7% |
| Length Calibration | 49.8 | 37.3% |

**Key findings**: s1K traces are efficient (no restarts) but lack self-verification (97.1% have none) and structural clarity. These traces are short and terse — they converge quickly without explicit convergence language, which keeps alignment moderate. 10.1% language mixing detected.

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

**Key findings**: OpenThoughts shows strong structural clarity (80.4) and moderate alignment (57.0). The conversational structure detection captures informal reasoning well. Self-verification is moderate (52.7) with implicit verification patterns contributing. 11.7% flagged for overthinking.

### Medical-o1 (5,000 traces) — Medical Reasoning
| Dimension | Avg | % ≥ 80 |
|-----------|-----|--------|
| Efficiency | 99.8 | 100.0% |
| Language Consistency | 100.0 | 100.0% |
| Answer Alignment | 41.9 | 0.0% |
| Structural Clarity | 52.9 | 0.4% |
| Repetition | 100.0 | 100.0% |
| Overthinking | 100.0 | 100.0% |
| Self-Verification | 37.8 | 3.5% |
| Length Calibration | 99.6 | 99.0% |

**Key findings**: Medical traces are linguistically clean (100% language consistency, zero repetition) with well-calibrated length. Alignment (41.9) is the weakest dimension because medical traces use highly diverse conversational endings. The structural convergence check (declarative endings) provides a baseline signal. Self-verification remains low (80.9% lack it) because medical reasoning uses narrative rather than explicit "let me verify" patterns.

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

**Key findings**: OpenCodeReasoning traces contain `<think>` tags with detailed reasoning. Structural clarity is strong (81.2) — code reasoning tends to be well-organized. 21.9% flagged for overthinking, and 48.5% lack self-verification. Only 0.4% language mixing, much lower than other datasets.

## What This Tells Us

1. **Curated datasets (LIMO) genuinely score higher** — ReasonMetrics validates that manual curation produces measurably better reasoning traces.
2. **Convergence-based alignment works across domains** — the multi-signal approach (phrase matching + structural declarative endings + anti-divergence) handles math, medical, and code traces.
3. **Self-verification is the most commonly missing quality signal** — across all non-curated datasets, 49-97% of traces lack verification phrases.
4. **Domain matters** — medical traces use conversational reasoning patterns that differ from math/code traces. The domain-aware verification and conversational structure detection partially address this.
5. **Language mixing is a real issue** — s1K has 10.1% mixed-language traces, consistent with the DeepSeek-R1 finding that RL-trained models mix languages.
6. **Speed is production-ready** — 750-45,000 traces/second depending on trace length (longer traces = more work per trace).

## Files in This Directory

- `limo_scored.parquet` — Full scored results for LIMO
- `s1k_scored.parquet` — Full scored results for s1K-1.1
- `openthoughts_scored.parquet` — Full scored results for OpenThoughts (5K sample)
- `medical_scored.parquet` — Full scored results for Medical-o1 (5K sample)
- `*_report.html` — Interactive HTML reports for each dataset
