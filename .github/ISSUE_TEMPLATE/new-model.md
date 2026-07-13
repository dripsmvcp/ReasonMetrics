---
name: New model / lexicon
about: Request or claim support for a model family's trace format, cost data, or language lexicon
title: "registry: add <model-or-language>"
labels: ["registry", "good first issue"]
---

**Model family / language:**

**What it needs** (tick all that apply):
- [ ] Think-tag or reasoning-field extraction (`registry/<id>.toml` + fixture)
- [ ] Cost table entry (with source + date)
- [ ] Restart/verification lexicon for a language (`[lexicon.<lang>]`)

**Sample output** (paste a short, real trace excerpt showing how the model
delimits its reasoning — this becomes the fixture):

```
```

**Notes** (API field names, format quirks, pricing source):

> How-to: see "Add a model family in 30 minutes" in CONTRIBUTING.md.
> A PR without a passing fixture fails CI by design.
