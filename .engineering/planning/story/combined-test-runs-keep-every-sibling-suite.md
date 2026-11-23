---
format: aep.planning-md/3
id: story:combined-test-runs-keep-every-sibling-suite
kind: story
status: implemented
title: A test run over several roots runs every sibling suite and fails when one fails
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:01Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-23T21:50:05Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a test run over several roots runs every sibling suite and fails when one fails

## Outcome

`ignite test a/one.ignite b/two.ignite`, in either argument order, runs the tests of both local `p.shared` modules and exits 1 when one of them fails. Roots whose transitive modules share a dotted name but come from different source files run in separate test executables.

## Why

IG-03, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive). Synthetic reproducer, four files:

```ignite
// a/one.ignite
package p.one
import p.shared.pick
test "first root" { assert pick() == 1 }
```

```ignite
// a/shared.ignite
package p.shared
fn pick() -> Int = 1
test "shared first" { assert true }
```

```ignite
// b/two.ignite
package p.two
import p.shared.pick
test "second root" { assert pick() == 1 }
```

```ignite
// b/shared.ignite
package p.shared
fn pick() -> Int = 1
test "shared second" { assert false }
```

`ignite test b/two.ignite` ran two tests and exited 1. `ignite test a/one.ignite b/two.ignite` ran three tests, printed three passes and exited 0.

`group_files` puts both roots in one group because their own module names differ (`crates/ignite/src/test_build.rs:80` to `:83`). `type_test_programs` adds a sibling only when no program of the same module name is in the group yet (`:295`), so the second `p.shared` is dropped, and the test list is built from module names (`:341` to `:346`).

## Acceptance

- A regression test with the four files, red first: both argument orders run four tests, report `shared second` as failed and exit 1.
- Controls: `ignite test b/two.ignite` alone runs two and fails, as today; two roots in one directory that share one physical `shared.ignite` still run its tests once.
- `task gate` green.

## Scope

- `crates/ignite/src/test_build.rs:70` to `:99` (`group_files`, cited `:80`), `:288` to `:314` (`type_test_programs`, cited `:295`), `:317` to `:346` (`prepare_group`, cited `:341`). Each cited line read at a1d1407e, as the review states.
- `story:repeated-test-file-runs-once` changes the same grouping; whichever lands second keeps both tests.
