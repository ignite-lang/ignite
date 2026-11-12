---
format: aep.planning-md/3
id: approval-record:after-v0.16.2-2026-11-12
kind: approval-record
status: draft
title: Three drafts accepted for the waves after v0.16.2
relations:
- decides: story:rust-target-constants-built-once
- decides: story:rust-target-text-reads-borrow
- decides: story:shared-list-costs-back
revision: 1
---
# Three drafts accepted for the waves after v0.16.2

Decided by Timo on 2026-11-12. The coordinator asked which stories get the go for implementation and listed three options; option A read: "accept three drafts now: `rust-target-constants-built-once`, `rust-target-text-reads-borrow`, `shared-list-costs-back`. Order: `path-cost-counted-at-every-merge`, then these three, then roadmap wave 9." Timo answered "A".

What it accepts:

- `story:rust-target-constants-built-once`: a module constant is rebuilt on every use on the Rust target; the first user of `std.protocol.mysql` counted about 9 % of the allocations of its largest answer in two such constants.
- `story:rust-target-text-reads-borrow`: a text or byte string that is only read is copied on the Rust target; 23 % of the same answer's allocations were copies of `Text`, and std's `string_value` in the MySQL client makes one avoidable copy per text cell.
- `story:shared-list-costs-back`: the four Rust-target rows the dedicated-host run of v0.16.2 lists as worse than v0.16.1 (3.7 todo/patch-16, 6.2, 3.9 terminal and todo) and row 3.2 of the local run.

The order: `story:path-cost-counted-at-every-merge` first (accepted on 2026-11-11, not started), then these three, then the protocol clients of the roadmap's wave 9 (`story:std-websocket`, `story:http-client-role`, `story:data-codec-combinators`).

Not decided here: the moves a read-only audit of the store proposes (started the same day), and accepting `story:rust-target-read-only-forms-borrow` and `story:rust-target-shared-list-storage`, whose units are merged and released.
