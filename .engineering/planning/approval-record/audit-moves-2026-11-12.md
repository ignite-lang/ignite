---
format: aep.planning-md/3
id: approval-record:audit-moves-2026-11-12
kind: approval-record
status: draft
title: Eleven moves from the store audit of 2026-11-12
relations:
- decides: story:channel-release-on-close
- decides: story:llvm-release-all-handles
- decides: story:infer-channel-ends
- decides: story:drain-keeps-bound-listeners
- decides: story:json-duplicate-keys
- decides: story:version-names-commit
- decides: story:std-digest
- decides: story:release-archive-carries-the-substrate
- decides: story:rust-target-read-only-forms-borrow
- decides: story:fixed-width-ints
- decides: story:task-thread-bound
revision: 1
---
# Eleven moves from the store audit of 2026-11-12

Decided by Timo on 2026-11-12. A read-only audit of the store compared the 23 active and 10 proposed stories with the code, the docs and the releases, and listed eleven moves whose evidence is a commit, a test or a doc line. The coordinator put three options to Timo; option A read: "apply all 11. 7 shipped stories to implemented, with the v0.16.0 gate as evidence. `release-archive-carries-the-substrate` and `rust-target-read-only-forms-borrow` accepted and to implemented. `fixed-width-ints` and `task-thread-bound` archived." Timo answered "A".

Shipped in v0.16.0 (`task gate` on `a317a6fc`, exit 0, merged as `591876af` through PR 138), moved to implemented:

- `story:channel-release-on-close`: commit `380204de`, merge `e07c8519`; `leak::g30_reproducer_stays_flat_in_every_mode`.
- `story:llvm-release-all-handles`: commit `380204de` ("KNOWN_ON_LLVM is gone").
- `story:infer-channel-ends`: merge `c9affcf2`; fixtures `valid_channel_end_from_store`, `valid_channel_end_from_call`, `e016_channel_end_inferred`.
- `story:drain-keeps-bound-listeners`: merge `813b2ffe`; the tests at the end of `crates/ignite/tests/drain.rs`.
- `story:json-duplicate-keys`: merge `39ef4f96`.
- `story:version-names-commit`: commit `6c971553`, merge `eceb865d`.
- `story:std-digest`: commit `98800a6b`, merge `4b226c8a`. Its 1 MiB timing line is an ignored test and has no recorded value.

Shipped in v0.16.1, accepted and moved to implemented:

- `story:release-archive-carries-the-substrate`: commits `6712f733`, `8b4b6a78`.
- `story:rust-target-read-only-forms-borrow`: PR 140, `21bc8eda`.

Archived:

- `story:fixed-width-ints`: an umbrella, split in `0a36ee6c` into `u32-words` (implemented), `list-capacity` and `fixed-size-lists`.
- `story:task-thread-bound`: `docs/scheduling.md` says it is answered by R1 of the readiness scheduler (`IGNITE_WORKERS`).

Not decided here: the audit's thirteen items that need a person (among them `list-capacity`, `http-parse-fewer-copies`, `apply-test-deadlines`, the two readiness stories behind the connect-8 row, and `rust-target-shared-list-storage`).
