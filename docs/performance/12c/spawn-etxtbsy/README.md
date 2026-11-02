# Transient executable write conflict

The integrated gate reported `run::probe::tests::the_first_line_is_the_result`
failing to execute its completed script with ETXTBSY (errno 26). Closing the
parent's writer does not suffice if a concurrent fork briefly inherited that
writer. `fd-reproduction/report.json` demonstrates the same error after the
parent close and successful execution after the child releases its descriptor.
This matches the [execve ETXTBSY contract](https://man7.org/linux/man-pages/man2/execve.2.html).

The existing dedicated spawner retries only ETXTBSY on its original parent
thread. Backoffs are 1, 2, 4, 8 and 16 ms, totaling 31 ms; a sixth failure is
returned unchanged. Other spawn errors return immediately. Process parent-death
signaling, journal events and restart policy are unchanged.

The deterministic regression holds the writable executable in another process,
allows a first failed exec, then releases the writer before the second exec.
`before.log` retains the exact pre-fix errno 26. Intermediate regression failures
exposed inherited test-barrier descriptors and process-parent lifetime; those
logs are retained too. `after-unit-lifecycle-fixed.log` passes all 22 library
tests, including the original probe and the new regression. Independent review
then replaced a borrowed numeric test descriptor with owned sockets and write
shutdown, keeping ownership valid even if a later retry occurs.
`after-unit-socket-shutdown.log` passes all 22 tests with that final barrier.
`run-replay.log`
passes two live manual runs, journal comparison and replay. Full integrated
validation and final timing are still required for the new frozen candidate.
