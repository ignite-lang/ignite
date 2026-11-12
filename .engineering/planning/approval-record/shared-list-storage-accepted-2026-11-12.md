---
format: aep.planning-md/3
id: approval-record:shared-list-storage-accepted-2026-11-12
kind: approval-record
status: draft
title: The shared list storage is accepted as implemented
relations:
- decides: story:rust-target-shared-list-storage
revision: 1
---
# The shared list storage is accepted as implemented

Decided on 2026-11-12 through the session Timo authorised that day to decide low-risk and easily reverted questions on his behalf. The coordinator asked whether `story:rust-target-shared-list-storage` is accepted; the answer: accept it as implemented now, so the store matches the merged unit and the release.

What it accepts:

- `story:rust-target-shared-list-storage`: its unit `shared-lists` is merged (PR 143) and released in v0.16.2. The story carries the gate result, the adversary pass and the dedicated-host run as evidence. The four Rust-target rows that run lists as worse are held by `story:shared-list-costs-back` (active).

Not decided here: the other items a read-only audit of the store marked as needing a person (waived rows, stories to close or keep, the order of the accepted stories).
