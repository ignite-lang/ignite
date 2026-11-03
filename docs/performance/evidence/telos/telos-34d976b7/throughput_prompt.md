You work on Telos, a Rust compiler for the Telos language with two targets: Rust (generated crates built with cargo, async on tokio today) and LLVM (IR → clang, linked with `libtelos_rt.a`, one OS thread per task). The repository is /home/timo/projects/telos. Read its `AGENTS.md`, `docs/llvm.md` and `examples/terminal/terminal.telos` (a Telnet command console: `terminald` server, `tn` client; commands `help`, `echo <text>`, `status`, `quit`).

Timo's request: "measure throughput of the server-side, benchmark both, make the telos version faster than the rust one". "Both" means the `terminald` built by the Rust target and the `terminald` built by the LLVM target. "The telos version" is the LLVM-target build. Compare release against release; report debug for information only.

Another agent is rewriting the substrate in the repository right now (slice 4: tokio removed, `telos-io` becomes blocking and the only IO implementation, `telos-rt` becomes the C ABI adapter). Do NOT edit anything under /home/timo/projects/telos in this phase.

Phase 1, which is your whole job now:
1. Work in the snapshot of the green slice-3 tree at `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/snap3` (a full copy, its own cargo workspace). Build there with `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only`. The `task` targets in the snapshot use its own `target/`, which is fine.
2. Write a load generator as a new crate in the snapshot, `crates/telos-load` (add it to the snapshot's workspace members). It is a Rust binary using std threads, with no new external dependencies unless one replaces substantial code; if you add one, name it and what it replaces. It speaks enough Telnet to talk to `terminald`: it ignores or refuses IAC negotiation and waits for the prompt `terminal> `. Workloads, each run for a fixed duration (default 10 s) after a 1 s warm-up:
   - `rr`: request–response. Each connection sends `echo <64 bytes>\r\n` and waits for the reply line and the next prompt. Report replies/s, p50/p99/p999 latency.
   - `pipe`: pipelined. Each connection keeps up to 64 requests in flight. Report replies/s and MB/s of replies.
   - `connect`: each iteration connects, waits for the prompt, sends `quit`, waits for close. Report sessions/s and p99.
   - Concurrency levels 1, 16 and 256 connections for `rr` and `pipe`; 1 and 16 for `connect`.
   - Also report server CPU seconds (utime+stime from `/proc/<pid>/stat`), CPU per 1000 replies, peak RSS and thread count.
   Pin nothing; record `nproc` and the load average at start.
3. Build release and debug `terminald` for both targets from the snapshot: Rust target `telos build ... --release --target-dir ...`, LLVM target `telos build --target llvm --release ...` with the release runtime archive. Run the whole matrix and write the results as a markdown table to `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/throughput-baseline.md`. Run each cell 3 times and report the median. Use ports outside 2323.
4. Profile the LLVM release `terminald` under the `pipe` 16 workload with `perf record -g`, if perf is available and allowed. Otherwise count instructions per function with `valgrind --tool=callgrind` on a short run, or sample with whatever exists. Do the same for the Rust release build. Write the top 15 functions with their share for each to the same file. From that, write a ranked list of concrete changes that would make the LLVM build faster than the Rust build. The list should name the generator construct (for example "deep copy of `Text` argument at every call to X in lower.rs"), the runtime function or the substrate call (syscalls per reply, read size, write coalescing, TCP_NODELAY), with the estimated share each removes. Also give `strace -c` syscall counts per 1000 replies for both servers.

Do not implement optimizations in this phase. Phase 2, after slice 4 lands, will port the load generator into the repository and implement the list. Write `throughput-baseline.md` so that phase 2 can work from it alone.

Rules (Timo's, non-negotiable):
- No git operations. No edits under /home/timo/projects/telos.
- No work files in /tmp or /dev/shm. Everything goes under `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/` with unique names.
- No heredocs into stdin readers, and no `python3 -c` with backticks. Write scripts with the Write tool and run them by path.
- Commands likely to run longer than 2 minutes: run them in the background and wait on their output file. Run `df -h /` before large builds, and stop and report if under 20G is free.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No comments in Cargo.toml or other config files.
- Kill every process you started (servers, load generators, perf), by PID, before you finish. Ports outside 2323.
- Another agent is running cargo builds at the same time. If CPU contention distorts a run (load average above nproc/2 at start), record it next to the cell and re-run that cell later.

Final report, at most 15 lines:
- the release-vs-release table for `rr` 16, `pipe` 16, `pipe` 256 and `connect` 16 (replies/s, p99, CPU per 1000 replies);
- the top 3 hot spots per target;
- the top 5 ranked changes;
- the path of `throughput-baseline.md`.
