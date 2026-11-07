---
format: aep.planning-md/3
id: review-result:data-formats-critic-design-1
kind: review-result
status: active
title: Plan critic (design), data-formats, round 1
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
story:data-adopters — the epic outcome is typed values to YAML or TOML text and back through `encode`/`decode`, but no story crosses the codec with `yaml` or `toml`, so the only place that seam is exercised is JSON; add `depends_on` edges from this story to story:data-yaml and story:data-toml and an acceptance line that round-trips one typed record through all three formats — .engineering/planning/epic/data-formats.md:13, .engineering/planning/story/data-adopters.md:20

**What I read:** 6 artifacts (the epic and 5 stories) with `aep plan artifact show`, plus `relations`, `graph` and `validate`, `docs/serde.md`, `docs/layering.md`, `std/json.ignite` and `examples/todo/todo.ignite`.
- The set has 9 edges, 4 `depends_on` and 5 `decomposes`, and none run outside it. The 337-edge graph has no other edge touching these ids, and there is no cycle.
- I traced 9 acceptance reads to a producer in the set, and 8 have a `depends_on` edge, direct or through the chain.
- The ninth is the typed-record-through-yaml/toml read behind the finding above.
- The edges are value-model, then derived-codec, yaml and toml in parallel, then adopters after derived-codec. The set is not a serialising chain, and each story can be shown working on its own.
- `validate` reports `valid`. Its only complaints are 7 old review-results without a findings block, which are not part of this set.

**What I could not establish (all out of my lane, none set the verdict):**
- **Layering table (parallel-safety or scope):**
  - Rule 4 of `docs/layering.md` gives every std module a table row, and `layering.rs:306` fails a module that has none. `std.yaml` and `std.toml` need rows, but their scopes name neither `docs/layering.md` nor `layering.rs`.
  - Rule 6 needs a "Std names in the crates" row for the `std.data` literal that derived-codec's compiler code will contain. Its scope omits that file too.
  - Four stories also all edit `docs/slice.md` and `AGENTS.md`.
- **Naming (scope or acceptance):**
  - `std/json.ignite:138` already has a private `fn parse_value`, which the new public `std.json.parse_value` collides with.
  - `std.data.encode` also shares a name with `std.json.encode`.
- **Todo byte-equality (acceptance):** `to_json` at `examples/todo/todo.ignite:59-60` writes 3 fields and `to_record` writes 5. The default `encode` of one `Todo` cannot produce both, so "HTTP bodies byte-equal" needs a second type the story does not mention.
- **Value-model acceptance (acceptance):** `Shape`, `Case`, `Tagging`, `Unknown` and `DecodeError` are declared by value-model but exercised by none of its acceptance lines. They have no reader until derived-codec, so that seam may be a drafting choice.
- **Mapping gap in `docs/serde.md` section 5 (scope):** it has no row for `Value.Moment` or `Value.Decimal`. TOML datetimes and floats therefore have no typed decode target.
- **Adopters scope (parallel-safety or scope):** data-adopters holds two independent tasks, the todo replacement and the LSP report. The LSP report reads `ignite-lsp/src/lsp_types.ignite`, which is in another repository and not in this tree.
- **Git:** `git pull` failed with "Cannot fast-forward to multiple branches". The checkout was already at 459b964c, and I did not run any other git command that moves the tree.

```findings
- file: .engineering/planning/story/data-adopters.md
  line: 20
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic outcome is typed values to YAML or TOML text and back through encode/decode, but no story crosses the codec with yaml or toml, so the only place that seam is exercised is JSON; add depends_on edges from this story to story:data-yaml and story:data-toml and an acceptance line that round-trips one typed record through all three formats"
```
