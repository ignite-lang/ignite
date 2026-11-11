---
format: aep.planning-md/3
id: story:rustgen-local-with-a-functions-name
kind: story
status: draft
title: The Rust target calls a function in method form on a local that has the function's name
relations:
- informed_by: story:std-mysql-client
revision: 1
---
# Story: the Rust target calls a function in method form on a local that has the function's name

## Outcome

A body in which a local has the name of a function and that function is called on it in method form builds and runs on the Rust target as it does on LLVM.

## Why

Reported on 2026-11-11 by the first user of `std.protocol.mysql`. The checker accepts `let bump = 1` followed by `bump.bump()`; rustgen writes the bare call `bump(&bump)`, where `bump` is the local, and rustc refuses it with E0618 ("expected function, found …"). `ignite test --target rust` fails on the file, `ignite test` (LLVM) passes. Not specific to `test fn`: any body with a `let`. Same class as `review-result:adversary-rustgen-names-pass-2` (a resource `f` and `x.f()` in a system `main`), which fixed the resource case only. Pre-existing.

Reproducer (`ignite test --target rust` fails, `ignite test` passes; the reporter saw the same with a plain `fn`):

```
package repro

test fn bump(n: Int) -> Int = n + 1

test "a local named like a test function, called as a method" {
  let bump = 1
  assert bump.bump() == 2
}
```

rustc: `error[E0618]: expected function, found {integer}` at `let __ignite_left: i64 = bump(&bump);`.

## Acceptance

- The reproducer passes on both targets. Held by a test of `ignite-rustgen` or `crates/ignite/tests` that fails with E0618 without the fix.
- The same holds for a parameter, a `for` binding and a pattern binding with a function's name, and for a function of an imported module. One test each.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-rustgen/src` (how a call in method form names its function: a path that a local cannot shadow). Inferred.
