---
format: aep.planning-md/3
id: story:file-system
kind: story
status: draft
title: Files under a root directory
summary: Read, write, list, create directories and remove files under a root, never outside it
relations:
- decomposes: epic:playground-substrate
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: std
revision: 2
---
# Story: file-system

## Outcome

An Ignite program reads, writes, lists, creates directories and removes files under a root
directory it is given, on both targets, and cannot reach a path outside that root.

## Context

- Gap P2 of the playground session (2026-11-04): its server holds the user's `.ignite` files.
- Substrate in `crates/ignite-io`, a std module over it, a new effect the checker tracks; the root
  is a capability value, as a listener is.

## Acceptance

- An inline test on both targets writes a file, reads it back byte for byte, lists the directory,
  creates a subdirectory and removes both.
- A path that leaves the root (`..`, an absolute path, a symlink out) is refused with an error
  value, not an IO.
- A function that touches files without declaring the effect is rejected by `ignite check`.

## Out of Scope

- File watching, permissions, memory-mapped files.
