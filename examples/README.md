# Examples

## [curate_a_reasoning_dataset](curate_a_reasoning_dataset.ipynb)

The whole curation loop in one file: load s1K-1.1 from HuggingFace, score every trace,
filter, inspect what you dropped, save the curated JSONL. Covers the two filtering modes
(absolute percentile vs. size-exact top-N%), how to read the score without misreading it,
and the limits of what structural scoring can tell you.

```bash
pip install reasonmetrics datasets polars
python examples/curate_a_reasoning_dataset.py     # or open the .ipynb
```

The `.py` is the source of truth and the `.ipynb` is generated from it, so the code in
the notebook is code that has actually been run:

```bash
jupytext --to notebook --output examples/curate_a_reasoning_dataset.ipynb \
    examples/curate_a_reasoning_dataset.py
```

If you change one, regenerate the other in the same commit.
