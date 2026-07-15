#!/usr/bin/env python3
"""Deterministically generate the `overthinking-v2` benchmark task set.

The set is 100 simple, unambiguous, machine-checkable reasoning tasks in the
spirit of LLMThinkBench (arXiv 2507.04023, MIT-licensed): problems a competent
model dispatches in a sentence or two, but that tempt weak reasoning models into
long "wait, let me reconsider" detours. Every answer is a single integer or a
single word so the bench's normalized answer match (numeric or string) can grade
it without a parser.

The generator is deterministic (fixed seed, fixed category order), so anyone can
regenerate `benchsets/overthinking-v2.jsonl` byte-for-byte and confirm the
sha256 the bench embeds in every result. These tasks are our own authored
instances, not copied from the LLMThinkBench repo; the *shape* is inspired by it.

    python3 scripts/gen_benchset.py > crates/reasonmetrics-cli/benchsets/overthinking-v2.jsonl
"""

import json
import random
import sys

SEED = 20260715
PER_CATEGORY = 10


def compact(obj):
    # Match the byte style of overthinking-v1: no spaces after ',' or ':'.
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))


def gen(rng):
    tasks = []

    def emit(problem, answer):
        tasks.append((problem, str(answer)))

    # 1. Addition
    for _ in range(PER_CATEGORY):
        a, b = rng.randint(11, 89), rng.randint(11, 89)
        emit(f"What is {a} + {b}?", a + b)

    # 2. Subtraction (non-negative)
    for _ in range(PER_CATEGORY):
        a, b = rng.randint(40, 99), rng.randint(1, 39)
        emit(f"What is {a} - {b}?", a - b)

    # 3. Multiplication
    for _ in range(PER_CATEGORY):
        a, b = rng.randint(3, 19), rng.randint(3, 19)
        emit(f"What is {a} multiplied by {b}?", a * b)

    # 4. Clean integer division
    for _ in range(PER_CATEGORY):
        b, q = rng.randint(3, 12), rng.randint(3, 12)
        emit(f"What is {b * q} divided by {b}?", q)

    # 5. Comparison (answer is the larger value)
    for _ in range(PER_CATEGORY):
        a, b = rng.randint(100, 999), rng.randint(100, 999)
        while a == b:
            b = rng.randint(100, 999)
        emit(f"Which is larger, {a} or {b}?", max(a, b))

    # 6. Parity (even / odd)
    for _ in range(PER_CATEGORY):
        n = rng.randint(100, 999)
        emit(f"Is {n} even or odd?", "even" if n % 2 == 0 else "odd")

    # 7. Count the odd numbers in a short list
    for _ in range(PER_CATEGORY):
        nums = [rng.randint(1, 99) for _ in range(5)]
        odds = sum(1 for x in nums if x % 2 == 1)
        emit(f"How many odd numbers are in the list {nums}?", odds)

    # 8. k-th smallest of a short list of distinct numbers
    for _ in range(PER_CATEGORY):
        nums = rng.sample(range(1, 99), 5)
        k = rng.randint(1, 5)
        ordinal = {1: "smallest", 2: "2nd smallest", 3: "3rd smallest",
                   4: "4th smallest", 5: "largest"}[k]
        answer = sorted(nums)[k - 1]
        emit(f"What is the {ordinal} number in the list {nums}?", answer)

    # 9. Remainder
    for _ in range(PER_CATEGORY):
        b = rng.randint(3, 12)
        a = rng.randint(b + 1, 99)
        emit(f"What is the remainder when {a} is divided by {b}?", a % b)

    # 10. Simple percentage (clean)
    for _ in range(PER_CATEGORY):
        p = rng.choice([5, 10, 20, 25, 50])
        base = rng.randint(2, 20) * (100 // p)
        emit(f"What is {p}% of {base}?", base * p // 100)

    return tasks


def main():
    rng = random.Random(SEED)
    tasks = gen(rng)
    assert len(tasks) == 100, len(tasks)
    for i, (problem, answer) in enumerate(tasks, start=1):
        rec = {"id": f"ov2-{i:03d}", "problem": problem, "expected_answer": answer}
        sys.stdout.write(compact(rec) + "\n")


if __name__ == "__main__":
    main()
