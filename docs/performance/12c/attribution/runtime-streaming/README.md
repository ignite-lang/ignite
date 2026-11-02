# Runtime archive checks

The existing owners now stream the runtime checks instead of each allocating
and reading the entire 29 MiB archive. Runner-symbol search preserves the exact
byte pattern anywhere in the file, including across short reads and buffer
boundaries. Target identification seeks over archive members and reads the
first ELF header, retaining full declared-member bounds, odd padding, byte
order, first-ELF selection and unknown-format fallback.

These predicates establish symbol presence and first-member architecture;
they do not validate unrelated trailer data. Successful early detection no
longer reads that trailer or encounters its possible I/O errors. The archive
target check requires a seekable runtime file. Relevant open/read/seek errors
retain the runtime path. No persistent cache or IR stamp semantics change.

All 25 library tests and five LLVM build/cache/target integration tests pass.
Clippy initially rejected test-module placement and a manual parity check;
both were corrected without lint suppression, and `clippy-final.log` passes.
One complete inline task passes all 148 cases on each target. Raw command
outcomes and failed development checks remain in this directory.

The external instrumented after-check in `../inline-streaming-phase/` lists
296 cases. Runner inspection falls to 0.465 ms and target inspection to
0.012 ms, versus about 13 and 11 ms in the prior full diagnostic. IR hashing
remains about 13 ms. Host loads differ, so these phase observations justify
the implementation but do not establish paired performance acceptance.
