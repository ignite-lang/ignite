---
format: aep.planning-md/3
id: approval-record:wave-10-release-cut-2026-11-10
kind: approval-record
status: draft
title: Release after wave 10 carries four of its seven stories
relations:
- decides: story:http-client-role
- decides: story:std-websocket
- decides: story:data-codec-combinators
revision: 1
---
# Approval: the release after wave 10 is cut at four stories

## Words

- Operator, 2026-11-10, asked for the release scope with options A (full waves) and B (cut each
  release at what is green when the running units finish, carry the rest into the next wave):
  "ignite -> B , else A".

## Decision

- The release carries waves 8 and 9 and, of wave 10, the stories whose units are running now and
  pass their reviews: `story:std-resp-client`, `story:std-nats-client`, `story:tls-client`,
  `story:http-app-connection`.
- `story:http-client-role`, `story:std-websocket` and `story:data-codec-combinators` stay active
  and open the next wave. No unit is started for them before the release.
- A story of the four that is not green when the other three are merged is carried the same way.
