---
format: aep.planning-md/3
id: story:directory-reads-through-links-beneath-its-root
kind: story
status: draft
title: A Directory reads a file through a symbolic link that stays beneath its root
relations:
- informed_by: story:std-mysql-client
revision: 1
---
# Story: a Directory reads a file through a symbolic link that stays beneath its root

## Outcome

`Directory` opens a file whose path crosses symbolic links when every link resolves beneath the directory's root, and still refuses a link that leaves the root.

## Why

Reported on 2026-11-11 by the first user of `std.protocol.mysql`. `Directory` refuses every symbolic link. A volume that a Kubernetes ConfigMap or Secret mounts is a directory of links (`ca.pem` → `..data/ca.pem`, `..data` → a dated directory), so a CA file mounted that way cannot be read through the mounted directory and the program needs a workaround path. Pre-existing: the refusal is the capability rule of `Directory` since it was built.

## Acceptance

- A file reached through links that all resolve beneath the root opens for reading on both targets (the kernel's "beneath" resolution, `openat2` with `RESOLVE_BENEATH`, or the same walk by hand where it is missing). Held by a test with the ConfigMap layout: a link to a link to a file.
- A link that resolves outside the root, an absolute link and a `..` that leaves the root are refused with the diagnostic of today. Held by tests that fail if the resolution follows them.
- `docs/slice.md` states the rule in the section of `Directory`.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- The `Directory` capability in `crates/ignite-io` and `crates/ignite-rt`, its std surface, `docs/slice.md`. Inferred.
