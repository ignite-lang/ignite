# Publish synchronous spawn completion at the turn boundary

The process adapter already knows Started/SpawnFailed before spawn returns.
Its existing public channel API keeps the same publication point before
watcher threads start. A private borrowed callback lets Live retain its own
synchronous acknowledgement and deliver it at a completed driver-turn boundary.

The initial Start fanout, full lifecycle recipient order, existing due-timer
snapshot, manual-clock settle instant and finite stop boundary are preserved.
Completions drain before unrelated held observations and before stop. Delivery
uses the current foreground timestamp. Engine and journal source are unchanged;
every effect still synchronizes before dispatch.

The deterministic regression uses real std.control reducers, an actual adapter
spawn and a held probe result after the restart timer fires. It asserts all
three Started lifecycle deliveries precede the probe, then verifies the probe's
three Publish effects still occur and the journal replays. Restoring the old
ordinary-channel spawn plus forwarding observation boundary makes that exact
journal-order assertion fail. Initial fixture failures and negative mutations
are retained alongside the final passing logs.

Validation passed all 26 library unit tests, 14 run/replay integration cases,
three drain cases covering both backends, Clippy and formatting. These checks
support the scheduling correction; the next frozen full batch must still pass
the unchanged real-clock bound and every performance criterion.
