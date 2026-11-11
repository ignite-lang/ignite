---
format: aep.planning-md/3
id: story:ignored-tests-and-long-files-back
kind: story
status: implemented
title: No more ignored tests and no more files over 3,000 lines than at v0.15.0
relations:
- depends_on: story:http-reply-cost-back
- informed_by: story:typer-record-literal-deferred
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: crates/ignite/tests/conformance.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: quality/baseline.json
- confidence: cited
  path: std/protocol/http.ignite
- confidence: cited
  path: std/protocol/http/syntax.ignite
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: no more ignored tests and no more files over 3,000 lines than at v0.15.0

## Outcome

Row 7.2 (ignored tests) is at or below 24 (now 27) and row 7.3 (source files over 3,000 lines) at
or below 4 (now 5).

## Why

Both rows moved worse by decisions since v0.15.0; no release is worse than the one before it
(`AGENTS.md` § Releases).

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/counts-ignored.md`, `counts-http-split.md`):

- 7.2: the three new attributes are `resp::a_real_server_and_sentinel_on_both_targets` and
  `nats::a_real_server_on_both_targets` (they need server binaries the gate machine does not
  have) and `adversary_http_app::a_record_literal_before_the_argument_that_binds_its_parameter`
  (a typer gap, `story:typer-record-literal-deferred`). A typer change of +45 -4 lines was tried
  (`cargo test -p ignite-types`: 111 passed, 4 ignored). The attribute at
  `crates/ignite/tests/conformance.rs:1484` is stale: the fix it cites is in the history.
- 7.3: `std/protocol/http.ignite` grew from 2,518 to 3,133 lines. 35 helpers that need only
  `std.text` can move to `std/protocol/http/syntax.ignite` (510 lines), leaving 2,636; checked in
  a copy against the 11 in-repo importers and the conformance matrix.

## Acceptance

- 7.2 measures 24 or lower, by tests that run: the record-literal case passes without `#[ignore]`
  (a record literal with an own type parameter nothing binds yet is typed after the arguments
  that bind it; programs that were rejected before and are accepted now are listed in the unit's
  report with the diagnostic they had); the stale attribute is removed and its test passes; and
  one more ignored test runs because what it waited for is done. Which one is the unit's choice,
  stated with the reason.
- No test is deleted, renamed out of the counter's sight or returned from early behind an
  environment switch.
- 7.3 measures 4 or lower: `std/protocol/http.ignite` is under 3,000 lines; every path an
  importer names is unchanged; `satisfies` and `verifies` stay in the protocol's module; the
  inline test names of `std/protocol/http.ignite` are unchanged so the differential baseline
  keeps its unit names.
- The bars of 7.2 and 7.3 are blessed to the measured values and their wave-10 decisions
  withdrawn. `AGENTS.md` § Layout and the book name the new module.

## Scope

- `crates/ignite-types/src/typer.rs`, its tests; `crates/ignite/tests/adversary_http_app.rs` or
  where that test lives; `crates/ignite/tests/conformance.rs`. Cited.
- `std/protocol/http.ignite`, new `std/protocol/http/syntax.ignite`, `docs/layering.md` if a row
  is needed. Cited.
- `AGENTS.md` § Layout, the book page that lists std modules. Inferred.
