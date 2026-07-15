# Benchmark results

Each `*.json` here is one committed run of `reasonmetrics bench` — a leaderboard
entry. They feed both `reasonmetrics leaderboard` (the assembled table) and the
generated [`../leaderboard/`](../leaderboard) site.

## Submitting a result (it's a pull request)

1. Run the benchmark against your model's OpenAI-compatible endpoint:

       reasonmetrics bench \
         --endpoint http://localhost:11434/v1 \
         --model my-model \
         --task-set overthinking-v2 \
         --temperature 0

2. Commit the JSON it writes here (default path
   `results/<task-set>-<model>-<hash>.json`).
3. Regenerate the leaderboard page and commit it too:

       reasonmetrics leaderboard --results results/ --site leaderboard/

4. Open a PR. CI runs `reasonmetrics leaderboard --results results/ --strict`;
   it must pass.

## What CI enforces

`--strict` validation (see `.github/workflows/leaderboard.yml`) rejects a result
that:

- is not valid result JSON, or carries an unsupported `schema_version`;
- has an empty `model` or `task_set.name`;
- has a `task_set.sha256` that is not 64 lowercase hex chars;
- reports `accuracy` outside `[0,1]` or `mean_quality` outside `[0,100]`;
- has `samples < 1`, or `n_scored` greater than the task set size;
- **claims a bundled task set but carries a different sha256** — results must be
  run against the exact frozen problem set, never a modified one.

The tool embeds the exact command, task-set hash, and tool version in every
result, so a reviewer can see precisely how each number was produced. Numbers
are self-reported from your endpoint; the sha256 guarantees the *problems* are
the committed ones, not that the *answers* weren't tampered with — treat entries
with the same scepticism as any self-reported benchmark.
