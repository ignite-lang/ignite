---
format: aep.planning-md/3
id: review-result:data-formats-critic-scope-2
kind: review-result
status: active
title: Plan critic (scope), data-formats, round 2
relations:
- reviews: epic:data-formats
- reviews: story:data-value-model
- reviews: story:data-codec-combinators
- reviews: story:data-derived-codec
- reviews: story:data-yaml
- reviews: story:data-toml
- reviews: story:data-adopters
revision: 1
---
needs-revision
epic:data-formats — design 5.1 promises a compile-time check of `renames` ("`renames` entries are checked at compile time when the `Shape` is a constant: a name that is no field or variant of a type the call reaches is a diagnostic"). No story claims it: `story:data-codec-combinators` gives only a run-time `DecodeError`, and the diagnostics list of `story:data-derived-codec` omits it. The epic's Not covered does not exclude it. Add the diagnostic and a fixture to `story:data-derived-codec`, or name it under Not covered — `docs/serde.md:210-212`; `.engineering/planning/story/data-derived-codec.md:23`

What I read: the epic, the six stories in full, `docs/serde.md` in full, round 1 (`review-result:data-formats-critic-scope-1`) and the `graph` output filtered to the data artifacts. I ran `git pull --ff-only origin main` (already up to date), `aep plan artifact show` on each id and `aep plan artifact graph`.

Round 1: all four findings landed.
- **Evaluator coverage:** the epic acceptance line 3, plus an evaluator differential in `data-value-model`, `data-yaml`, `data-toml` and `data-codec-combinators`.
- **`read_all`:** now in the `data-yaml` outcome and acceptance.
- **O1 diagnostic:** now in `data-derived-codec` acceptance line 23.
- **`conforms`:** now under the epic's Not covered, matching `docs/serde.md:294`.

Promise count: I extracted 17 promises from the epic and the design sections it points to. I traced 16 to an item. The 17th is the finding above. There is no reach beyond the parent and no two items claiming one outcome.

Not established:
- Out of my lane (acceptance): `data-yaml` and `data-toml` name no fixture for "a duplicate key is a parse error" (`docs/serde.md:167`) or for `!!binary` reading.
- Out of my lane (design): `docs/quality.md` still does not exist, and `data-value-model` hedges with "where it exists".

```findings
- file: .engineering/planning/story/data-derived-codec.md
  line: 23
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "docs/serde.md:210-212 promises that renames entries of a constant Shape are checked at compile time (a name that is no field or variant of a reached type is a diagnostic), and no story claims it: data-codec-combinators gives only a run-time DecodeError and this story's diagnostics list omits it, while the epic's Not covered does not exclude it"
```
