---
format: aep.planning-md/3
id: story:checker-builtin-table-gaps
kind: story
status: draft
title: 'Checker and type checker agree on builtins: aliases, debug on parameters, missing names'
revision: 1
---
# Story: checker-builtin-table-gaps

## Outcome

`ignite-check` and `ignite-types` agree on which method calls are builtins for every receiver, including element-type aliases and `debug` on type parameters, so a user fn named like a builtin never gets a wrong E003, E016 or E042, and no stream escapes its effects.

## Context

- Found by the second adversary of the checker-names unit (`review-result:adversary-checker-names-pass-2`, 2026-11-07); all pre-existing on `f4232674`.
- F1 `crates/ignite-check/src/lib.rs:4179-4180`: `builtin_applies` compares the element type's name without expanding aliases.
- F2 `lib.rs:2297` and `:4176`: the checker treats `debug` on a type parameter as a builtin; the type checker (`builtins::method`, `has_param`) calls a user fn `debug`. The result gets no roots, so E016 is missed: an effect-soundness hole.
- F3 same cause: a false E042 on a valid program.
- F4: `BUILTIN_METHODS` lacks `next_id`, `values`, `update`, `upsert`; test `builtin_tables::every_type_checker_builtin_name_is_known_to_the_checker` (commit `b5261cb` on `fix/checker-names`, main after wave 6) is `#[ignore]`d for this story.

Fixtures (from the adversary; add each under its crate's `tests/fixtures/`):

`crates/ignite-check/tests/fixtures/valid_alias_element_builtin.ignite` (expected: empty):

```
type Point {
  x: Int
}

type Octet = Byte

type Line = Text

fn to_text(p: Point, extra: Int) -> Text = "user"

fn join(p: Point) -> Text = "user"

fn decode(b: List<Octet>) -> Text = b.to_text()

fn joined(lines: List<Line>) -> Text = lines.join(",")
```

Actual today: `18:37: error[E003]: wrong number of arguments to `to_text`: expected 2, found 1` and `20:40: … `join`: expected 1, found 2`.

`crates/ignite-check/tests/fixtures/e016_bounded_debug_user_fn.ignite`:

```
fn debug<U>(x: U) -> U = x

fn send<S: ByteStream>(stream: S) -> Result<Unit, IoError> requires [stream.read] {
  let other = stream.debug()
  other.write_all("x".to_bytes())
}
```

Expected: `e016_bounded_debug_user_fn.ignite:9:3: error[E016]: missing effect `stream.write` for `other.write_all`; add it to the `requires` of `send`` (line numbers with the fixture's 4 comment lines); actual today: clean.

`crates/ignite-types/tests/fixtures/valid_generic_debug_user_fn.ignite` (expected: empty):

```
fn debug<U>(x: U, store: Store<Int>) -> Text requires [store.write] {
  store.put(1, 1)
  "user"
}

fn show<T>(items: List<T>, store: Store<Int>) -> Text requires [store.write] = items.debug(store)
```

Actual today: `E042 cannot check the effects of `debug``.

## Acceptance

- The three fixtures pass; the ignored table test is un-ignored and passes.
- `task gate` green.
