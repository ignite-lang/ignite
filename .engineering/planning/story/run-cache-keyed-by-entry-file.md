---
format: aep.planning-md/3
id: story:run-cache-keyed-by-entry-file
kind: story
status: active
title: A shared build directory never starts the executable of another entry file
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:01Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:02Z", actor: "human:timo", revision: 3}
---
# Story: a shared build directory never starts the executable of another entry file

## Outcome

Two entry files in one directory, run through the same `--build`, each start their own executable: the run stamp includes the canonical identity of the entry file, so switching entries rebuilds.

## Why

IG-04, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: two copies of the supervisor fixture `crates/ignite/tests/fixtures/run_gave_up.ignite` in one directory, distinct package names, the component name `daemon` in both, and `Daemon.main` returning `Stdout.write_all(b"first\n")` in one and `Stdout.write_all(b"second\n")` in the other; both files exist before the first run. Each entry run with `ignite run`, a fresh `--state` each, the same `--build`, `--clock manual` and the control line `stop`; the second entry once more with an empty build directory and fresh state. The three copied binaries at `<state>/bin/1/daemon` printed first, first, second; the shared binary matched the first byte for byte.

`build_stamp` (`crates/ignite/src/lib.rs:1614`) hashes target and profile, the path and contents of every `.ignite` file of the entry's directory and of the standard library (`:1619` to `:1629`), the driver and the runtime, but not which file is the entry. Both entries get the same stamp, and `build_members` returns the previous executables when the stamp matches (`:1696` to `:1703`).

## Acceptance

- A regression test as above, red first: the three binaries print first, second, second.
- Controls: running the same entry twice with unchanged sources does not rebuild; the empty build directory still gives second.
- `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:1614` to `:1656` (`build_stamp`, cited `:1614`, `:1624`), `:1683` to `:1705` (`build_members`, cited `:1698`). Each read at a1d1407e, as the review states.
- `story:run-accepts-a-bare-entry-filename` and `story:run-stamp-covers-the-imported-modules-only` change the same function.
