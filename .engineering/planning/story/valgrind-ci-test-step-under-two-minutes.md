---
format: aep.planning-md/3
id: story:valgrind-ci-test-step-under-two-minutes
kind: story
status: draft
title: The valgrind CI job's test step stays under two minutes
summary: task test-valgrind in CI took 4 min 26 s on its first run; time each test and bring the step under two minutes
revision: 1
---
# Story: the valgrind CI job's test step stays under two minutes

## Outcome

The `valgrind` workflow's step "Run the valgrind tests" (`task test-valgrind`) takes under two minutes on a pull request, and its log names each test's time so the slowest are known from any run.

## Why

The first run of the workflow (run 45348350881, PR #151, 2026-11-24) took 5 min 2 s; its test step 4 min 26 s: 1 min 17 s to build the `test` profile of `ignite`, then 188.48 s for the 48 tests of the twelve valgrind modules. The log gives no per-test time: libtest prints none on stable, and the only slow test it names is `moves_cost::the_flat_forms_stay_within_the_same_bound` ("has been running for over 60 seconds"). The ten slowest are therefore not known yet; that is the first acceptance line.

## Acceptance

- The workflow prints each test's time (for example the libtest JSON output, or a timing wrapper around each module), and the story's next revision lists the ten slowest from one run.
- The test step of a pull request run takes under two minutes, by the step's start and end times in `gh run view --json jobs`, without removing a test or loosening a bar.

## Scope

- `.github/workflows/valgrind.yml`, `Taskfile.yml` (`test-valgrind`), the slowest of the twelve modules in `crates/ignite/tests`. Inferred.
