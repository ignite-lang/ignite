---
format: aep.planning-md/3
id: epic:slice-14-protocols-p1
kind: epic
status: implemented
title: 'Slice 14, wave B protocols P1: in-process streams and generic protocol roles'
summary: Pipe.pair()/PipeStream, the ByteStream bound, Telnet and Http roles generic over the stream with limits records, in-process protocol tests on both targets (docs/slice.md 18)
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T05:41:33Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T05:41:33Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T07:51:40Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---

## Outcome

Protocol code is written once over any byte stream: the Telnet and Http roles run over TCP in
production and over an in-process pipe in tests, on both targets, with static dispatch.

## Scope

The contract is `docs/slice.md` section 18 (acceptance 18.0, decisions 18.15, units 18.16),
reviewed in `docs/slice14-review.md`. Stories U1 to U5 decompose it, one per row of 18.16.

## Acceptance

Every row of 18.0 holds; `task gate` exit 0 on the slice branch; merged to main; tag `v0.3.0`.

## Approval

Approved for implementation in advance by Timo on 2026-11-03: "Contract approved in advance:
implement without asking again."
