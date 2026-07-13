#!/usr/bin/env python3
"""
ReasonMetrics LLM Judge — semantic correctness scoring for reasoning traces.

Complements ReasonMetrics's structural scoring with LLM-based evaluation
of logical validity, factual correctness, and answer quality.

Quickstart (pick one provider):

    # Groq  (fast, free tier available)
    export GROQ_API_KEY=gsk_...
    python3 scripts/llm_judge.py --provider groq traces.jsonl

    # OpenRouter  (200+ models, pay-per-token)
    export OPENROUTER_API_KEY=sk-or-...
    python3 scripts/llm_judge.py --provider openrouter traces.jsonl

    # OpenAI
    export OPENAI_API_KEY=sk-...
    python3 scripts/llm_judge.py --provider openai traces.jsonl

    # Local Ollama
    python3 scripts/llm_judge.py --provider ollama traces.jsonl

    # Any OpenAI-compatible endpoint
    python3 scripts/llm_judge.py traces.jsonl \\
        --api-url https://my-endpoint.com/v1 --api-key $KEY --model my-model

Options:
    -n 50           Sample 50 random traces (default: 20)
    -o out.jsonl    Output path (default: <input>_judged.jsonl)
    --model NAME    Override the default model for the provider
    --workers 8     Concurrent requests (default: 4)

Requirements:  pip install httpx   (or: pip install requests)
"""

from __future__ import annotations

import argparse
import json
import os
import random
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    import httpx

    def _post(url: str, headers: dict, payload: dict, timeout: float) -> dict:
        with httpx.Client(timeout=timeout) as client:
            resp = client.post(url, json=payload, headers=headers)
            resp.raise_for_status()
            return resp.json()

except ImportError:
    try:
        import requests

        def _post(url: str, headers: dict, payload: dict, timeout: float) -> dict:  # type: ignore[misc]
            resp = requests.post(url, json=payload, headers=headers, timeout=timeout)
            resp.raise_for_status()
            return resp.json()

    except ImportError:
        print("Error: install httpx or requests:\n  pip install httpx", file=sys.stderr)
        sys.exit(1)


# ── Provider presets ─────────────────────────────────────────────
# Each preset maps to: (base_url, env_var_for_key, default_model)

PROVIDERS: dict[str, tuple[str, str, str]] = {
    "groq": (
        "https://api.groq.com/openai/v1",
        "GROQ_API_KEY",
        "llama-3.3-70b-versatile",
    ),
    "openrouter": (
        "https://openrouter.ai/api/v1",
        "OPENROUTER_API_KEY",
        "meta-llama/llama-3.3-70b-instruct",
    ),
    "openai": (
        "https://api.openai.com/v1",
        "OPENAI_API_KEY",
        "gpt-4o-mini",
    ),
    "ollama": (
        "http://localhost:11434/v1",
        "",  # no key needed
        "llama3.1",
    ),
}


# ── Evaluation prompt ────────────────────────────────────────────

JUDGE_SYSTEM_PROMPT = """\
You are an expert reasoning evaluator. You will be given a problem, \
the model's step-by-step thinking trace, and the model's final answer.

Evaluate the reasoning trace on these dimensions:
1. **Logical Validity** (0-100): Are the reasoning steps logically sound? \
   Do conclusions follow from premises?
2. **Factual Correctness** (0-100): Are stated facts, calculations, and \
   domain knowledge accurate?
3. **Answer Correctness** (0-100): Is the final answer correct or \
   reasonable for the given problem?
4. **Reasoning Completeness** (0-100): Does the trace cover all necessary \
   steps without skipping critical logic?

Respond ONLY with a JSON object in this exact format:
{
  "logical_validity": <int 0-100>,
  "factual_correctness": <int 0-100>,
  "answer_correctness": <int 0-100>,
  "reasoning_completeness": <int 0-100>,
  "explanation": "<brief 1-2 sentence explanation>"
}"""

JUDGE_USER_TEMPLATE = """\
**Problem:** {problem}

**Thinking Trace:**
{thinking}

**Final Answer:** {answer}"""


# ── Data classes ─────────────────────────────────────────────────

@dataclass
class JudgeConfig:
    api_url: str
    api_key: str
    model: str
    max_tokens: int = 256
    temperature: float = 0.0
    timeout: float = 60.0


@dataclass
class JudgeResult:
    trace_id: str
    logical_validity: int
    factual_correctness: int
    answer_correctness: int
    reasoning_completeness: int
    semantic_composite: float
    explanation: str
    error: str | None = None


# ── Core logic ───────────────────────────────────────────────────

def call_judge(trace: dict[str, Any], config: JudgeConfig) -> JudgeResult:
    """Evaluate a single trace via the LLM judge."""
    trace_id = trace.get("id", "unknown")

    thinking = trace.get("thinking", "")
    if len(thinking) > 12_000:
        thinking = thinking[:6000] + "\n\n[... truncated ...]\n\n" + thinking[-4000:]

    user_msg = JUDGE_USER_TEMPLATE.format(
        problem=trace.get("problem", "N/A"),
        thinking=thinking,
        answer=trace.get("answer", "N/A"),
    )

    payload = {
        "model": config.model,
        "messages": [
            {"role": "system", "content": JUDGE_SYSTEM_PROMPT},
            {"role": "user", "content": user_msg},
        ],
        "max_tokens": config.max_tokens,
        "temperature": config.temperature,
    }

    headers = {
        "Authorization": f"Bearer {config.api_key}",
        "Content-Type": "application/json",
    }

    try:
        url = f"{config.api_url.rstrip('/')}/chat/completions"
        data = _post(url, headers, payload, config.timeout)
        content = data["choices"][0]["message"]["content"].strip()

        # Strip markdown code fences if present
        if content.startswith("```"):
            content = content.split("\n", 1)[1].rsplit("```", 1)[0].strip()

        scores = json.loads(content)

        lv = _clamp(scores.get("logical_validity", 0))
        fc = _clamp(scores.get("factual_correctness", 0))
        ac = _clamp(scores.get("answer_correctness", 0))
        rc = _clamp(scores.get("reasoning_completeness", 0))
        composite = lv * 0.25 + fc * 0.30 + ac * 0.30 + rc * 0.15

        return JudgeResult(
            trace_id=trace_id,
            logical_validity=lv,
            factual_correctness=fc,
            answer_correctness=ac,
            reasoning_completeness=rc,
            semantic_composite=round(composite, 1),
            explanation=scores.get("explanation", ""),
        )

    except Exception as e:
        return JudgeResult(
            trace_id=trace_id, logical_validity=0, factual_correctness=0,
            answer_correctness=0, reasoning_completeness=0,
            semantic_composite=0.0, explanation="", error=str(e),
        )


def _clamp(v: Any) -> int:
    try:
        return max(0, min(100, int(v)))
    except (TypeError, ValueError):
        return 0


def load_traces(path: str) -> list[dict[str, Any]]:
    """Load traces from JSONL file."""
    traces = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                traces.append(json.loads(line))
    return traces


def run_judge(
    traces: list[dict[str, Any]],
    config: JudgeConfig,
    sample_n: int | None = None,
    workers: int = 4,
    seed: int = 42,
) -> list[JudgeResult]:
    """Run the LLM judge on a sample of traces with a live progress bar."""
    if sample_n and sample_n < len(traces):
        rng = random.Random(seed)
        traces = rng.sample(traces, sample_n)

    total = len(traces)
    results: list[JudgeResult] = []
    errors = 0
    start = time.time()

    print(f"\n  Provider model : {config.model}")
    print(f"  API endpoint   : {config.api_url}")
    print(f"  Traces to judge: {total}  ({workers} concurrent workers)\n")

    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(call_judge, t, config): t.get("id", "?") for t in traces}

        for i, future in enumerate(as_completed(futures), 1):
            result = future.result()
            results.append(result)

            if result.error:
                errors += 1

            # Progress bar
            pct = i / total
            bar_len = 40
            filled = int(bar_len * pct)
            bar = "█" * filled + "░" * (bar_len - filled)
            elapsed = time.time() - start
            rate = i / elapsed if elapsed > 0 else 0

            if result.error:
                status = f"ERR {result.trace_id}"
            else:
                status = f"{result.semantic_composite:5.1f} {result.trace_id}"

            sys.stdout.write(
                f"\r  {bar} {i}/{total} ({rate:.1f}/s) │ {status:<30}"
            )
            sys.stdout.flush()

    elapsed = time.time() - start
    sys.stdout.write("\n")
    print(f"\n  Done in {elapsed:.1f}s · {errors} errors · {total - errors} scored")
    return results


def print_summary(results: list[JudgeResult]) -> None:
    """Print aggregate statistics."""
    valid = [r for r in results if r.error is None]
    if not valid:
        print("\n  No valid results to summarize.")
        return

    n = len(valid)

    print(f"\n  {'═' * 56}")
    print(f"  LLM-as-Judge Summary ({n} traces)")
    print(f"  {'═' * 56}")
    print(f"  {'Dimension':<28} {'Avg':>8} {'Min':>6} {'Max':>6}")
    print(f"  {'─' * 48}")

    for dim in ["logical_validity", "factual_correctness",
                "answer_correctness", "reasoning_completeness",
                "semantic_composite"]:
        vals = [getattr(r, dim) for r in valid]
        name = dim.replace("_", " ").title()
        print(f"  {name:<28} {sum(vals)/n:>8.1f} {min(vals):>6} {max(vals):>6}")

    # Quality distribution
    high = sum(1 for r in valid if r.semantic_composite >= 80)
    med = sum(1 for r in valid if 50 <= r.semantic_composite < 80)
    low = sum(1 for r in valid if r.semantic_composite < 50)
    print(f"\n  Quality Distribution:")
    print(f"    High (≥80): {high:>4} ({high/n*100:.1f}%)")
    print(f"    Med (50-79): {med:>4} ({med/n*100:.1f}%)")
    print(f"    Low  (<50): {low:>4} ({low/n*100:.1f}%)")


def save_results(results: list[JudgeResult], output_path: str) -> None:
    """Save results to JSONL."""
    with open(output_path, "w") as f:
        for r in results:
            f.write(json.dumps({
                "trace_id": r.trace_id,
                "logical_validity": r.logical_validity,
                "factual_correctness": r.factual_correctness,
                "answer_correctness": r.answer_correctness,
                "reasoning_completeness": r.reasoning_completeness,
                "semantic_composite": r.semantic_composite,
                "explanation": r.explanation,
                "error": r.error,
            }) + "\n")
    print(f"\n  Results saved → {output_path}")


# ── Auto-detect provider from environment ────────────────────────

def _auto_detect_provider() -> tuple[str | None, str | None, str | None]:
    """Try to auto-detect provider from environment variables."""
    for name, (url, env_var, default_model) in PROVIDERS.items():
        if env_var and os.environ.get(env_var):
            return name, url, os.environ[env_var]
    return None, None, None


# ── CLI ──────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(
        description="ReasonMetrics LLM Judge — semantic scoring for reasoning traces",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("input", help="JSONL file with traces to judge")
    parser.add_argument("-p", "--provider",
                        choices=list(PROVIDERS.keys()),
                        default=None,
                        help="API provider preset (groq, openrouter, openai, ollama)")
    parser.add_argument("-n", "--sample", type=int, default=20,
                        help="Number of traces to sample (default: 20)")
    parser.add_argument("-o", "--output", default=None,
                        help="Output JSONL path")
    parser.add_argument("--api-url", default=None,
                        help="Custom API base URL (overrides provider)")
    parser.add_argument("--api-key", default=None,
                        help="API key (overrides env var)")
    parser.add_argument("--model", default=None,
                        help="Model name (overrides provider default)")
    parser.add_argument("--workers", type=int, default=4,
                        help="Concurrent API requests (default: 4)")
    parser.add_argument("--seed", type=int, default=42,
                        help="Random seed for sampling")

    args = parser.parse_args()

    # ── Resolve provider config ──────────────────────────────────
    api_url = args.api_url
    api_key = args.api_key
    model = args.model

    if args.provider:
        # Explicit --provider flag
        preset_url, env_var, default_model = PROVIDERS[args.provider]
        api_url = api_url or preset_url
        api_key = api_key or os.environ.get(env_var, "") if env_var else (api_key or "ollama")
        model = model or default_model
    else:
        # Auto-detect from env vars
        detected, detected_url, detected_key = _auto_detect_provider()
        if detected:
            _, _, default_model = PROVIDERS[detected]
            api_url = api_url or detected_url
            api_key = api_key or detected_key
            model = model or default_model
            print(f"  Auto-detected provider: {detected}")
        else:
            # Fallback to generic OpenAI env vars
            api_url = api_url or os.environ.get("OPENAI_API_BASE", "https://api.openai.com/v1")
            api_key = api_key or os.environ.get("OPENAI_API_KEY", "")
            model = model or "gpt-4o-mini"

    if not api_key:
        print("\n  Error: No API key found.\n", file=sys.stderr)
        print("  Set one of these environment variables:", file=sys.stderr)
        print("    export GROQ_API_KEY=gsk_...          (Groq — free tier)", file=sys.stderr)
        print("    export OPENROUTER_API_KEY=sk-or-...  (OpenRouter)", file=sys.stderr)
        print("    export OPENAI_API_KEY=sk-...         (OpenAI)", file=sys.stderr)
        print("\n  Or use: --provider ollama  (no key needed)\n", file=sys.stderr)
        sys.exit(1)

    config = JudgeConfig(api_url=api_url, api_key=api_key, model=model)

    # ── Load and run ─────────────────────────────────────────────
    traces = load_traces(args.input)
    print(f"\n  Loaded {len(traces)} traces from {args.input}")

    results = run_judge(traces, config, sample_n=args.sample,
                        workers=args.workers, seed=args.seed)

    print_summary(results)

    output_path = args.output or str(Path(args.input).stem) + "_judged.jsonl"
    save_results(results, output_path)


if __name__ == "__main__":
    main()
