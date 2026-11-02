# Slice 13 contract review

Status: independently reviewed proposal; awaiting user approval before runtime implementation.
This batch changes documentation only. Acceptance scenarios are future obligations,
not executed tests or claims of conformance.

## Agreed scope

- Explicit foreground local host; no daemon lifecycle.
- Source-bound saved plans; no portable artifact bundle.
- Replacement-first changes and existing draining behavior; no in-place updates or stateful migration.
- Performance recovery remains deferred.

## Deliverables

| Approved work | Contract location |
|---|---|
| First terminal workflow and CLI | slice.md 17.1 and 17.6 |
| Execution owner and host lifetime | 17.1 and 17.2 |
| Reviewed input versus operational identity | 17.3 |
| Change, removal and dependency behavior | 17.4 |
| Durable operations, retry and recovery | 17.5 |
| Inputs/actions/observable acceptance matrix | 17.7 |
| Assignable implementation owners and dependencies | 17.7 task table |

## Independent review findings resolved

| Finding | Resolution |
|---|---|
| New readiness cannot precede old listener release on the same exclusive port | Build/verify first, drain/release old listener, start replacement, then gate dependents on Ready |
| Existing replay requires one exact model and is not operation reconciliation | Separate versioned host history, owned revision context and multi-revision recovery obligation |
| Model equality alone misses changed application or compiler behavior | Pin raw inputs separately; operational identity includes implementation and codegen semantics |
| Plan hashing could accidentally include its own id | Exclude the id field from canonical envelope hashing |
| A completed retry appears stale after its own revision commit | Verify payload integrity and look up durable receipt before new-plan stale-base rejection |
| Historical PID or matching port is not ownership proof | Require exact owned instance identity; uncertain outcomes block mutation |
| Missing deletion vocabulary could prohibit the existing terminal example | Host creation grants exact process drain/stop and listener-release authority; retain artifacts/history by default |
| Removing the last managed workload must not use run's runs-nothing refusal | Apply permits zero hosted machines and keeps host idle; run E125 remains unchanged |

Review was grounded in design.md 7.1–7.3/10.1, vision.md 4.1, and the existing
run/model/build/journal/replay ownership boundaries. The main drafting and runtime-boundary
review were delegated to separate agents; the parent reviewed the complete contract and
roadmap alignment. Scope is documentation, not command/schema/runtime implementation.

## Verification

- Documentation diff and whitespace checks.
- New-section structure, command examples, matrix and local document references checked.
- No runtime source/manifests changed; no builds or runtime tests run for this documentation batch.
- User approval remains required before assigning runtime implementation work.
