---
format: aep.planning-md/3
id: story:slice-14-rust-target
kind: story
status: active
title: Bounded generics on the Rust target
summary: 'U3 of docs/slice.md 18.16: rustgen arms, turbofish on bounded generics, CODEGEN_REVISION bump'
relations:
- decomposes: epic:slice-14-protocols-p1
- depends_on: story:slice-14-frontend
- depends_on: story:slice-14-substrate
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T05:41:33Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T05:41:33Z", actor: "human:timo", revision: 3}
---

## Scope

The row of this unit in `docs/slice.md` 18.16 (files owned, dependencies) and the subsections it
names. Contract approved in advance by Timo on 2026-11-03.

## Acceptance

The rows of `docs/slice.md` 18.0 whose unit column names this unit, and the first failing test of
its 18.16 row, red first and green at the end.

## Approval

Approved for implementation in advance by Timo on 2026-11-03 (the slice-14 goal).
