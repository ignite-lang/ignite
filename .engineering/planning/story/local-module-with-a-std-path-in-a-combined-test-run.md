---
format: aep.planning-md/3
id: story:local-module-with-a-std-path-in-a-combined-test-run
kind: story
status: draft
title: A local module with a standard-library path in a combined test run
relations:
- informed_by: story:combined-test-runs-keep-every-sibling-suite
revision: 2
---
# Story: a local module with a standard-library path in a combined test run

## Outcome

`ignite test` with two roots, one a local file that declares a package path of the standard library (for example `package std.option`, or the `package std.rfc.rfc<n>` that `ignite rfc import` writes by default) and one that imports that standard-library module, runs both suites with the result each gives alone, or refuses the combination with a diagnostic. It does not panic.

## Why

Found by the adversary pass on the unit of `story:combined-test-runs-keep-every-sibling-suite` (wave 16, 2026-11-23); older than that unit. Observed on the LLVM target: each root alone exits 0; both together panic at `crates/ignite-llvm/src/lower.rs:2816` ("call of Fn(QualName { module: \"std.option\", name: \"unwrap_or\" })") and exit 101.

Traced: the grouping of test roots leaves standard-library modules out of the set of module paths it compares (`crates/ignite/src/test_build.rs:104`), and one standard library is loaded for every group (`:185`), so putting the two roots in separate executables alone does not fix it.

## Acceptance

- The program of the adversary case `a_local_module_with_a_standard_library_path_keeps_both_suites`, in commit `95b030fd` of branch `wave/16-combined-tests`, file `crates/ignite/tests/combined_tests_adversary.rs` (removed from the tree in `865e8ae1` so the count of ignored tests stays at its bar), comes back as a test and passes on both targets.
- A local root with a standard-library package path that nothing imports keeps its result.

## Scope

- `crates/ignite/src/test_build.rs` (grouping, the standard library per group), the driver's module loading. Inferred.
