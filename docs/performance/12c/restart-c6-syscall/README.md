# One bounded restart trace

One invocation of the frozen c6 real-clock restart case ran under syscall tracing,
with a 30-second watchdog. It passed diagnostically; it does not replace the
failed warm batch. Source and test/CLI binary hashes were unchanged. Exact
commands, loads, raw trace, journal and per-record analysis are retained.

The trace's restart journal interval was 115.849 ms. The new child completed
`execve` about 114.048 ms after the Signalled journal write, and Started was
written 2.087 ms later. The probe result's five required effect syncs took
31.205 ms total in this trace and happened before timer/spawn dispatch.

The failed 599.617 ms journal had a different interleaving: timer/spawn dispatch
preceded handling a probe result; Started then waited through that probe's
effect fanout. Synchronous spawn had returned before AttemptDone was handled,
bounding actual restart by 320.243 ms after Signalled, while foreground Started
publication came 279.375 ms later. Timer timestamps use the scheduled deadline,
so their exact 100 ms interval does not establish on-time dispatch.

This establishes publication queueing in the failure, but the passing trace
does not assign the failed run's costs between CPU scheduling and storage.
The proposed correction prioritizes the driver's known synchronous completion
at a completed turn boundary, uses the current foreground clock and retains
every O30 effect barrier. The 100–500 ms assertion remains unchanged.
