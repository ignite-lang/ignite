---
format: aep.planning-md/3
id: review-result:data-formats-critic-parallel-2
kind: review-result
status: active
title: Plan critic (parallel), data-formats, round 2
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
story:data-value-model — frontmatter has no `scope`, so `aep plan artifact waves` lists it under `unassessed` and never places it; the Scope prose is not machine-read, so the shared-file collisions the bodies name cannot be derived into waves — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-value-model.md:1-10
story:data-codec-combinators — same: no `scope` frontmatter, `unassessed`; it shares `docs/slice.md`, the `AGENTS.md` layout table and `crates/ignite/tests/main.rs` with yaml and toml, so a recorded scope is needed for the wave tool to keep them apart — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-codec-combinators.md:1-10
story:data-yaml — same: no `scope` frontmatter, `unassessed`, never placed; its shared files (`docs/layering.md`, `layering.rs`, `docs/slice.md`, `AGENTS.md`, `main.rs`) exist only as prose — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-yaml.md:1-10
story:data-toml — same: no `scope` frontmatter, `unassessed`, never placed, and the same shared files exist only as prose — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-toml.md:1-10
story:data-derived-codec — same: no `scope` frontmatter, `unassessed`; it also names `crates/ignite-types`, which story:mid-level-ir (scope recorded, inferred) holds, and the "not in one wave" rule is prose only — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-derived-codec.md:1-10
story:data-adopters — same: no `scope` frontmatter, `unassessed`; `examples/todo/todo.ignite` and `crates/ignite/tests/main.rs` are recorded nowhere the wave tool reads — aep plan artifact waves (unassessed list) and .engineering/planning/story/data-adopters.md:1-10

The round 1 findings are fixed. Yaml, toml and combinators now name the shared files and the layering rows. Derived-codec names `docs/layering.md` and `AGENTS.md`. The epic names the shared-file wave. The `depends_on` edges order every file collision I traced, so no pair collides unnamed. Both remedies for round 1 hold in the prose.

**What you read:** 7 artifacts. I ran `aep plan artifact show` on the epic, the six stories and review-result:data-formats-critic-parallel-1. I also ran `aep plan artifact waves` and `aep plan artifact --help`, and read `docs/layering.md`, the `LAYERS` table of `crates/ignite-check/tests/layering.rs`, and the story files' frontmatter. Main is at 4b301935.

Surfaces: 6 of 6 stories placed cited in the body prose, 0 inferred, 0 unplaceable by prose. None is recorded in the `scope` field, so the wave tool places 0 of 6. I traced 9 acceptance reads to a producer in the set. They are yaml, toml and combinators reading value-model, derived-codec reading value-model and combinators, and adopters reading derived-codec, yaml, toml and combinators. All 9 have a `depends_on` edge.

**What you could not establish:**
- Whether `std/data.ignite` is touched by anything outside the set. `std/data.ignite`, `std/yaml.ignite` and `std/toml.ignite` do not exist yet.
- The ignite-lsp repository at `v0.2.0`, which adopters reads, is outside this tree, and nothing in the set produces it.
- The test-file names under `crates/ignite/tests/` that yaml and toml create are not given. Each is "a test file", so two stories could pick one name. This is a weak concern, so I do not list it as a finding.
- The unclear `yaml/yaml-test-suite` grep acceptance and whether the split is right are outside my lane.

```findings
- file: .engineering/planning/story/data-value-model.md
  line: 10
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so aep plan artifact waves lists the story as unassessed and never places it; record the Scope paths with aep plan artifact scope (cited)"
- file: .engineering/planning/story/data-codec-combinators.md
  line: 10
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so the story is unassessed and the shared docs/slice.md, AGENTS.md and crates/ignite/tests/main.rs with yaml and toml are not derivable into waves; record the Scope paths (cited)"
- file: .engineering/planning/story/data-yaml.md
  line: 10
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so the story is unassessed and never placed; its shared layering, slice.md, AGENTS.md and main.rs paths exist only as prose; record them (cited)"
- file: .engineering/planning/story/data-toml.md
  line: 10
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so the story is unassessed and never placed; its shared layering, slice.md, AGENTS.md and main.rs paths exist only as prose; record them (cited)"
- file: .engineering/planning/story/data-derived-codec.md
  line: 10
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so the story is unassessed; crates/ignite-types is held by story:mid-level-ir (scope inferred) and the not-in-one-wave rule is prose only; record the Scope paths (cited)"
- file: .engineering/planning/story/data-adopters.md
  line: 10
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "frontmatter has no scope, so the story is unassessed and never placed; record examples/todo/todo.ignite and the shared crates/ignite/tests/main.rs (cited)"
```
