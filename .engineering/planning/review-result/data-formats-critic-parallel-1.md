---
format: aep.planning-md/3
id: review-result:data-formats-critic-parallel-1
kind: review-result
status: active
title: Plan critic (parallel), data-formats, round 1
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

- story:data-yaml — shares `docs/slice.md` (a new section appended after §25), the `AGENTS.md` layout table and a `mod` line in `crates/ignite/tests/main.rs` with story:data-toml and story:data-derived-codec, which can all run in one wave after value-model, and its Scope does not say so (cited; fix by an ordering edge recording the shared files, or by splitting the surface, for example a separate docs landing) — .engineering/planning/story/data-yaml.md:32, .engineering/planning/story/data-toml.md:27, docs/serde.md:250
- story:data-yaml — `std.yaml` is a new std module outside `std.protocol.` and `std.rfc`, so layering rule 4 requires a row in `docs/layering.md` and in `LAYERS` of `crates/ignite-check/tests/layering.rs`, but the Scope omits both files and story:data-toml needs the same two edits (cited, the file `std/yaml.ignite` does not exist yet) — docs/layering.md:29, crates/ignite-check/tests/layering.rs:55-70, .engineering/planning/story/data-yaml.md:32
- story:data-toml — same collision as story:data-yaml: `docs/slice.md` section, `AGENTS.md` layout row, a `mod` line in `crates/ignite/tests/main.rs` and the layering rows for `std.toml` (`docs/layering.md`, `layering.rs`), all unnamed against story:data-yaml and story:data-derived-codec (cited; `std/toml.ignite` does not exist yet) — .engineering/planning/story/data-toml.md:27, .engineering/planning/story/data-yaml.md:32
- story:data-derived-codec — its Scope omits `docs/layering.md` and `AGENTS.md`, yet it adds a `docs/slice.md` section and a `crates/ignite-types` pass that must name `std.data` in a string literal, which layering rule 6 permits only through a row in the "Std names in the crates" table, so it collides unnamed with story:data-yaml and story:data-toml on those files (inferred for the layering row, cited for `docs/slice.md`) — .engineering/planning/story/data-derived-codec.md:29, docs/layering.md:40-45

**What you read:** 6 artifacts (epic and five stories) through `aep plan artifact show` for each, plus `aep plan artifact graph` and `relations`. I also read `docs/serde.md` §3, §4 and §8, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, the `mod` list of `crates/ignite/tests/main.rs`, and the section headings of `docs/slice.md`. Main is at 459b964c.

Surfaces: 5 stories placed cited (each Scope names paths, though the drafts label them "Inferred"), 0 inferred, 0 unplaceable. Of the 5 acceptance reads I traced to a producer in the set, 4 have a direct `depends_on` edge and 1 runs through derived-codec to value-model. They are yaml, toml and derived-codec reading value-model; adopters reading derived-codec; and adopters reading the `Json` bridge from value-model.

**What you could not establish:**
- Whether story:mid-level-ir, story:large-file-splits or other active stories outside the set land on `crates/ignite-types`, `std/json.ignite` or `examples/todo/todo.ignite`; that is outside the set.
- story:data-adopters reads `ignite-lsp/src/lsp_types.ignite`, which lives in the sibling repository `/home/timo/projects/ignite-lang/ignite-lsp`, not this tree. No item in the set produces it, so no collision follows, but its Scope does not name that repository (not set by my lane).
- yaml's acceptance "`parse(emit(v)) == v` for generated values" names no generator (acceptance lane, out of my lane).
- Whether `docs/serde.md` §8 should drop its "disjoint files" claim is a design-doc edit outside the set.

```findings
- file: .engineering/planning/story/data-yaml.md
  line: 32
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares docs/slice.md (a new section after 25), the AGENTS.md layout table and a mod line in crates/ignite/tests/main.rs with story:data-toml and story:data-derived-codec, all startable in one wave after story:data-value-model, and the body does not say so; add an ordering edge recording the shared files or split the surface (cited)"
- file: docs/layering.md
  line: 29
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "story:data-yaml adds std/yaml.ignite, a new std module that layering rule 4 requires a row for in docs/layering.md and LAYERS in crates/ignite-check/tests/layering.rs, but its Scope omits both files, which story:data-toml and story:data-value-model also edit (cited; std/yaml.ignite does not exist yet)"
- file: .engineering/planning/story/data-toml.md
  line: 27
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares docs/slice.md (a new section), the AGENTS.md layout table, a mod line in crates/ignite/tests/main.rs and the layering rows for std.toml (docs/layering.md, layering.rs) with story:data-yaml and story:data-derived-codec, and the body does not say so; add an ordering edge recording the shared files or split the surface (cited; std/toml.ignite does not exist yet)"
- file: .engineering/planning/story/data-derived-codec.md
  line: 29
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its Scope omits docs/layering.md and AGENTS.md although it adds a docs/slice.md section and a crates/ignite-types pass that names std.data in a string literal, which layering rule 6 allows only through a row in the Std names table; it collides unnamed with story:data-yaml and story:data-toml on those files (layering row inferred, docs/slice.md cited)"
```
