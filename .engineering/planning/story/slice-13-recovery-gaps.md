---
format: aep.planning-md/3
id: story:slice-13-recovery-gaps
kind: story
status: draft
title: Close the slice-13 recovery and inspect gaps
summary: Two recovery precedence cases from 17.5 As landed (U5), and inspect naming a refused plan as runtimeOperation after a later corrective
relations:
- derived_from: epic:slice-13-local-apply
revision: 1
---

# Story: slice-13-recovery-gaps

## Outcome

An operator who inspects a host after a failed or blocked operation sees one reason and the
operation that is actually running, whatever order the failures happened in.

## Context

Found during slice 13 (`epic:slice-13-local-apply`, released as v0.2.0) and left open on purpose:

- `docs/slice.md` 17.5 "As landed (U5)", known gaps: a `foreign_listener` block on the committed
  revision replaces the non-blocking `reducer_revision_changed` report (the receipt keeps the
  reason); `record()`'s reducer-failed path rejects the committed revision while a `gave_up` is
  active, and which failure takes precedence is undecided.
- U6 case `apply_transition::a_corrective_after_a_failed_replacement_completes_with_its_receipt`:
  after the corrective completes, `inspect` still reports the refused plan as `runtimeOperation`.
- Unchecked: `controller.rs` `finished()` returns early on `recovery_reason.is_some()`.

## Acceptance

- A test per case above, red first, on the Rust target.
- `inspect` after the corrective names the corrective (or nothing) as `runtimeOperation`.
- 17.5 states the precedence of the two reasons and the known-gaps list is empty.

## Out of Scope

- New commands or journal record kinds.
- LLVM twins of these cases (the logic is the host's).

## Ambiguities

- Precedence of reducer-failed against `gave_up`: `requires-stakeholder-input` (Timo), unless
  17.5 already orders recovery reasons; not yet checked.

## Open Questions

None.
