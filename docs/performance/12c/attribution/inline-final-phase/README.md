# Current combined inline phases

`profile.py` exports the current source into a separate diagnostic directory,
adds private phase timers there, builds a separate CLI and runs one preparation,
one list and one complete combined suite. It never instruments the production
candidate. To reproduce elsewhere, adjust its explicit `repo`, `root` and
runtime paths. `source.json` records the exported bytes and `environment.json`
records build settings;
`summary.json` retains exact commands, load and every measured phase.

The full invocation executed all 148 tests on both targets. At host load about
14 it took 0.697 s, including about 104 ms shared preparation, 94 ms Rust std
emission, 155 ms LLVM emission and 130/135 ms native execution. Those diagnostic
timings do not satisfy the final low-load paired acceptance protocol.

Two runtime checks each read the whole 29 MiB archive: runner-symbol presence
took about 13 ms and first-ELF target identification about 11 ms. This supports
bounded streaming in their existing owners. IR stamp hashing took about 13 ms
and remains unchanged: generated IR matching a current `.ll` file does not
prove that an older object/stamp belongs to that IR.
