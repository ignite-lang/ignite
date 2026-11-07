---
format: aep.planning-md/3
id: review-result:data-formats-critic-scope-1
kind: review-result
status: active
title: Plan critic (scope), data-formats, round 1
relations:
- reviews: epic:data-formats
- reviews: story:data-value-model
- reviews: story:data-derived-codec
- reviews: story:data-yaml
- reviews: story:data-toml
- reviews: story:data-adopters
revision: 1
---
needs-revision
epic:data-formats — the outcome promises codecs "on both targets and in the evaluator", but only `story:data-derived-codec` runs anything on the evaluator, and no story runs a format module (`parse`/`emit` of json, yaml or toml) there. Add an evaluator differential to each format story, or name the omission under Not covered. Design goal 2 requires the format modules to run in the evaluator — `.engineering/planning/epic/data-formats.md:13`
story:data-yaml — design 6.2 lists "several documents in one stream (`parse_all`)", and the outcome and acceptance never mention `parse_all`. The promise is dropped without a record; add it to the outcome and acceptance, or record the drop — `.engineering/planning/story/data-yaml.md:16` (design `docs/serde.md:209-210`)
story:data-derived-codec — design O1 fixes the default that `decode` without a fixing type annotation "require[s] an annotated `let`; a diagnostic names it". The story's outcome and acceptance have no such diagnostic and no fixture for it — `.engineering/planning/story/data-derived-codec.md:16` (design `docs/serde.md:267`)
epic:data-formats — design section 6 says "A format declares `conforms` against a catalogue in `std/rfc/` … so `ignite conformance` reports its coverage", and no story claims a catalogue or a `conforms` claim for json, yaml or toml. The epic's Not covered section does not exclude it either. `story:data-yaml` and `story:data-toml` would take it — `docs/serde.md:197-198`

What I read: the epic, the 5 stories in full, `docs/serde.md` in full, and the graph via `aep plan artifact graph`. I ran `git fetch` and `git merge --ff-only origin/main`; main was already at 459b964c.

Promise count: I extracted 14 promises from the epic and the design sections it points to. I traced 10 to an item (the other 4 are the findings above). The epic's Not covered section excludes sections 7 and 9. There is no reach beyond the parent and no two items claiming one outcome. `data-adopters` traces to design step 5 (`docs/serde.md:248`).

Not established:
- Out of my lane, `plan-critic-design`: the design (`docs/serde.md:253`) refers to `docs/quality.md`, which does not exist in the tree, so "adds rows to the quality ratchet" has nothing to attach to.
- Out of my lane, acceptance: `data-derived-codec` has no fixture for the Map-key diagnostic ("another key type is E1xx at check", `docs/serde.md:154`) or for `Duration`.
- Out of my lane, design: `data-adopters` scopes `ignite-lsp/src/lsp_types.ignite`, but AGENTS.md puts ignite-lsp in a separate repository. I did not check whether the path resolves from here.
- Out of my lane, parallel-safety: `data-derived-codec` and `data-value-model` both edit `std/data.ignite`. The `data-derived-codec` body already notes the `mid-level-ir` overlap.

```findings
- file: .engineering/planning/epic/data-formats.md
  line: 13
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome promises codecs 'on both targets and in the evaluator', but only story:data-derived-codec runs on the evaluator and no story runs any format module's parse/emit there; add an evaluator differential to each format story or name the omission under Not covered"
- file: .engineering/planning/story/data-yaml.md
  line: 16
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "docs/serde.md:209-210 lists several documents in one stream (parse_all) as part of std.yaml, and the story's outcome and acceptance never mention parse_all, so the promise is dropped without a record"
- file: .engineering/planning/story/data-derived-codec.md
  line: 16
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "design O1 sets the default that decode without a fixing annotation requires an annotated let with a diagnostic naming it (docs/serde.md:267), and the story's outcome and acceptance have no such diagnostic or fixture"
- file: .engineering/planning/epic/data-formats.md
  line: 13
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "docs/serde.md:197-198 promises each format declares conforms against a std/rfc catalogue so ignite conformance reports coverage, and no story (natural owners data-yaml and data-toml) claims it nor does Not covered exclude it"
```
