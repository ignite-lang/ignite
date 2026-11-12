---
format: aep.planning-md/3
id: story:substrate-lookup-says-what-it-found
kind: story
status: draft
title: The substrate lookup resolves a path once and says what it found
relations:
- informed_by: story:release-archive-carries-the-substrate
revision: 1
---
# Story: the substrate lookup resolves a path once and says what it found

## Outcome

`--substrate` with a relative path builds from any working directory, a released driver that finds no substrate names the three places it looked at, and a substrate of another release beside the driver is refused with both versions in the message.

## Why

The adversary pass on unit `pkg` (2026-11-12) found three defects in the lookup of the Rust target's substrate. Two are older than the unit. Its tests are on branch `wave/13-pkg-adv` (`aa68393a`), file `crates/ignite/tests/dist_adversary.rs`:

- `a_relative_substrate_is_resolved_against_the_generated_project`: `ignite build words.ignite --target rust --out <dir>/out --substrate ../substrate/crates/ignite-io` fails for a program that imports `std.text`. The generated `std/` crate, one directory below the project, resolves the relative path again (`failed to read <out>/substrate/crates/ignite-io/Cargo.toml`), and `seed_lock` resolves the same path against the working directory. `site/book/src/reference/cli.md:64` says the path is resolved against the generated project. Older than the unit; `ignite test --substrate`, added by the unit, reaches it too.
- `the_released_driver_without_its_substrate_says_where_it_looked` (runs with `IGNITE_DIST_ARCHIVE`): a released driver with `share/ignite/std` and no substrate fails with cargo's `failed to load manifest for dependency ignite-io` only. Nothing names `--substrate` or the order of the places. Users who installed v0.16.0 by its install page (which copied `bin/ignite` and `std` only) reach it when they replace the driver alone. Older than the unit.
- No test yet: the build-tree driver placed beside a v0.15.0 `share/ignite/substrate` uses it without a notice, and the mismatch shows as `error[E0433]: cannot find census in ignite_io`. Introduced by the unit's order (beside the driver before the build tree); reached by a partial upgrade.

## Acceptance

- The driver makes a `--substrate` path absolute once, against one stated base, before it writes the manifests and seeds the lock; `cli.md` names that base.
- A driver that finds no substrate exits with a message that names `--substrate`, `<prefix>/share/ignite/substrate` and the build tree, before cargo runs.
- A substrate whose package version differs from the driver's is refused with both versions; `--substrate` given by hand is exempt or warned, and the story's implementor says which and why.
- The two tests are taken from the branch and are green; a third covers the version check. None is `#[ignore]` in `task gate` unless it needs the archive.
- `task gate` green.

## Scope

- `crates/ignite/src/lib.rs` (`located`, `installed_substrate_path`, `seed_lock`), `crates/ignite-rustgen` (the path written into the generated manifests), `crates/ignite/tests/dist.rs`, `site/book/src/reference/cli.md`. Inferred.
