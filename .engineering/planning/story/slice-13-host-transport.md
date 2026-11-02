---
format: aep.planning-md/3
id: story:slice-13-host-transport
kind: story
status: implemented
title: Host control socket, state lock and inspect
relations:
- decomposes: epic:slice-13-local-apply
- depends_on: story:slice-13-host-split
scope:
- confidence: cited
  path: crates/ignite/src/apply/host/transport.rs
- confidence: cited
  path: crates/ignite/src/apply/mod.rs
- confidence: cited
  path: crates/ignite/src/main.rs
- confidence: cited
  path: crates/ignite/tests/apply_host.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:21Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-02T21:23:20Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Scope

Versioned length-framed control frames with request ids and a size limit; state lock and owner-only socket with no permission window; atomic state identity; `inspect` fields and offline labelling; exit codes 0/1/2/3.

## Acceptance

17.7 rows 12 (second host refused) and 15 (`ignite run` on host state refused); 17.1 and 17.2 protocol, socket, identity and inspect fields. First failing test: `apply_host::malformed_oversized_and_wrong_version_frames_reject_with_status_2`.

## State

In progress on branch `s13/u2-transport`.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
