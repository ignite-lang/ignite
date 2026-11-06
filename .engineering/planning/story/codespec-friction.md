---
format: aep.planning-md/3
id: story:codespec-friction
kind: story
status: active
title: Language gaps found by the codespec adapters
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T03:47:31Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-06T03:47:32Z", actor: "human:timo", revision: 3}
---
# Story: codespec-friction

## Outcome

The language gaps the ignite-codespec adapters hit on ignite 0.7.0 are rows of `docs/friction.md`, and the five that force copied code or refusals there are fixed: sibling program files, a stderr stream, ordering on `Text`, list element assignment and concatenation, and wrapping integer arithmetic.

## Context

- Found by ignite-lang/ignite-codespec stories cir-model and ignite-writer, 2026-11-06 (that repository's `src/cir_json.ignite`, `src/write.ignite`, `tools/gen_model.ignite`).
- Fix list, each with where it bit:
  1. `ignite test` runs no test block of a sibling module, a sibling given as a file cannot import its siblings (E031, `crates/ignite/src/lib.rs:941` `parse_siblings`), and one program file cannot import another of the same package; ignite-codespec copies its reader and pretty printer into `src/write.ignite:124`, `:167`.
  2. No `Stderr`: refusal lines go out through `log()` (`src/write.ignite:89`).
  3. `Text` has no `<`, `<=`, `>`, `>=` (E045; `tools/gen_model.ignite:370`), so Go string ordering is refused (`src/write.ignite:1269`).
  4. `List` has no element assignment (`src/write.ignite:1633`; friction row 41 is the field-step part) and `List + List` is E045 (`:1832`).
  5. No wrapping integer arithmetic in std; the writer emits its own `wrapping_add`/`wrapping_sub` helpers.
- Recorded only, not fixed here: `Module` is a builtin type name (E004); a type's functions cannot be called with method syntax from another module (E041); `to_text` is a reserved method name; `{` in a normal string starts interpolation and `"{{"` is one `{`; a continuation line cannot start with `+`; List has no `contains`; `for` and assignment are not expressions; a sibling needs the main file's package prefix.

## Acceptance

- `docs/friction.md` has one open row per item above (fixed ones move to a "Solved" section with the test that covers them).
- Each fixed item has a test on both targets (inline Ignite test or a `crates/ignite/tests` case) and a `docs/slice.md` entry; new diagnostics have fixtures.
- `task gate` green on the PR head.

## Out of Scope

- Changes in ignite-codespec (its writer drops the copies after a release that carries this).
- Closure-capture moves (llvm-lowering-copies P4).
