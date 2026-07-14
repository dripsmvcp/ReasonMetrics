# Known Limitations

ReasonMetrics scores **structure, not meaning**. The nine dimensions are fast, deterministic
heuristics — a lens for triaging traces, not ground truth about reasoning quality. This page
documents, concretely and reproducibly, where the heuristics fail: every failure mode below is
pinned by an adversarial fixture in `crates/reasonmetrics-core/tests/fixtures/adversarial/`, and
a CI test asserts the gaming still works. If a scorer improvement closes a hole, that test fails
on purpose and this page gets updated with it.

## Failure modes, demonstrated

Observed with default config; each fixture is a deliberately bad trace engineered to fool one scorer.

"Gamed score" is the raw score of the dimension the fixture attacks — the hole itself.
"Composite" is the calibrated percentile the whole trace ends up with, i.e. what a user filters on.

| Gamed dimension | How | Fixture | Gamed score | Composite |
|---|---|---|---|---|
| Efficiency | Restarts phrased in novel words ("scratch that whole approach") dodge the fixed phrase list — six abandoned attempts read as zero | `efficiency-paraphrased-restarts` | **100** | 6.7 |
| Self-verification | Verification *phrases* with no verification *act* ("Let me verify: yes, definitely correct") get full credit | `verification-hollow` | **100** | 0.8 |
| Repetition | The same idea paraphrased six ways is invisible to normalized-exact dedup — classic model spinning passes clean | `repetition-paraphrase-spin` | **100** | 0.4 |
| Language consistency | Detection is one-language-per-chunk, so bilingual mixing *inside* every chunk (zh clauses woven into en sentences) passes as monolingual | `language-cjk-inline` | **100** | 0.3 |
| Structural clarity | Content-free sentences dressed in "Step 1 / therefore / thus" scaffolding earn full structural credit | `structure-cargo-cult` | **100** | **50.3** |
| Answer alignment | A wandering trace with "Therefore, the answer is 42" bolted on the end reads as converged | `alignment-bolted-conclusion` | 77 | 3.2 |
| Overthinking | Problem complexity is largely length-based: padding a trivial question with narrative quadruples the allowed thinking budget (expected_max ~500 → ~2,100 words) | `overthinking-padded-problem` | **100** | 0.7 |

Every dimension score above is a **raw** score, and every one of these holes is still open:
each fixture still games the dimension it targets, exactly as it did before. The scorers have
not been made harder to fool.

## What this means in practice

- **Each scorer is still individually gameable — but the composite no longer waves the garbage
  through.** On the calibrated scale (see [CALIBRATION.md](CALIBRATION.md) and
  [#30](https://github.com/dripsmvcp/ReasonMetrics/issues/30)) **0 of the 7 fixtures survive
  `reasonmetrics filter --min-score 70`**, the documented default. Before calibration, **5 of 7
  did** — not because they were convincing, but because the raw scale put 99.9% of *all* real
  traces above 70, so the threshold excluded nothing.

  Be precise about what changed: the *threshold* became meaningful, the *scorers* did not get
  smarter. `structure-cargo-cult` still lands at the 50th percentile and would sail through
  `--min-score 50`. A trace engineered to game **several** dimensions at once would still score
  well. Treat filtering as removing the worst traces, not as certifying the survivors.
- **Adversarial robustness is a non-goal for the heuristic tier.** These scorers are built for
  honest-model output at native speed, not for inputs optimizing against the metric. If your
  traces might be adversarial (e.g., generated to pass a quality gate), pair the scores with the
  [LLM judge](../README.md#llm-as-judge-optional-semantic-scoring) for semantic spot-checks.
- **Semantic correctness is out of scope by design.** A fluent, well-structured, confidently
  verified trace that is *wrong* scores well on all nine dimensions. Use `expected_answer` +
  the accuracy-efficiency scorer, or the LLM judge, when correctness matters.
- **Language coverage is uneven.** Restart/verification phrase lists are strongest in English
  with partial CJK; see the registry lexicon issues for per-language contributions, and issue #3
  for the CJK word-count gap.

## Found a new one?

That's a contribution we actively want: file a **miscalibration report** (issue template) with the
trace and the score you expected vs got. The best reports become fixtures in the adversarial suite.
