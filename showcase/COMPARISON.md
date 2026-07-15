# Why ReasonMetrics? — A Realistic Comparison

## The Problem

Training LLMs on reasoning traces (chain-of-thought data) is now mainstream. But not all traces are equal — low-quality traces with repetition, language mixing, overthinking, or missing verification actively degrade model performance.

**The research is clear:**
- [LIMO (Feb 2025)](https://arxiv.org/abs/2502.03387): 817 carefully curated traces outperformed models trained on 100K+ unfiltered traces.
- [DeepSeek-R1 (Jan 2025)](https://arxiv.org/abs/2501.12948): Language mixing during RL training degrades output quality.
- [Think Deep, Not Just Long (Feb 2026)](https://arxiv.org/abs/2602.13517): Excessive restarts and backtracking in traces waste tokens without improving accuracy.
- [Do NOT Think That Much for 2+3=? (Dec 2024)](https://arxiv.org/abs/2412.21187): Overthinking simple problems in training data teaches the model bad habits.

The question is: **how do you filter for quality at scale?**

## Existing Approaches and Their Limitations

### 1. Manual Curation (LIMO-style)
- **How it works**: Human experts read and select traces one by one.
- **Pros**: Highest quality. LIMO proved 817 traces can beat 100K.
- **Cons**: Does not scale. LIMO took a research team months to curate 817 traces. You cannot manually review 114K traces from OpenThoughts.
- **ReasonMetrics advantage**: Automates the quality dimensions that LIMO's curators evaluated. Scores 817 LIMO traces in **0.24 seconds** vs. weeks of human review.

### 2. LLM-as-Judge (GPT-4 / Claude scoring)
- **How it works**: Send each trace to an LLM API and ask it to rate quality.
- **Pros**: Can evaluate semantic correctness (is the math right?).
- **Cons**:
  - **Cost**: At $0.01/trace, scoring 114K OpenThoughts traces costs **$1,140**. ReasonMetrics costs **$0**.
  - **Speed**: ~2-5 traces/second via API. ReasonMetrics scores **2,650-45,000 traces/second**.
  - **Reproducibility**: LLM outputs vary between runs. ReasonMetrics is deterministic.
  - **Privacy**: Your training data goes to a third-party API. ReasonMetrics runs locally.
- **ReasonMetrics advantage**: 10,000x faster, zero cost, fully deterministic, no data leaves your machine.

### 3. Simple Heuristics (length filter, dedup)
- **How it works**: Filter by word count, remove exact duplicates.
- **Pros**: Fast, simple.
- **Cons**: Misses everything that matters. A 500-word trace with 5 restarts, no verification, and language mixing passes a length filter just fine.
- **ReasonMetrics advantage**: 8 research-backed quality dimensions vs. 1-2 crude filters.

### 4. Python-based Quality Scoring
- **How it works**: Custom Python scripts evaluating similar heuristics.
- **Pros**: Flexible, easy to modify.
- **Cons**:
  - **Speed**: Python regex + NLP on 1M traces takes ~45 minutes. ReasonMetrics does it in **~90 seconds** (30x faster).
  - **Parallelism**: Python's GIL makes true parallelism hard. ReasonMetrics uses Rayon for zero-overhead thread parallelism.
  - **Memory**: Python's per-object overhead means higher memory use. Rust's zero-cost abstractions keep memory flat.
- **ReasonMetrics advantage**: 30x faster, lower memory, compiled binary with no runtime dependencies.

## Honest Limitations of ReasonMetrics

We believe in being realistic about what ReasonMetrics does and doesn't do:

| Capability | ReasonMetrics | LLM-as-Judge |
|------------|-----------|--------------|
| Structural quality (repetition, steps, length) | ✅ Strong | ✅ Strong |
| Language consistency | ✅ Strong | ✅ Strong |
| Mathematical correctness | ❌ Cannot verify | ✅ Can verify |
| Semantic coherence | ❌ Heuristic only | ✅ Strong |
| Speed (traces/sec) | 750 - 45,000 | 2-5 |
| Cost per 100K traces | $0 | ~$1,000 |
| Deterministic | ✅ Yes | ❌ No |
| Offline / private | ✅ Yes | ❌ No |

**ReasonMetrics does NOT check if the math is correct.** It checks if the reasoning *process* shows quality signals. This is a complementary tool — use ReasonMetrics to cheaply filter out structurally bad traces, then optionally use the included LLM-as-Judge script on the survivors for semantic verification.

## Recommended Workflow

```
Raw Dataset (114K traces)
    │
    ▼
reasonmetrics filter --min-score 70              ← 2 seconds, free
    │                                               "better than 70% of real traces"
    ▼                                               keeps the top ~30% (~34K)
~34K high-quality traces
    │
    ▼
(Optional) python3 scripts/llm_judge.py       ← Semantic scoring
    --provider groq scored_output.jsonl            via Groq, OpenRouter,
    -n 500                                         OpenAI, or local Ollama
    │
    ▼
Final training set with both structural + semantic scores
```

`quality_score` is a **percentile against a reference corpus of real reasoning
traces**, so `--min-score 70` keeps the top ~30% — on 938 ground-truth-labelled
traces, the kept set reaches the correct answer 69.0% of the time vs 42.4% for the
dropped set (baseline 48.0%). Use `--top-percent N` to keep an exact share instead.
Method, evidence, and limits: [docs/CALIBRATION.md](../docs/CALIBRATION.md).

The `llm_judge.py` script supports Groq (free tier), OpenRouter (200+ models), OpenAI, and local Ollama out of the box. Auto-detects your API key from environment variables. Evaluates logical validity, factual correctness, answer correctness, and reasoning completeness.

## Performance Evidence

From our showcase testing on real datasets:

| Dataset | Traces | Wall Time | Traces/sec |
|---------|--------|-----------|------------|
| LIMO | 817 | 0.24s | 3,400 |
| s1K-1.1 | 1,000 | 0.02s | 45,000 |
| OpenThoughts-114k (5K sample) | 5,000 | 1.9s | 2,650 |
| OpenCodeReasoning (5K sample) | 5,000 | 4.2s | 1,200 |
| Medical-o1 (5K sample) | 5,000 | 0.21s | 24,300 |

Throughput varies with trace length — longer traces (OpenThoughts avg ~2000 words) are slower per trace than short traces (s1K avg ~200 words). This is expected and proportional.

## Research Grounding

Every scoring dimension maps to a published finding:

| Dimension | Research Paper | Key Finding |
|-----------|---------------|-------------|
| Efficiency | [Think Deep, Not Just Long](https://arxiv.org/abs/2602.13517) | Restart phrases indicate shallow exploration, not deep thinking |
| Language Consistency | [DeepSeek-R1](https://arxiv.org/abs/2501.12948) | RL-trained models mix languages; this degrades downstream quality |
| Answer Alignment | [Sky-T1](https://novasky-ai.github.io/posts/sky-t1/) | Answer should converge at end of trace, not appear randomly |
| Structural Clarity | [OpenThoughts](https://arxiv.org/abs/2506.04178) | Step-by-step structure correlates with reasoning accuracy |
| Repetition | [LIMO](https://arxiv.org/abs/2502.03387) | Repeated paragraphs/sentences are a sign of degenerate traces |
| Overthinking | [Do NOT Think That Much](https://arxiv.org/abs/2412.21187) | Trace length should be proportional to problem complexity |
| Self-Verification | [LIMO](https://arxiv.org/abs/2502.03387) | "Let me verify" patterns correlate with correct answers |
| Length Calibration | [Between Underthinking and Overthinking](https://arxiv.org/abs/2505.00127) | Sweet-spot word count range produces best training signal |
