---
format: aep.planning-md/3
id: review-result:data-formats-critic-acceptance-1
kind: review-result
status: active
title: Plan critic (acceptance), data-formats, round 1
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
story:data-adopters — the acceptance requires files and HTTP bodies "byte-equal to before" from the derived `encode`, but `Todo` has `deleted_by`, `to_json` writes three fields and `to_record` five, so one derived `encode(todo)` cannot give both and the acceptance names no wire type to do it — .engineering/planning/story/data-adopters.md:20
story:data-adopters — "the existing todo suites stay green unchanged" contradicts "no `to_json`/`to_record`": inline `test "to_json"` calls `to_json` directly and two tests call `to_record` — examples/todo/todo.ignite:355
story:data-adopters — the report bullet names `ignite-lsp/src/lsp_types.ignite`, which is not in this tree (`ls ignite-lsp` fails). The file lives in the external ignite-lsp repository, so the acceptance does not say which checkout or revision the number is taken against — .engineering/planning/story/data-adopters.md:21
story:data-yaml — the "list of refused cases is a checked-in file" has no rule limiting its entries to the four named refused features, so any failing in-subset case can be listed and the suite test passes vacuously — .engineering/planning/story/data-yaml.md:24
story:data-toml — the Outcome says `emit` refuses a non-`Map` root and any `Null`, but no acceptance bullet tests either refusal; the round-trip bullet covers only roots without `Null` — .engineering/planning/story/data-toml.md:20
story:data-derived-codec — the Outcome says "the mapping is the table of section 5", but the fixtures omit `Duration`, `Map` keyed by a unit-variant enum, the check diagnostic for another `Map` key type, `Bytes` decoded from base64 text and the O1 annotated-`let` diagnostic, so those rows can fail while the story is marked done — .engineering/planning/story/data-derived-codec.md:20
story:data-value-model — the Outcome declares eight types (`Value`, `Shape`, `Case`, `Tagging`, `Unknown`, `DecodeError`, `DataLimits`, `FormatError`), but the acceptance observes only `Json` conversion and layering, so nothing checks `Shape`'s defaults, `DecodeError`'s `describe()` or `FormatError`'s positions — .engineering/planning/story/data-value-model.md:19
epic:data-formats — the epic has no `## Acceptance` section (the sibling epics `slice-14-protocols-p1` and `playground-substrate` have one), and its Outcome says any data value becomes TOML "and back" while `story:data-toml` refuses a non-`Map` root and any `Null` — .engineering/planning/epic/data-formats.md:11

What I read: 6 artifacts given, 6 read in full (`aep plan artifact show` for the epic and the five stories), plus `docs/serde.md` at 459b964c, `aep plan artifact kinds` and `lifecycle`, and `examples/todo/todo.ignite` and the epic files for comparison.

What I could not establish:
- I did not check whether `yaml-test-suite` or `toml-test` ship an expected-JSON file for every case the acceptances assume.
- Several stories have bullets that join more than one outcome, for example `CODEGEN_REVISION bumped; docs/slice.md section; task gate green`. I judged this the store's convention and did not report it.
- Out of my lane (design or scope): the story:data-derived-codec scope overlaps story:mid-level-ir on `crates/ignite-types`. The epic Outcome's "in the evaluator" appears in only one story's acceptance (derived-codec).

```findings
- file: .engineering/planning/story/data-adopters.md
  line: 20
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance requires files and HTTP bodies byte-equal to before from the derived encode, but Todo has deleted_by and to_json writes three fields while to_record writes five, so one derived encode(todo) cannot give both and the acceptance names no wire type to do it"
- file: examples/todo/todo.ignite
  line: 355
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance 'existing todo suites stay green unchanged' contradicts 'no to_json/to_record': the inline test \"to_json\" calls to_json directly and two tests call to_record"
- file: .engineering/planning/story/data-adopters.md
  line: 21
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the report bullet names ignite-lsp/src/lsp_types.ignite, which is not in this tree, and names no checkout or revision of the external ignite-lsp repository the count is taken against"
- file: .engineering/planning/story/data-yaml.md
  line: 24
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the list of refused cases is a checked-in file with no rule limiting its entries to anchors, aliases, tags, complex keys and merge keys, so any failing in-subset case can be listed and the suite test passes vacuously"
- file: .engineering/planning/story/data-toml.md
  line: 20
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says emit refuses a non-Map root and any Null, but no acceptance bullet tests either refusal; the round-trip bullet covers only roots without Null"
- file: .engineering/planning/story/data-derived-codec.md
  line: 20
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the mapping is the table of section 5, but the fixtures omit Duration, Map keyed by a unit-variant enum, the check diagnostic for another Map key type, Bytes decoded from base64 text and the O1 annotated-let diagnostic"
- file: .engineering/planning/story/data-value-model.md
  line: 19
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome declares eight types but the acceptance observes only Json conversion and layering, so nothing checks Shape defaults, DecodeError.describe or FormatError positions"
- file: .engineering/planning/epic/data-formats.md
  line: 11
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic has no Acceptance section and its Outcome says any data value becomes TOML and back while story:data-toml refuses a non-Map root and any Null"
```
