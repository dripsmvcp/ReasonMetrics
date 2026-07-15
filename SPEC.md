# ReasonMetrics Specification

**Version 1.0.0** · status: **frozen**

This document is the normative contract for the ReasonMetrics trace schema and
scoring semantics, so other tools can produce and consume compatible data. It is
versioned with [semver](#versioning); the reference implementation is this
repository at the crate version that declares `spec = "1.0.0"` (currently
`0.2.0`). Where prose and code disagree, this document is authoritative for the
*contract*; the code is authoritative for exact numeric output.

Terminology: **MUST**/**SHOULD**/**MAY** per RFC 2119.

---

## 1. Trace record (input)

A trace is a JSON object. Producers MUST emit `problem`, `thinking`, and
`answer`; all other fields are optional. Consumers MUST accept the documented
aliases and MUST ignore (or round-trip) unknown fields.

| Field | Type | Required | Aliases | Meaning |
|---|---|---|---|---|
| `id` | string | no (defaults to a generated id) | `idx`, `index`, `uuid` | Stable identifier. A JSON number is coerced to its string form. |
| `problem` | string | **yes** | `question`, `prompt`, `query`, `input` | The task posed to the model. |
| `thinking` | string | **yes** | `reasoning`, `chain_of_thought`, `cot`, `thought` | The reasoning trace. May contain `<think>`/`<reasoning>` tags (see §2.1). |
| `answer` | string | **yes** | `solution`, `response`, `output`, `result` | The final answer. |
| `domain` | string \| null | no | — | Optional domain label (e.g. `math`, `code`). Not used by scoring in v1. |
| `source` | string \| null | no | — | Optional dataset name. Not used by scoring in v1. |
| `expected_answer` | string \| null | no | `ground_truth`, `label`, `target` | Ground truth. Its presence enables the `accuracy_efficiency` dimension (§3). |

Unknown top-level fields are preserved under an `extra` map by the reference
implementation and MUST NOT cause rejection.

JSONL (one object per line) is the canonical batch format. A single object is
also valid.

### 1.1 Thinking extraction

Before scoring, the reasoning text is extracted from `thinking` (or, when a raw
completion is scored, from the whole text): the content between the first
`<think>`/`<reasoning>` open tag and its matching close tag. When no such tags
are present, the entire `thinking` string is the reasoning text.

---

## 2. Scored trace (output)

Scoring a trace yields a JSON object with these fields (snake_case, stable):

| Field | Type | Meaning |
|---|---|---|
| `id`, `problem`, `thinking`, `answer` | as input | Echoed through. |
| `quality_score` | number `[0,100]` | **The number to rank and filter on.** Percentile of `raw_score` against a reference corpus of real traces (§4): "better than N% of real reasoning traces." |
| `raw_score` | number `[0,100]` | The weighted composite before calibration (§3). Monotonic with `quality_score`. |
| `efficiency_score` | number `[0,100]` | Dimension 0 (§3). |
| `language_score` | number `[0,100]` | Dimension 1. |
| `answer_alignment_score` | number `[0,100]` | Dimension 2. |
| `structural_score` | number `[0,100]` | Dimension 3. |
| `repetition_score` | number `[0,100]` | Dimension 4. |
| `overthinking_score` | number `[0,100]` | Dimension 5. |
| `verification_score` | number `[0,100]` | Dimension 6. |
| `length_score` | number `[0,100]` | Dimension 7. |
| `thinking_word_count` | integer | Estimated token/word count of the reasoning. |
| `restart_count` | integer | Detected "wait, let me start over" restarts. |
| `detected_language` | string | Dominant language code of the reasoning. |
| `has_self_verification` | boolean | Whether a self-checking pass was detected. |
| `is_language_mixed` | boolean | Whether the reasoning mixes languages. |
| `answer_in_trace_end` | boolean | Whether the final answer appears at the end of the reasoning. |

The per-dimension scores are **diagnostics**: they are NOT individually
calibrated, and several saturate on real data (e.g. `language_score` is exactly
100 for the large majority of real traces). Do not percentile them.

The `accuracy_efficiency` dimension (§3, index 8) contributes to `raw_score`
only; it is not emitted as its own output field in v1.

---

## 3. Scoring semantics

`raw_score` is the weighted sum of nine dimension scores, each in `[0,100]`,
with weights that MUST be finite and sum to `1.0 ± 0.001`:

```
raw_score = clamp(Σ  wᵢ · scoreᵢ , 0, 100)      for i in 0..=8
```

The canonical dimension order and the default weights are frozen for v1:

| # | Dimension | Default weight | What it rewards |
|---|---|---|---|
| 0 | `efficiency` | 0.20 | Signal per token; penalizes padding. |
| 1 | `language` | 0.12 | Staying in one language. |
| 2 | `alignment` | 0.18 | The reasoning actually reaching the stated answer. |
| 3 | `structure` | 0.10 | Legible structure (steps, delimiters). |
| 4 | `repetition` | 0.15 | Not repeating passages. |
| 5 | `overthinking` | 0.10 | Not looping/second-guessing without progress. |
| 6 | `verification` | 0.08 | Checking its own work. |
| 7 | `length` | 0.07 | Length in a sensible band for the task. |
| 8 | `accuracy_efficiency` | 0.00 | Correct **and** concise — requires `expected_answer`. Off by default so unlabeled datasets keep the composite; raising it MUST be accompanied by rebalancing the others to sum to 1. |

Weights and the per-dimension algorithms MAY be overridden via configuration,
but a producer claiming SPEC 1.0.0 output MUST use the default weights and the
reference algorithms — otherwise `raw_score` and `quality_score` are not
comparable across producers.

---

## 4. Calibration

`quality_score = calibrate(raw_score)` maps the raw composite to its percentile
in a fixed **reference distribution of 2,517 real reasoning traces**. The
mapping is:

- **monotone non-decreasing** — it never reorders traces, so ranking and
  filtering by `quality_score` are identical to ranking by `raw_score`;
- **clamped**: a `raw_score` at or below the reference minimum maps to 0, at or
  above the maximum maps to 100;
- defined by a frozen 101-knot quantile table (percentiles 0..100).

Motivation: the raw composite is crushed into the top of its range (on the
reference corpus it runs ≈66.6..100 with a median of ≈85.3, and 99.9% of real
traces exceed 70), which made raw-scale thresholds useless. See
[docs/CALIBRATION.md](docs/CALIBRATION.md).

The reference table is part of the spec version: **changing it changes published
`quality_score` values and is a breaking change** (§6).

---

## 5. Bench result artifact

A benchmark run emits a separate, independently versioned JSON artifact carrying
its own `schema_version` field (currently `"1"`). Its schema, metrics, and
integrity rules (including that a bundled task set MUST carry that set's frozen
sha256) are specified in [docs/BENCH.md](docs/BENCH.md) and enforced by
`reasonmetrics leaderboard --strict`. The bench schema version is orthogonal to
this document's version.

---

## 6. Versioning

This spec is semver'd. Given `MAJOR.MINOR.PATCH`:

- **MAJOR** — any change that alters the score of an existing trace or breaks a
  consumer: removing/renaming a field, changing a field's meaning, changing a
  dimension algorithm, the composite formula, the default weights, or the
  calibration reference table.
- **MINOR** — backward-compatible additions: a new optional input field or
  alias, a new output diagnostic field, or a new dimension introduced at weight
  0.0 (so existing scores are unchanged).
- **PATCH** — clarifications and fixes that do not change any output.

A producer MUST declare which spec version it implements. A consumer MUST accept
any artifact whose MAJOR matches and whose MINOR is ≤ the one it knows, ignoring
unknown additive fields.

### Changelog

- **1.0.0** — Initial frozen release: trace schema, nine-dimension composite with
  the default weights above, and percentile calibration against the 2,517-trace
  reference corpus.
