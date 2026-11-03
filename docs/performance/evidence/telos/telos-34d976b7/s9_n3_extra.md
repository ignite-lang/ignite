## N3 additions: wave A0 soundness fixes

These come from the steering session, to which Timo delegated direction. Read `docs/vision.md` section 10.1 and 10.3 and `docs/audit/2026-11-01/README.md`.

- **M1 to M5 of vision 10.1.**
  - **Mutation analysis by variable name misses aliases.** It misses `to_bytes()` aliases, for-element bindings and match-payload bindings; this causes memory corruption, verified. Make the read-only/mutation analysis sound for every binding form. N1 adds E006 for in-place mutation through let, for and pattern bindings; that removes some cases at the source. The `to_bytes_push` probe must still pass two tests on both targets.
  - **M4:** an index on an `if` receiver never runs.
  - **M5:** `len` on a `match` receiver panics the compiler.
- **Probes.** Turn every `docs/audit/2026-11-01/llvm/*.telos` probe into a build-and-run test on both targets, with the outcome in the README's "expected after the fix" column. A probe that becomes a `telos check` error under N1's E006 is a fixture of N1's; for those, agree with N1 who keeps it.
- **R2 and R3 of vision 10.3.** R2: the last handle's drop does not send. R3: EPIPE is delivered to the other task's read. Take them only if your other work touches `crates/telos-io` anyway; otherwise report them as moved to slice 10. `docs/audit/2026-11-01/io/iotest/` reproduces both (subcommands `drop` and `cross`; build it with its own `--target-dir`).
