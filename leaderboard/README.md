# Overthinking leaderboard

`index.html` in this directory is **generated**, never hand-edited. It is a
single self-contained page (inline CSS, no external assets, no telemetry) built
from the committed result JSONs under [`../results/`](../results):

    cargo run --release --features bench -p reasonmetrics-cli -- \
      leaderboard --results results/ --site leaderboard/

Rebuild it whenever a result JSON is added or updated, and commit the regenerated
page alongside. Output is deterministic — the page only diffs when the underlying
results change.

## How an entry gets here

Each row is one committed result JSON produced by `reasonmetrics bench`, which
embeds the exact command, the task-set sha256, and the tool version. So adding a
model to the leaderboard is a reviewable pull request:

1. Run `reasonmetrics bench` against the model's OpenAI-compatible endpoint.
2. Commit the result JSON it writes to `results/`.
3. Regenerate this page with the command above and commit it too.

See [`../docs/BENCH.md`](../docs/BENCH.md) for the metrics, the task sets, and the
submission rules; CI validates every result JSON in `results/` against the schema
(see `.github/workflows/leaderboard.yml`).

## Publishing

The page is plain HTML — host it anywhere. GitHub Pages currently serves the web
analyzer at the site root, so the leaderboard is published as a committed
artifact here rather than at the domain root; routing it to a public URL is a
deployment decision left to the maintainer.
