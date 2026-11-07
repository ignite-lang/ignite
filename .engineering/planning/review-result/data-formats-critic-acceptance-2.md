---
format: aep.planning-md/3
id: review-result:data-formats-critic-acceptance-2
kind: review-result
status: active
title: Plan critic (acceptance), data-formats, round 2
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
story:data-adopters — `to_record` becomes `encode(codec(), todo)` "over `Todo`'s fields", but `Todo` has six fields including `deleted_by` (a derived encode emits it) while `to_record` writes five, and no todo suite reads file bytes, so "data files are byte-equal" is stated but nothing checks it — .engineering/planning/story/data-adopters.md:23 (`examples/todo/todo.ignite:45`, `:63-69`; `crates/ignite/tests/todo.rs` has no assertion on file content)
story:data-yaml — "Each case outside the refused features gives the suite's expected JSON" says nothing about the suite's `error` cases outside the refused features, so a reader that accepts invalid YAML passes (data-toml states its invalid-case bullet) — .engineering/planning/story/data-yaml.md:24
story:data-derived-codec — the Outcome promises the `Codec&lt;T&gt;` the combinators "would build by hand", but the acceptance compares only `encode(codec(), x)` with `encode(hand_codec, x)`, so a derived `decode` can be wrong while every bullet passes — .engineering/planning/story/data-derived-codec.md:21

Round 1 landed on `story:data-value-model`, `story:data-toml`, `story:data-yaml` (refused-list rule), `story:data-derived-codec` (fixtures and diagnostics), the `ignite-lsp` pin, and `epic:data-formats` (Acceptance section and the TOML "root is a map, no null" scope). It did not land on the byte-equal claim for `story:data-adopters`, which is the first finding above.

What I read: 7 artifacts given, 7 read in full (`aep plan artifact show` for the epic, the five stories, `story:data-codec-combinators` and `review-result:data-formats-critic-acceptance-1`), plus `docs/serde.md` sections 3 and 6, `examples/todo/todo.ignite`, `crates/ignite/tests/todo.rs` and `gh api` for the `v0.2.0` tag of `ignite-lsp`. The tag exists and resolves to `9b31d37`.

What I could not establish:
- Whether `yaml-test-suite` has an `in.json` for every valid case, or `toml-test` can express local date and local time as `Value.Moment` (a design matter, out of my lane).
- Whether `src/lsp_types.ignite` exists at `v0.2.0` (my file query errored).
- Out of my lane: `story:data-derived-codec` overlaps `story:mid-level-ir` on `crates/ignite-types`. The epic bullet "Every story implemented" is the store's convention, and I did not report it.

```findings
- file: .engineering/planning/story/data-adopters.md
  line: 23
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "to_record becomes encode(codec(), todo) over Todo's fields, but Todo has deleted_by (todo.ignite:45) which a derived encode emits while to_record writes five fields, and no todo suite reads file bytes, so 'data files are byte-equal' is not checked by the suites named"
- file: .engineering/planning/story/data-yaml.md
  line: 24
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'Each case outside the refused features gives the suite's expected JSON' says nothing about the suite's error cases outside the refused features, so a reader that accepts invalid YAML passes"
- file: .engineering/planning/story/data-derived-codec.md
  line: 21
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises the Codec&lt;T&gt; the combinators would build by hand but the acceptance compares only encode(codec(), x) with encode(hand_codec, x), so a derived decode can be wrong while every bullet passes"
```
