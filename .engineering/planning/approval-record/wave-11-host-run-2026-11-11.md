---
format: aep.planning-md/3
id: approval-record:wave-11-host-run-2026-11-11
kind: approval-record
status: draft
title: Wave 11 takes the two rows the dedicated-host run has worse than v0.15.0
relations:
- decides: story:readiness-session-cost
- decides: story:rust-peak-memory-at-one-connection
revision: 1
---
# Wave 11 takes the two rows the dedicated-host run of 2026-11-11 has worse than v0.15.0

Decided by the coordinator on 2026-11-11 under Timo's goal for this release ("new release cut, no performance/quality drop") and his rule of the same week ("every release should be an improvement in quality, not even allowed to accept 10% degregation").

The run `iq-20261111t171623z-cf0f` (v0.15.0 against the integration head with unit size, 5 passes interleaved, 0.69 USD) has four cells worse than v0.15.0 in every pass; every other timing and benchmark cell is at or better, or overlaps:

- 3.7 `llvm/terminal/connect-8` 1.107× to 1.284× and `rust/terminal/connect-8` 1.049× to 1.221×: `story:readiness-session-cost`, a draft since unit size, joins the wave (a `terminald` session makes 63 system calls where v0.15.0 made 25).
- 3.8 `rust/terminal/rr-1` 1.069× to 1.137× and `rust/todo/get-1` 0.781× to 0.814×: `story:rust-peak-memory-at-one-connection`, new.

Both are regressions of the release being cut. The release waits for them; `story:std-mysql-client` is merged before the cut and does not wait for them.
