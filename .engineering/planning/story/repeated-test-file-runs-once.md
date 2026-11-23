---
format: aep.planning-md/3
id: story:repeated-test-file-runs-once
kind: story
status: draft
title: Naming a test file twice runs it once instead of panicking
revision: 1
---
# Story: naming a test file twice runs it once instead of panicking

## Outcome

`ignite test x.ignite x.ignite`, and overlapping selections that name one file twice under the same or a different spelling, run the tests of that file once and exit by their result. Two distinct files that declare the same package still both run, as today.

## Why

IG-12, severity medium, class logic; traced in source only (the maintainer review of 2026-11-23 against main at a1d1407e). No command was run.

`prepare_tests` parses each file argument once, keyed by the path as given (`crates/ignite/src/test_build.rs:139` to `:147`, the check at `:143`). `group_files` puts the second occurrence of the same module into a new group (`:80` to `:87`). `prepare_group` removes the parsed entry for the first group, and for the second group `expect("every program of a group was parsed")` (`:327` to `:329`) panics. Read further at a1d1407e: two spellings of one file (`x.ignite`, `./x.ignite`) are parsed twice and land in two groups, so their tests would run twice rather than panic.

## Acceptance

- A test, red first: the same path named twice runs its tests once with no panic; `x.ignite` and `./x.ignite` together run them once.
- Control: two physically distinct files that declare the same package run both suites.
- `task gate` green.

## Scope

- `crates/ignite/src/test_build.rs:135` to `:150` (`prepare_tests`, cited `:143`), `:70` to `:99` (`group_files`, cited `:80`), `:317` to `:331` (`prepare_group`, cited `:329`). Each read at a1d1407e, as the review states.
- `story:combined-test-runs-keep-every-sibling-suite` changes the same grouping.
