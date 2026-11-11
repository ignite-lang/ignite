---
format: aep.planning-md/3
id: review-result:adversary-counts-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit counts (wave 11)
relations:
- reviews: story:ignored-tests-and-long-files-back
revision: 1
---
red

Adversary pass 1 on unit counts, `wave/11-counts` at `5f37aa6d`, test commits `f475cca9`, `2b174111`.

verdict: red; cases executed 1148→1151, red 3; origin: introduced 5, pre-existing 1.

Findings:
1. `crates/ignite/tests/fixtures/http_reply_adversary.ignite:100`, CONFIRMED, introduced, blocker. An in-repo importer names `hex` through `import std.protocol.http.*`; the split moved it to `std.protocol.http.syntax` and the fixture no longer checks (E003). No lane compiles the fixture, so nothing was red. Red case `http_chunked_adversary::every_program_importing_std_protocol_http_still_checks`.
2. `crates/ignite-types/src/typer.rs:685`, CONFIRMED, introduced, warning. `literal_waits` counts a nested literal that would wait on its own as binding nothing, even when a sibling field binds the parameter it needs; a fully bound outer literal waits, and a later literal with `touch = None` is typed with `T` unbound: E042 on a program the base accepts. Red case `adversary_http_app::a_literal_that_binds_every_parameter_through_a_nested_literal_does_not_wait`. No in-repo program is reached (602 identical emit outputs, 524 identical failures).
3. `crates/ignite-check/src/lib.rs:2940`, CONFIRMED, introduced, warning. `stores` matches the `Map.put` row by name for any receiver type outside `IN_PLACE` without asking `builtin_method`: a user fn named `put` gives its receiver the argument's roots and a correct program is E016. Red case `ownership_adversary::a_user_fn_named_put_gives_its_receiver_no_roots`. The same false E016 for a user `push` exists at base. Proposed: `&& self.builtin_method(receiver, name)` in `stores`.
4. `std/protocol/http.ignite:12`, CONFIRMED, introduced, warning. The 34 moved helpers (`is_token`, `trim_ows`, `RequestLine`, `request_line`, `hex`, `chunk`, `is_host`, …), exported by `std.protocol.http` at v0.15.0 and at base, are no longer importable under that path: imports are not re-exported (E031, E003).
5. `crates/ignite-types/src/typer.rs:666`, CONFIRMED, pre-existing, note. Giving a defaulted field its default explicitly (`touch = None`) keeps the literal from waiting, so `run(Box { state = "s", touch = None }, 7)` stays E042 while the same call without the field is now accepted.
6. `std/protocol/http/syntax.ignite`, CONFIRMED, introduced, note. Release binary bytes base → head: Rust `todod` 1,085,480 → 1,085,744 (+264), LLVM `todod` 1,535,264 → 1,535,440 (+176), Rust `terminald` 766,832 → 766,808 (−24), LLVM `terminald` 1,153,152 → 1,153,136 (−16).

Held: 7.2 `#[ignore` lists at base and head (27, 24) differ only by the three attributes; the three bodies are unchanged; `#[test]` 1,630 → 1,636, inline tests 630 → 630; the counter is unchanged. 7.3: `http.ignite` 2,715 and `syntax.ignite` 455 lines, every base line in one of the two. `satisfies` 21/21, `verifies` 39/39 in `http.ignite`; golden diffs are the three positions moved one line. Typer: conflicting bindings, a conflicting later argument, a literal field against a later argument, five-deep nesting, a literal as receiver and a list of literals stay rejected and none hangs; a literal in a closure body and in a `match` scrutinee are newly and correctly accepted; `run(Box { state = first()? }, second()?)` returns `Err("first")` on both targets. Checker: scalar map values, a scalar key, a typed `Map<Text, TcpStream>` under the right `requires`, a `Store` put give no false E016. Suite: `cargo test -p ignite-types` 112 passed, 1 failed (case 2), 4 ignored; `cargo test -p ignite-check` 23 passed, 2 failed (case 3 and the known layering red); `task test-ignite` 452 + 452; `conformance:: plan:: typed_json:: todo:: drain::` 121 passed, 1 ignored; clippy and fmt exit 0.

```findings
[
{"file":"crates/ignite/tests/fixtures/http_reply_adversary.ignite","line":100,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"an in-repo importer names hex through import std.protocol.http.*, the split moved it to std.protocol.http.syntax, and the fixture no longer checks (E003); http_chunked_adversary::every_program_importing_std_protocol_http_still_checks is red"},
{"file":"crates/ignite-types/src/typer.rs","line":685,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"literal_waits counts a nested literal that would wait as binding nothing even when a sibling field binds its parameter, so a fully bound outer literal waits and a later touch = None literal is typed with T unbound: E042 on a program the base accepts"},
{"file":"crates/ignite-check/src/lib.rs","line":2940,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"stores matches the Map.put row by name for any receiver type outside IN_PLACE without builtin_method, so a user fn named put gives its receiver the argument's roots and a correct program is E016"},
{"file":"std/protocol/http.ignite","line":12,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the 34 moved helpers, exported by std.protocol.http at v0.15.0 and at base, are no longer importable under that path because imports are not re-exported (E031, E003)"},
{"file":"crates/ignite-types/src/typer.rs","line":666,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"giving a defaulted field its default explicitly (touch = None) keeps the literal from waiting, so the program stays E042 while the same call without the field is now accepted"},
{"file":"std/protocol/http/syntax.ignite","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"release todod grows 264 bytes on Rust and 176 on LLVM with the split; terminald shrinks by 24 and 16"}
]
```
