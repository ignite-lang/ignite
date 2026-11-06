# Static executables: size and build time (`docs/slice.md` 23)

Measured 2026-11-06 02:53 to 02:58 local time (UTC+1) on the `feat/static-linking` tree
(base `27065d6b8` plus the uncommitted implementation of `story:static-linking`). 20 cores,
shared with other sessions' builds; 1-minute load 7.6 to 14.4 at the start of each build
(column `load1`). sccache was on. The call ran through the session's build-slot wrapper
(`CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=8`, `nice -n 10`, `ionice -c3`).

Method (`measure.sh`, one line): `target/release/ignite` (the release driver, which carries the
release runtime and the release musl kit) builds each program with `--release` into a fresh
`--out` and `--target-dir`, three runs, each run going through todod and `ignite-lsp`
(`examples/lsp/server.ignite`), each target, dynamic then `--static`. The time is the wall
time of the whole `ignite build` call; the size is the executable's byte count. Raw rows:
`records.tsv`.

`examples/lsp/` left this repository in story:lsp-repository; the server is `src/server.ignite`
of github.com/ignite-lang/ignite-lsp. To rerun `measure.sh` on a later tree, change its
`examples/lsp/server.ignite` to that file in a checkout of ignite-lsp.

## Programs

Median of 3 runs, range in brackets; sizes were identical in all three runs.

| program | target | dynamic s | static s | dynamic bytes | static bytes | size ratio |
|---|---|---|---|---|---|---|
| todod | LLVM | 1.42 (1.13 to 2.54) | 1.49 (1.12 to 2.14) | 883,784 | 993,192 | 1.124 |
| todod | Rust | 13.59 (6.91 to 46.68) | 11.44 (3.72 to 18.01) | 871,944 | 948,424 | 1.088 |
| ignite-lsp | LLVM | 8.22 (8.04 to 9.16) | 7.80 (7.09 to 9.36) | 1,316,344 | 1,405,816 | 1.068 |
| ignite-lsp | Rust | 13.96 (13.43 to 28.98) | 25.14 (15.29 to 47.90) | 1,584,064 | 1,574,824 | 0.994 |

- LLVM: a static build costs no measurable time; the link reads `libc.a` and `libunwind.a`
  instead of the shared libraries. The first static build of a driver also writes the musl kit
  (32 MB) to the runtime cache once; run 1 of todod includes that write.
- Rust: every build compiles `ignite-io` and its dependencies in a fresh target directory, the
  static one for `x86_64-unknown-linux-musl`. The spread (3.7 to 47.9 s for the same build)
  is that work under shared load and sccache hits and misses, not the link mode; these rows
  show no difference between the modes that the spread does not cover.
- Sizes: the static executables hold musl's `libc.a` objects they use and `libunwind`; the
  dynamic ones need `libc.so.6`, `libm.so.6`, `libgcc_s.so.1` and the loader beside them. The
  growth is 6.8 to 12.4 % on LLVM; on Rust todod grows 8.8 % and `ignite-lsp` shrinks 0.6 %.
- `file` on every static executable: `ELF 64-bit LSB executable, x86-64, … statically linked`;
  on every dynamic one: `pie executable, … dynamically linked, interpreter
  /lib64/ld-linux-x86-64.so.2`.

## The driver

The driver carries the musl kit when the Rust target is installed where it is built
(`docs/slice.md` 23.3): the runtime archive for musl and the seven files of the target's
`self-contained` directory (6,621,452 bytes: `libc.a` 6,406,314, `libunwind.a` 201,418, five
start files 13,720).

| driver | without kit bytes | with kit bytes | carried by the kit |
|---|---|---|---|
| `target/debug/ignite` | 89,033,768 (measured, base tree, 02:13) | 131,234,632 (measured) | musl runtime 35,425,994 + 6,621,452 |
| `target/release/ignite` | 35,650,054 (computed: with kit minus the kit's bytes) | 67,846,688 (measured) | musl runtime 25,575,182 + 6,621,452 |

The release row's "without" figure is computed, not measured: no release driver of the base
tree was built.

Build time of the kit: `build.rs` builds the musl runtime in the same nested `cargo build` as
the host one (two `--target` flags). A cold standalone build of
`cargo rustc -p ignite-rt --features runner --target x86_64-unknown-linux-musl` (dev profile,
dependencies included) took 2 min 35 s at load 19 (02:08 to 02:11); how much of that the
nested build overlaps with the host runtime's build is not measured. The first release driver
build in this tree, both runtimes included, took 1 min 33 s (02:52 to 02:53, load about 10). The gate's cost is in the
merge PR's gate line.
