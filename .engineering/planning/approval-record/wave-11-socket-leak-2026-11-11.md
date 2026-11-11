---
format: aep.planning-md/3
id: approval-record:wave-11-socket-leak-2026-11-11
kind: approval-record
status: draft
title: Wave 11 takes the socket leak; the MySQL client starts beside the release
relations:
- decides: story:llvm-http-connection-closes-its-socket
- decides: story:std-mysql-client
revision: 1
---
# Wave 11 takes the socket leak as its twelfth story, and the MySQL client starts beside it

Decided by the coordinator on 2026-11-11 under Timo's goal for this release ("new release cut, no performance/quality drop") and his messages of the same day ("its not accreptable to work in that pace", "fix your process", "we are desperately waiting for the sql client too").

- `story:llvm-http-connection-closes-its-socket` joins wave 11: the leak is a regression of the release being cut, found by the wave's own review.
- `story:std-mysql-client` (accepted by Timo on 2026-11-11) is implemented now, in its own worktree on the integration branch, beside the release work. It does not wait for the release and the release does not wait for it. Its edge to `story:path-cost-counted-at-every-merge` is taken back; that story stays accepted and follows.
- The dedicated-host run is started as soon as the integration branch holds the units that move time rows, not after the last merge.
