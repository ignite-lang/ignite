---
format: aep.planning-md/3
id: approval-record:plan-2026-11-07-waves-2-to-6
kind: approval-record
status: draft
title: Accept the stories of waves 2 to 6
relations:
- decides: story:spawn-moves-adversary-findings
- decides: story:rustgen-reserved-names
- decides: story:stable-node-ids
- decides: story:store-adversary-findings
- decides: story:shared-ownership-analysis
- decides: story:http-chunked-adversary-findings
- decides: story:mid-level-ir
- decides: story:large-file-splits
- decides: story:store-api-gaps
revision: 1
---
# Approval: waves 2 to 6 of plan-2026-11-07

## Words

- The coordinator asked on 2026-11-07: "Waves 2 to 6. The 10 stories are still draft; plan in .engineering/waves/plan-2026-11-07.md. A: accept all 10 stories now; wave 2 (spawn-moves-adversary-findings) starts next. B: accept only wave 2's story. C: change the plan first."
- Timo answered: "A accept all".

## Decision

- Move to active: spawn-moves-adversary-findings, rustgen-reserved-names, stable-node-ids, store-adversary-findings, shared-ownership-analysis, http-chunked-adversary-findings, mid-level-ir, large-file-splits, store-api-gaps.
- Waves run in the plan's order, each dispatched after the previous one closes; the approval of each wave's stage-1 stop is taken as given by this answer.
- Commits authorised per wave: the unit commits, the PR merged into main after a green task gate, the store commits on main. No tag, no release.
- story:apply-test-deadlines stays active (decision 2, default B: the next waves' gates count toward its load row).
