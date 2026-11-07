---
format: aep.planning-md/3
id: review-result:data-formats-critic-design-2
kind: review-result
status: active
title: Plan critic (design), data-formats, round 2
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
story:data-derived-codec — its outcome says the frontend writes, for every data type, the `Codec` that story:data-codec-combinators' combinators would build, but those combinators stop at `record8`, so a record of nine or more fields has no combinator form. Neither body says what is written for it, and adopters measures ignite-lsp types that exceed eight fields. Either the outcome states that the pass writes the `Codec` directly, or combinators gains a record builder with no field-count cap, with the existing `depends_on` edge unchanged — .engineering/planning/story/data-derived-codec.md:17, docs/serde.md:104, docs/serde.md:145, .engineering/planning/story/data-codec-combinators.md:16

**What I read:**
- 7 artifacts: the epic, the 5 stories and review-result:data-formats-critic-design-1.
- Commands: `aep plan artifact show` on each, `relations`, `graph`, `validate`.
- Also `docs/serde.md`.
- Edges: I walked 18 edges touching the set (6 `decomposes`, 8 `depends_on` among the stories, plus the review edges). None runs to an artifact outside the set. There is no cycle.
- Shape: value-model comes first, then combinators, yaml and toml in parallel, then derived-codec after combinators, then adopters after all four. It is not a serial chain.
- Acceptance reads: I traced 12 reads of state a sibling produces. All 12 have a `depends_on` edge, direct or through the chain: `of_data`, `Shape`, `DataLimits`, the combinators fixture, `Codec&lt;T&gt;`, `codec()`, `std.yaml` and `std.toml`.
- Round 1: the finding on story:data-adopters is fixed. It now depends on yaml and toml and has a three-format round-trip line (data-adopters.md:21).
- `validate` reports `valid`. It also lists 9 older review-results with no findings block, which are not part of this set.

**What I could not establish (out of my lane, none set the verdict):**
- Scope: serde.md:210 says a constant `Shape` with a `renames` entry naming no field is a compile-time diagnostic. Combinators tests only the run-time `DecodeError`, and derived-codec's diagnostic list omits this one, so no story owns the compile-time check.
- Scope: serde.md section 5 still has no `Value.Moment` or `Value.Decimal` row, so TOML datetimes and floats have no typed decode target. This is carried over from round 1.
- Parallel-safety: `std/json.ignite:138` has a private `parse_value`, and `std.data.encode` shares a name with `std.json.encode`. Both are carried over from round 1.

```findings
- file: .engineering/planning/story/data-derived-codec.md
  line: 17
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome says the frontend writes, for every data type, the Codec that the combinators would build, but the combinators stop at record8 (docs/serde.md:104, data-codec-combinators.md:16), so a record of nine or more fields has no combinator form and neither body says what is written for it; either state that the pass writes the Codec directly, or give combinators a record builder with no field-count cap"
```
