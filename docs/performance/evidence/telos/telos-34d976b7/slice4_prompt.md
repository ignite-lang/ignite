You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets: Rust (generated crates built with cargo) and LLVM (IR → clang, linked with `libtelos_rt.a`). Read `AGENTS.md` (layout rules, including the substrate rule), `docs/slice.md` (sections 6 and 7: Rust lowering and substrate) and `docs/llvm.md` (sections 4 and 5: memory and the runtime ABI) first. You are the only agent working in the tree.

Problem (Timo's direction): substrate IO must be implemented once, in one language, and called by every target. Today it is implemented twice:
- `crates/telos-io/src/lib.rs` is the async tokio implementation that the Rust target's generated code calls.
- `crates/telos-rt/src/net.rs` and `stdio.rs` are a second, blocking implementation over std::net that the LLVM target calls through the C ABI. They reuse only the sync parts of telos-io (errors, args, termios, `socket_address`, `read_buffer`).

Target layering for this slice (slice 4):
- `crates/telos-io` is the one substrate. It is blocking (std::net, std::thread) with no tokio. It provides the Rust API: `TcpListener::bind/accept/local_port`, `TcpStream::connect/read/write_all/close`, `Stdin`, `Stdout`, `Process::exit`, `IoError`, `Input`, `args`, `terminal`, `spawn` (one OS thread per task), `log` and `run`. Move the blocking implementations from `telos-rt` into it.
- `crates/telos-rt` keeps only the C ABI adapter: it converts `TelosList`/`TelosIoError`/handles to and from telos-io types, and keeps the arena, regions, the handle table and the task/spawn handshake. It must contain no socket or stdin/stdout logic of its own: every `telos_rt_tcp_*`, `stdin_*` and `stdout_*` function calls telos-io.
- The Rust target becomes synchronous:
  - an effectful fn lowers to a plain `fn`;
  - `await e` lowers to `e`;
  - `spawn f(a)` lowers to `telos_io::spawn(move || ...)`;
  - the generated `main` calls `telos_io::run` with a plain closure.

  Update `crates/telos-rustgen`, its goldens (`TELOS_BLESS=1` or the existing bless mechanism; check that the diff is only the lowering change) and `docs/slice.md` sections 6 and 7.
- Remove tokio from the workspace entirely (workspace `Cargo.toml`, `telos-io`, and the generated `Cargo.toml` of the Rust target). The commit message must say what replaces it, so put that line in your report: "tokio removed; replaced by std::net and std::thread in telos-io".
- Slice 5 (not yours) will make telos-io async with tokio as the event loop inside the substrate. Keep the telos-io API shaped so that change touches telos-io and the two adapters, not every caller. One shape that works: blocking functions now, async versions later under the same names.

Acceptance:
- `task gate` is green: all suites, clippy `-D warnings`, `fmt --check`. Both targets pass the same 28 end-to-end cases in `crates/telos/tests/terminal.rs`; the pty tests run on both.
- `crates/telos/tests/llvm_memory.rs` stays green (flat RSS).
- `task compile` and `task compile-llvm` both build `terminald` and `tn`.
- `nm` on both targets' binaries shows no `tokio` symbols. Add a test for this to an existing driver test file.
- `cargo tree -p telos-rt` and the generated Rust crate's dependency tree contain no tokio.
- Thread count when idle is 1 for the Rust-target `terminald` too. Measure it from `/proc/<pid>/status`.
- Run `task bench` before and after. Report the size, build-time and startup rows for both targets.
- Update `AGENTS.md`: the layout table rows for telos-io and telos-rt, and replace the "Open debt" text in the substrate rule with the new state. Keep the slice-5 sentence. Update `docs/llvm.md` sections 1 to 5 wherever they say telos-rt implements IO itself.

Rules (Timo's, non-negotiable):
- No git commits, and no git operations that discard changes (no checkout, restore, reset or stash). The tree has uncommitted work from earlier slices that you build on.
- Every cargo command runs with `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only`. The exceptions are `task` targets, which use the repo's `target/`. Run `df -h /` before large builds, and stop and report if under 20G is free.
- No work files in /tmp or /dev/shm. Use `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/` with unique file names.
- No heredocs into stdin readers, and no `python3 -c` with backticks. Write scripts with the Write tool and run them by path.
- Commands likely to run longer than 2 minutes: run them in the background and wait on their output file.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No comments in Cargo.toml, Taskfile.yml or other config files.
- Use test ports outside 2323, because Timo may run a server there. Kill every process you started, by PID, before you finish.
- Generated Rust is an artifact: never edit it by hand.

Final report, at most 15 lines:
- what moved where, with line counts before and after for telos-io and telos-rt;
- test counts from cargo output;
- the bench before/after rows;
- thread count;
- deviations and contract points.
