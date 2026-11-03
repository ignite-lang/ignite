# terminald throughput baseline (phase 1, slice-3 snapshot)

Measured 2026-11-01 on the slice-3 snapshot
`/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/snap3`. The snapshot has
`crates/telos-load` added. Tokio Rust target against blocking thread-per-connection LLVM target.
Phase 2 works from this file alone.

## Setup

- Machine: Intel i9-10900K (10 cores, 20 threads, `nproc` 20), 62 GiB, Linux 6.18.49. Nothing
  was pinned. Another agent was running cargo builds at the same time.
- Toolchains: rustc 1.99.1, clang 22.1.8, perf 7.2.3 (`perf_event_paranoid` 2, user space only),
  valgrind 3.25.1, strace 7.0 (`ptrace_scope` 1, so the tracee has to be started under strace).
- Builds (`CARGO_TARGET_DIR=…/target-k`, `CARGO_INCREMENTAL=0`,
  `CARGO_PROFILE_DEV_DEBUG=line-tables-only`). Script: `tp/build_servers.sh`.
  - Rust target: `telos build examples/terminal/terminal.telos --out target-k/srv/rust-{release,debug} --target-dir target-k/srv/rust-target [--release]`.
    Binaries are in `target-k/srv/rust-target/{release,debug}/terminald`: tokio multi-thread
    runtime with 20 workers, cargo's default release profile.
  - LLVM target: `telos build … --target llvm [--release] --runtime target-k/{release,debug}/libtelos_rt.a --out target-k/srv/llvm-{release,debug}`.
    Release is `clang -O2` per module with no LTO; debug is `-O1`. One OS thread per connection.
- Load generator: `snap3/crates/telos-load` (std threads only, no new dependency;
  `setsockopt` is declared `extern "C"` against the libc that std already links). Usage:
  `telos-load --workload rr|pipe|connect --connections N [--server BIN | --pid PID] [--port P] [--duration 10] [--warmup 1] [--payload 64] [--depth 64] [--quickack] [--label L] [--out FILE]`.
  - It opens every connection and waits for its prompt, then warms up for 1 s, then measures for 10 s.
  - `rr` sends `echo <64 bytes>\r\n` and waits for `<64 bytes>\r\nterminal> `, which is 76 reply
    bytes.
  - `pipe` keeps 64 requests in flight per connection.
  - `connect` connects, waits for the prompt, sends `quit`, waits for `bye` and the close.
  - It reads server CPU from `/proc/<pid>/stat` (utime+stime of all threads, exited ones included)
    and peak RSS from `VmHWM`.
  - It samples the thread maximum every 50 ms.
  - At start it records `nproc`, `load1`, and the mean runnable-task count over 1 s, sampled
    before the server starts.
  - It writes one JSON line per run.
- Matrix: `tp/matrix.sh` covers 4 builds × {rr 1/16/256, pipe 1/16/256, connect 1/16} × 2
  client modes × 3 runs. Every cell is the median of 3 runs, each metric taken separately. Ports
  2451-2990.
- Contention rule: a run counts as contended when the runnable-task mean before it exceeded
  nproc/2 = 10. I used this instead of `load1` because our own previous run inflates `load1`: a
  pipe-256 run alone pushes it above 100. Both values are printed per run.
  - Every cell with a contended run was re-run 3 times, with `tp/rerun.sh` waiting up to 2 min for
    runnable ≤ 10 before each run. The re-runs replace the cell.
  - The four key release cells were re-run as well: rr 16, pipe 16, pipe 256 and connect 16.
  - 10 cells still have 1 contended run out of 3. They are marked in the tables.
- Raw data: `tp/results-main.jsonl` (192 runs) and `tp/results-rerun.jsonl` (141 runs).

### The two client modes

The server writes the reply line and the prompt with two `write_all` calls, in
`std/protocol/telnet.telos` `answer_line`: `render(reply)`, then `write_text(prompt)`. Its
sockets do not set `TCP_NODELAY`. Nagle holds the prompt until the client ACKs the reply line,
and the client's delayed ACK fires after about 40 ms. So every request round costs 41 ms on
**both** targets, for any client that does not ACK at once. The stock-client tables show this:
rr 1 gives 24 replies/s, and every rr and pipe cell has a p50 of 41.0 ms.

`--quickack` sets `TCP_QUICKACK` after every client read, which removes the stall from the
client side and exposes the server's own cost. **The comparison below uses the quickack client.**
The stock tables are the baseline as a plain client sees it.

## Release vs release (quickack client, medians of 3)

| workload | conns | build | replies/s | p50 | p99 | CPU ms / 1000 replies | peak RSS MiB | threads |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| rr | 16 | rust | 300.9 k | 42 µs | 180 µs | 11.18 | 4.8 | 21 |
| rr | 16 | llvm | **427.2 k** | 33 µs | 93 µs | 12.70 | 4.5 | 17 |
| pipe | 16 | rust | **1.63 M** | 417 µs | 3.10 ms | 5.53 | 5.7 | 21 |
| pipe | 16 | llvm | 777.7 k | 1.06 ms | 3.79 ms | 10.06 | 5.7 | 17 |
| pipe | 256 | rust | 693.1 k | 19.84 ms | 75.87 ms | 10.47 | 10.7 | 21 |
| pipe | 256 | llvm | **2.63 M** | 5.75 ms | 17.56 ms | 5.03 | 42.8 | 257 |
| connect | 16 | rust | **82.2 k** | 162 µs | 513 µs | 34.67 | 5.2 | 21 |
| connect | 16 | llvm | 43.8 k | 356 µs | 531 µs | 48.49 | 11.3 | 5 |

The LLVM build loses pipe 16 (0.48×) and connect 16 (0.53×). It wins rr 16 (1.42×) and
pipe 256 (3.8×). Run-to-run variance is high on this shared machine. Outside the matrix, in the
profiling runs, rr 16 rust measured 344-398 k and pipe 16 llvm measured 1.0-1.2 M.

## Full matrix

Medians of 3; per-run `load1/runnable` and contention in the last column.

#### release, client with TCP_QUICKACK

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-release | 54.6 k | 4.15 | 18 µs | 27 µs | 68 µs | 4.51 | 8.27 | 3.8 | 21 | 3 | no (load1/runnable: 6.0/0.6, 6.5/0.6, 6.3/0.4) |
| rr | 1 | llvm-release | 56.0 k | 4.26 | 16 µs | 30 µs | 61 µs | 3.70 | 6.61 | 2.9 | 2 | 3 | no (load1/runnable: 7.2/2.5, 8.2/2.3, 9.0/9.2) |
| rr | 16 | rust-release | 300.9 k | 22.87 | 42 µs | 180 µs | 261 µs | 33.81 | 11.18 | 4.8 | 21 | 3 | no (load1/runnable: 8.2/1.6, 36.5/1.5, 22.9/5.0) |
| rr | 16 | llvm-release | 427.2 k | 32.47 | 33 µs | 93 µs | 274 µs | 52.68 | 12.70 | 4.5 | 17 | 3 | 1/3 contended (load1/runnable: 19.1/1.4, 27.2/3.1, 23.7/25.1) |
| rr | 256 | rust-release | 450.3 k | 34.22 | 512 µs | 2.20 ms | 4.72 ms | 62.32 | 14.54 | 9.1 | 21 | 3 | no (load1/runnable: 22.3/4.3, 26.2/2.5, 18.2/1.5) |
| rr | 256 | llvm-release | 532.0 k | 40.43 | 473 µs | 965 µs | 3.48 ms | 60.05 | 11.29 | 23.8 | 257 | 3 | no (load1/runnable: 30.0/3.1, 34.3/0.5, 22.0/2.8) |
| pipe | 1 | rust-release | 295.5 k | 22.46 | 214 µs | 361 µs | 385 µs | 11.01 | 3.69 | 3.8 | 21 | 3 | 1/3 contended (load1/runnable: 6.3/6.1, 7.2/12.5, 7.0/0.8) |
| pipe | 1 | llvm-release | 303.4 k | 23.06 | 210 µs | 321 µs | 352 µs | 9.26 | 3.05 | 3.2 | 2 | 3 | no (load1/runnable: 8.8/2.7, 9.0/1.8, 8.9/1.2) |
| pipe | 16 | rust-release | 1.63 M | 123.68 | 417 µs | 3.10 ms | 4.53 ms | 88.13 | 5.53 | 5.7 | 21 | 3 | no (load1/runnable: 9.7/5.3, 31.2/0.9, 21.5/6.2) |
| pipe | 16 | llvm-release | 777.7 k | 59.10 | 1.06 ms | 3.79 ms | 6.37 ms | 79.56 | 10.06 | 5.7 | 17 | 3 | 1/3 contended (load1/runnable: 19.3/0.6, 30.2/8.6, 25.5/11.1) |
| pipe | 256 | rust-release | 693.1 k | 52.68 | 19.84 ms | 75.87 ms | 101.03 ms | 84.13 | 10.47 | 10.7 | 21 | 3 | 1/3 contended (load1/runnable: 12.8/1.0, 30.7/15.7, 25.0/2.5) |
| pipe | 256 | llvm-release | 2.63 M | 199.79 | 5.75 ms | 17.56 ms | 24.73 ms | 132.33 | 5.03 | 42.8 | 257 | 3 | no (load1/runnable: 19.1/0.8, 29.6/7.3, 25.8/3.1) |
| connect | 1 | rust-release | 16.1 k | 0.24 | 60 µs | 88 µs | 123 µs | 5.52 | 34.37 | 3.8 | 21 | 3 | no (load1/runnable: 6.7/1.9, 7.2/2.1, 8.5/0.4) |
| connect | 1 | llvm-release | 14.8 k | 0.22 | 63 µs | 116 µs | 195 µs | 5.97 | 40.62 | 5.6 | 2 | 3 | no (load1/runnable: 12.4/7.2, 10.7/1.4, 10.6/3.0) |
| connect | 16 | rust-release | 82.2 k | 1.23 | 162 µs | 513 µs | 1.34 ms | 28.61 | 34.67 | 5.2 | 21 | 3 | 1/3 contended (load1/runnable: 19.5/0.7, 30.4/11.4, 26.2/3.8) |
| connect | 16 | llvm-release | 43.8 k | 0.66 | 356 µs | 531 µs | 1.56 ms | 21.26 | 48.49 | 11.3 | 5 | 3 | no (load1/runnable: 61.5/0.7, 68.5/3.6, 64.7/3.5) |

#### debug, client with TCP_QUICKACK

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-debug | 30.3 k | 2.31 | 30 µs | 53 µs | 88 µs | 8.75 | 28.85 | 4.3 | 21 | 3 | no (load1/runnable: 48.8/1.9, 34.9/2.6, 48.3/3.5) |
| rr | 1 | llvm-debug | 41.6 k | 3.16 | 23 µs | 37 µs | 76 µs | 5.74 | 13.81 | 3.1 | 2 | 3 | no (load1/runnable: 83.2/1.4, 86.8/8.9, 58.3/3.6) |
| rr | 16 | rust-debug | 185.3 k | 14.08 | 76 µs | 286 µs | 499 µs | 66.92 | 36.12 | 5.5 | 21 | 3 | no (load1/runnable: 42.3/0.8, 24.3/7.5, 39.1/4.7) |
| rr | 16 | llvm-debug | 284.1 k | 21.59 | 56 µs | 113 µs | 218 µs | 89.64 | 31.87 | 4.4 | 17 | 3 | no (load1/runnable: 20.8/4.3, 26.0/3.2, 16.0/1.2) |
| rr | 256 | rust-debug | 251.5 k | 19.11 | 985 µs | 2.20 ms | 4.18 ms | 126.68 | 50.37 | 8.4 | 21 | 3 | no (load1/runnable: 84.0/3.1, 90.7/0.7, 104.6/1.6) |
| rr | 256 | llvm-debug | 393.8 k | 29.93 | 663 µs | 1.77 ms | 4.69 ms | 96.53 | 24.83 | 24.9 | 257 | 3 | no (load1/runnable: 70.4/1.1, 74.0/1.1, 53.7/2.6) |
| pipe | 1 | rust-debug | 81.9 k | 6.22 | 797 µs | 1.45 ms | 2.17 ms | 11.58 | 14.14 | 4.4 | 21 | 3 | no (load1/runnable: 75.1/1.1, 80.3/2.2, 70.8/2.8) |
| pipe | 1 | llvm-debug | 87.8 k | 6.67 | 725 µs | 1.17 ms | 1.24 ms | 8.82 | 10.04 | 3.2 | 2 | 3 | no (load1/runnable: 71.9/3.1, 70.6/3.7, 69.6/1.4) |
| pipe | 16 | rust-debug | 375.9 k | 28.57 | 2.37 ms | 8.95 ms | 13.19 ms | 90.19 | 24.69 | 5.9 | 21 | 3 | no (load1/runnable: 64.1/7.2, 53.6/0.2, 56.6/3.0) |
| pipe | 16 | llvm-debug | 448.8 k | 34.11 | 1.83 ms | 8.09 ms | 12.20 ms | 82.49 | 18.38 | 5.8 | 17 | 3 | no (load1/runnable: 7.4/2.9, 19.4/5.6, 5.8/1.6) |
| pipe | 256 | rust-debug | 349.7 k | 26.58 | 40.68 ms | 131.67 ms | 172.45 ms | 114.82 | 33.43 | 10.9 | 21 | 3 | no (load1/runnable: 56.5/4.4, 48.6/3.8, 49.9/8.2) |
| pipe | 256 | llvm-debug | 892.5 k | 67.83 | 17.02 ms | 48.78 ms | 64.90 ms | 150.86 | 16.90 | 43.8 | 257 | 3 | 1/3 contended (load1/runnable: 39.0/5.2, 24.2/3.8, 15.3/30.9) |
| connect | 1 | rust-debug | 11.8 k | 0.18 | 79 µs | 127 µs | 214 µs | 10.79 | 92.42 | 4.4 | 21 | 3 | no (load1/runnable: 116.8/3.4, 114.3/2.2, 99.1/5.0) |
| connect | 1 | llvm-debug | 11.8 k | 0.18 | 80 µs | 150 µs | 422 µs | 7.18 | 59.98 | 5.2 | 2 | 3 | no (load1/runnable: 5.4/3.7, 6.7/2.1, 6.0/3.1) |
| connect | 16 | rust-debug | 75.0 k | 1.12 | 200 µs | 519 µs | 885 µs | 62.93 | 84.06 | 5.5 | 21 | 3 | no (load1/runnable: 92.9/0.6, 90.2/3.0, 79.1/2.5) |
| connect | 16 | llvm-debug | 19.8 k | 0.30 | 451 µs | 5.90 ms | 19.00 ms | 15.12 | 71.39 | 7.0 | 6 | 3 | no (load1/runnable: 5.2/1.6, 15.8/1.8, 5.5/1.4) |

#### release, stock client

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-release | 24.2 | 0.00 | 41.01 ms | 42.05 ms | 42.10 ms | 0.01 | 41.32 | 3.8 | 21 | 3 | no (load1/runnable: 16.1/6.0, 15.3/1.2, 18.1/4.7) |
| rr | 1 | llvm-release | 24.1 | 0.00 | 41.02 ms | 42.14 ms | 42.41 ms | 0.00 | 0.00 | 2.9 | 2 | 3 | no (load1/runnable: 31.2/5.5, 32.0/1.2, 24.7/6.3) |
| rr | 16 | rust-release | 386.6 | 0.03 | 41.01 ms | 42.09 ms | 42.16 ms | 0.08 | 20.69 | 4.3 | 21 | 3 | no (load1/runnable: 13.9/6.2, 13.3/0.9, 14.8/1.9) |
| rr | 16 | llvm-release | 386.6 | 0.03 | 41.01 ms | 42.97 ms | 45.01 ms | 0.09 | 23.28 | 4.3 | 17 | 3 | no (load1/runnable: 36.9/3.2, 19.3/8.0, 14.6/1.9) |
| rr | 256 | rust-release | 6.2 k | 0.47 | 41.03 ms | 42.18 ms | 42.89 ms | 0.94 | 15.23 | 8.6 | 21 | 3 | no (load1/runnable: 23.7/8.2, 16.3/6.5, 9.1/0.2) |
| rr | 256 | llvm-release | 6.2 k | 0.47 | 41.02 ms | 42.24 ms | 43.99 ms | 0.97 | 15.71 | 23.6 | 257 | 3 | 1/3 contended (load1/runnable: 25.9/11.9, 27.4/0.5, 21.4/3.6) |
| pipe | 1 | rust-release | 1.5 k | 0.12 | 41.01 ms | 42.06 ms | 42.33 ms | 0.05 | 3.25 | 3.8 | 21 | 3 | no (load1/runnable: 17.4/6.5, 18.6/2.5, 20.9/5.2) |
| pipe | 1 | llvm-release | 1.5 k | 0.12 | 41.00 ms | 42.07 ms | 42.47 ms | 0.04 | 2.58 | 3.0 | 2 | 3 | no (load1/runnable: 40.2/1.5, 50.6/3.6, 40.2/7.8) |
| pipe | 16 | rust-release | 24.7 k | 1.88 | 41.02 ms | 42.27 ms | 42.83 ms | 0.92 | 3.72 | 4.3 | 21 | 3 | no (load1/runnable: 8.8/1.6, 10.8/8.6, 4.0/3.5) |
| pipe | 16 | llvm-release | 24.7 k | 1.87 | 41.01 ms | 42.15 ms | 43.89 ms | 0.77 | 3.12 | 5.3 | 17 | 3 | no (load1/runnable: 7.6/3.5, 10.7/8.4, 3.7/0.4) |
| pipe | 256 | rust-release | 394.0 k | 29.95 | 41.02 ms | 42.71 ms | 47.08 ms | 12.52 | 3.17 | 8.6 | 21 | 3 | no (load1/runnable: 6.2/2.8, 10.0/2.1, 4.7/0.7) |
| pipe | 256 | llvm-release | 395.9 k | 30.09 | 41.01 ms | 42.18 ms | 43.04 ms | 13.84 | 3.49 | 39.6 | 257 | 3 | 1/3 contended (load1/runnable: 34.3/0.9, 40.2/1.4, 29.8/11.8) |
| connect | 1 | rust-release | 14.3 k | 0.21 | 60 µs | 129 µs | 375 µs | 5.42 | 37.82 | 3.8 | 21 | 3 | 1/3 contended (load1/runnable: 20.8/21.8, 21.6/2.4, 19.9/2.8) |
| connect | 1 | llvm-release | 14.0 k | 0.21 | 63 µs | 139 µs | 569 µs | 5.96 | 42.66 | 5.4 | 2 | 3 | no (load1/runnable: 23.3/1.5, 6.2/4.0, 21.6/3.9) |
| connect | 16 | rust-release | 68.9 k | 1.03 | 192 µs | 840 µs | 2.78 ms | 23.43 | 31.95 | 5.2 | 21 | 3 | 1/3 contended (load1/runnable: 19.2/5.8, 18.1/1.4, 21.4/10.6) |
| connect | 16 | llvm-release | 42.0 k | 0.63 | 334 µs | 1.65 ms | 5.97 ms | 19.45 | 47.02 | 10.9 | 6 | 3 | no (load1/runnable: 50.4/2.1, 61.0/5.2, 56.5/5.7) |

#### debug, stock client

| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| rr | 1 | rust-debug | 24.1 | 0.00 | 41.01 ms | 42.08 ms | 42.13 ms | 0.02 | 82.99 | 4.3 | 21 | 3 | no (load1/runnable: 6.5/1.2, 6.9/8.9, 6.2/0.3) |
| rr | 1 | llvm-debug | 24.2 | 0.00 | 41.02 ms | 42.26 ms | 42.75 ms | 0.01 | 41.32 | 3.1 | 2 | 3 | no (load1/runnable: 4.7/1.3, 5.0/1.3, 5.0/1.6) |
| rr | 16 | rust-debug | 386.2 | 0.03 | 41.01 ms | 42.43 ms | 43.05 ms | 0.26 | 67.43 | 5.0 | 21 | 3 | no (load1/runnable: 33.7/7.2, 19.5/4.6, 12.7/1.1) |
| rr | 16 | llvm-debug | 387.2 | 0.03 | 41.01 ms | 42.09 ms | 42.20 ms | 0.15 | 38.75 | 4.5 | 17 | 3 | no (load1/runnable: 27.7/9.5, 18.1/5.8, 10.8/0.1) |
| rr | 256 | rust-debug | 6.2 k | 0.47 | 41.03 ms | 42.58 ms | 43.39 ms | 2.63 | 42.61 | 9.0 | 21 | 3 | no (load1/runnable: 16.9/1.1, 13.0/6.5, 6.0/0.7) |
| rr | 256 | llvm-debug | 6.2 k | 0.47 | 41.03 ms | 42.42 ms | 43.11 ms | 1.90 | 30.79 | 24.7 | 257 | 3 | no (load1/runnable: 14.4/0.9, 12.2/6.8, 4.7/0.1) |
| pipe | 1 | rust-debug | 1.5 k | 0.12 | 41.01 ms | 42.99 ms | 43.48 ms | 0.17 | 11.02 | 4.4 | 21 | 3 | no (load1/runnable: 11.2/0.3, 10.1/4.7, 4.8/2.0) |
| pipe | 1 | llvm-debug | 1.5 k | 0.12 | 41.00 ms | 42.08 ms | 42.14 ms | 0.15 | 9.69 | 3.1 | 2 | 3 | no (load1/runnable: 10.6/1.4, 11.8/7.0, 4.4/2.5) |
| pipe | 16 | rust-debug | 24.7 k | 1.88 | 41.01 ms | 42.09 ms | 42.98 ms | 2.23 | 9.04 | 5.0 | 21 | 3 | no (load1/runnable: 7.9/2.1, 7.0/2.9, 5.8/1.3) |
| pipe | 16 | llvm-debug | 24.7 k | 1.88 | 41.02 ms | 42.12 ms | 42.85 ms | 2.41 | 9.72 | 5.5 | 17 | 3 | no (load1/runnable: 6.5/1.2, 5.2/1.9, 5.1/1.4) |
| pipe | 256 | rust-debug | 394.8 k | 30.00 | 41.01 ms | 42.81 ms | 43.98 ms | 44.41 | 11.32 | 8.7 | 21 | 3 | no (load1/runnable: 7.2/0.7, 6.6/7.3, 5.2/3.4) |
| pipe | 256 | llvm-debug | 394.5 k | 29.98 | 41.01 ms | 44.00 ms | 46.05 ms | 44.69 | 11.33 | 40.8 | 257 | 3 | no (load1/runnable: 5.8/1.5, 5.2/7.5, 4.2/3.0) |
| connect | 1 | rust-debug | 11.6 k | 0.17 | 83 µs | 130 µs | 174 µs | 10.85 | 93.22 | 4.4 | 21 | 3 | no (load1/runnable: 4.0/0.8, 4.6/6.2, 4.4/2.3) |
| connect | 1 | llvm-debug | 13.1 k | 0.20 | 73 µs | 129 µs | 210 µs | 7.35 | 57.01 | 5.6 | 2 | 3 | no (load1/runnable: 5.1/0.3, 5.5/1.9, 5.2/1.1) |
| connect | 16 | rust-debug | 70.8 k | 1.06 | 194 µs | 712 µs | 1.27 ms | 55.71 | 78.72 | 5.5 | 21 | 3 | no (load1/runnable: 3.9/0.8, 4.8/2.6, 4.3/1.2) |
| connect | 16 | llvm-debug | 39.7 k | 0.60 | 393 µs | 528 µs | 710 µs | 24.09 | 60.66 | 10.0 | 5 | 3 | no (load1/runnable: 5.2/2.0, 5.3/2.8, 4.9/1.3) |

#### cells with a contended run

- quickack=False llvm-release pipe 256
- quickack=False llvm-release rr 256
- quickack=False rust-release connect 1
- quickack=False rust-release connect 16
- quickack=True llvm-debug pipe 256
- quickack=True llvm-release pipe 16
- quickack=True llvm-release rr 16
- quickack=True rust-release connect 16
- quickack=True rust-release pipe 1
- quickack=True rust-release pipe 256

## Where the time goes

### User against kernel CPU (`/proc/<pid>/stat` deltas, quickack, 16 connections)

| run | replies | user s | sys s | user µs / reply | sys µs / reply |
|---|---:|---:|---:|---:|---:|
| llvm pipe 16 (`tp/prof/*.cpusplit`) | 5.26 M | 14.8 | 34.2 | 2.8 | 6.5 |
| rust pipe 16 | 8.73 M | 22.0 | 24.2 | 2.5 | 2.8 |
| llvm rr 16 | 2.11 M | 7.2 | 20.6 | 3.4 | 9.8 |
| rust rr 16 | 1.72 M | 8.2 | 12.4 | 4.7 | 7.2 |

`perf stat -e instructions:u,cycles:u` per reply:

| run | instructions | cycles |
|---|---:|---:|
| llvm pipe 16 | 18.4 k | 12.5 k |
| rust pipe 16 | 16.7 k | 12.2 k |
| llvm rr 16 | 19.2 k | 17.6 k |
| rust rr 16 | 20.9 k | 25.6 k |

The kernel takes about 70% of LLVM server CPU (pipe 70%, rr 74%) and about 50-60% of Rust
server CPU. In pipe 16 the user-space cost per reply is a tie, so the 2.3× gap is all kernel
time.

### Syscalls per 1000 replies

| run | sendto | recvfrom | futex | epoll_wait | write |
|---|---:|---:|---:|---:|---:|
| llvm pipe 16 | 2003 | 17.4 | 1.3 / **63.0** | - | - |
| rust pipe 16 | 2002 | 17.4 | 0.6 / 10.7 | 127.8 / 3.6 | 28.0 / 0.6 |
| llvm rr 16 | 2000 | 1000 | 6.1 / 32.1 | - | - |
| rust rr 16 | 2000 | 1000 | 655 / 663 | 136.8 / 97.9 | 1.2 |

How the counts were taken:

- `strace -c -f`, with the server started under strace: `tp/strace.sh`, output in
  `tp/prof/*.strace.txt`. Full tracing slows the server to 28-141 k replies/s.
- Where a cell has two values, the second comes from
  `strace -c -f --seccomp-bpf -e trace=futex,epoll_wait,write`: `tp/futex.sh`, output in
  `tp/prof/*.futex.txt`. Only those calls stop, so the server runs at about 1.2 M (pipe) and
  2.1 M (rr) replies/s, near full speed. The futex counts under full tracing are understated.
- Both targets make exactly **2 sendto per reply** (reply line, then prompt). In pipe they read a
  4 KiB batch of about 57 requests per `recvfrom`.

### Context switches per 1000 replies

Summed over all server threads from `/proc/<pid>/task/*/status`, script `tp/csw.sh`.

| run | voluntary | involuntary |
|---|---:|---:|
| llvm pipe 16 | 5.3 | **557.5** |
| rust pipe 16 | 11.7 | 77.1 |
| llvm rr 16 | 677.2 | 591.3 |
| rust rr 16 | 270.9 | 17.5 |

Verified: the LLVM build makes 6-7× more context switches and 6× more futex calls in pipe 16,
with identical send and recv counts.

Inferred, not verified: the extra kernel time per reply is wakeup preemption between 16 server
and 16 client threads ping-ponging through 114 small sends per batch, plus contention on the
global handle lock (see change 2).

### Top 15 functions, LLVM release, pipe 16

`perf record -e cycles:u -g`, self time. Share of **user** samples, and user is about 30% of
server CPU.

| % | function |
|---:|---|
| 19.03 | `std.protocol.telnet.session` |
| 14.64 | `std.protocol.telnet.decode` |
| 13.49 | `telos_rt::arena::alloc` |
| 11.23 | `telos_rt_arena_due` |
| 4.95 | `core::str::lossy::Utf8Chunks::next` (from `telos_rt_text_from_bytes_lossy`) |
| 4.15 | `telos_rt::net::lookup` |
| 3.33 | `telos_rt_alloc` |
| 2.70 | `telos_rt_tcp_write_all` |
| 2.88 | libc `__memcpy_avx_unaligned_erms` (0x176411 + 0x176400) |
| 1.70 | libc `send` |
| 1.60 | `core::str::converts::from_utf8` |
| 1.47 | `telos_rt_arena_release` |
| 1.28 | `telos_rt::arena::Arena::release` |
| 1.15 | `std.protocol.telnet.answer_line` |
| 1.03 | `std::sys::sync::mutex::futex::Mutex::lock_contended` (handle table) |

In rr 16 the order is the same: session 14.1, arena::alloc 11.7, decode 10.9, arena_due 7.3,
lookup 4.8, Utf8Chunks 3.6, tcp_read 3.3, telos_rt_alloc 3.1, write_all 2.8, memset 2.5 (zeroing
the 4 KiB read buffer).

### Top 15 functions, Rust release, pipe 16

Same method. User is about 48% of server CPU.

| % | function |
|---:|---|
| 16.13 | `terminald::main::{closure}` (session future, inlined) |
| 13.43 | `telos_std::protocol::telnet::step` |
| 9.37 | libc `malloc` |
| 8.86 | `telos_std::protocol::telnet::decode` |
| 7.97 | libc `free` |
| 4.56 | `core::str::lossy::Utf8Chunks::next` |
| 1.99 | `String::clone` |
| 1.48 | libc memcpy |
| 1.48 | `__rdl_alloc` |
| 1.32 | libc `send` |
| 1.27 | `telos_std::protocol::telnet::render` |
| 1.19 | `tokio::sync::batch_semaphore::Acquire::poll` |
| 1.10 | `tokio::sync::batch_semaphore::Semaphore::add_permits_locked` |
| 1.08 | `tokio::sync::Mutex<OwnedWriteHalf>::lock` |
| 1.06 | `tokio::net::TcpStream::poll_write_priv` |

In rr 16, tokio scheduling shows up as well: `worker::Context::run` 3.7 and `steal_into` 3.5.

### Instruction counts per reply

Callgrind, rr 1, 3 s, `tp/cg.sh`; output in `tp/prof/*.callgrind.out.1`, call counts from
`tp/cgcalls.py`.

| | LLVM | Rust |
|---|---:|---:|
| Ir per reply | 23.3 k | 25.3 k |
| decode calls | 71 (one per request byte) | 71 decode + 71 step |
| loop-region checks | 73 `telos_rt_arena_due` | - |
| allocations | 39 arena (34 `telos_rt_alloc` from generated code) | 37 malloc + 38 free |
| memcpy | 33 | 31 |
| other | 3 `text_from_bytes_lossy`, 2 `tcp_write_all`, 1 `tcp_read` (+ 1 calloc 4 KiB + memset + free) | 15 `String::clone` |

LLVM `telos_rt_alloc` callers per reply: render 8, parse 6, answer_line 4, first_word 4,
rest_after_word 4, session 3.2, respond 3, execute 2.

Shares of Ir: session 18.0%, memset 17.7%, arena_due 16.0%, decode 13.1%, arena::alloc 12.8%,
Utf8Chunks 4.4%. Callgrind counts `rep stos` per byte, so the memset share is overstated; perf
puts it at 2.5%.

## Ranked changes that would make the LLVM build faster than the Rust build

The estimates are shares of LLVM release server CPU in pipe 16 (user ≈ 30%, sys ≈ 70%) unless a
row says otherwise. The counts are measured. The shares are inferred and labelled "est.".

Slice 4 moves both targets onto the blocking `telos-io`. After that, changes 1-3 (substrate)
help both targets, and the target-specific levers are 4-9 (generator and runtime ABI). Re-run
this baseline with the ported `telos-load` right after slice 4 lands, before any change.

1. **Coalesce writes: one send per reply, or one per read batch.**
   - std: `answer_line` in `std/protocol/telnet.telos` sends `render(reply)` and then
     `write_text(prompt)`. Make it one `write_all` of reply plus prompt.
   - substrate: give `tcp_write_all` (`telos-rt/src/net.rs` `write_all`, `telos-io` after
     slice 4) a per-stream buffer. Flush it before a read on that stream, on close and release,
     and above 64 KiB. Then set `TCP_NODELAY` on accepted and connected sockets.
   - Measured: 2 sendto per reply, which is all of the kernel time in pipe. The 41 ms stall for
     every non-quickack client: rr 1 gives 24 replies/s against 56 k/s with quickack.
   - Effect: sendto drops 2.0 → 1.0 per reply in rr and 2.0 → about 0.02 in pipe. Est. −50% CPU
     in pipe 16 and −25-30% in rr 16. Stock-client throughput goes up about 1000×.
   - Probably also removes most of the 557 involuntary switches per 1000 replies, since every
     send can wake the client (inferred).
2. **Remove the global handle-table lock from every IO call.**
   - `telos_rt::net::lookup` locks `HANDLES: Mutex<Vec<Option<Handle>>>` and clones an `Arc` on
     every read, write and close.
   - Measured: lookup 4.2% + `lock_contended` 1.0% of user, and futex 63 per 1000 replies against
     10.7 for Rust.
   - Fix: make the handle an owned pointer (`Arc::into_raw`, freed in `tcp_release`), or keep
     slot-stable `AtomicPtr` slots, or use an `RwLock` or sharded table.
   - Est. −5-10% CPU in pipe 16, more at 256 connections.
3. **Thread per connection: `telos_rt_spawn` in `telos-rt/src/task.rs` and the accept loop in
   `telos-llvm/src/lib.rs`.**
   - Each accepted stream gets a new OS thread with a 2 MiB default stack, and `accept` blocks on
     a `sync_channel` until `spawn_ready`.
   - Measured: connect 16 gives 43.8 k against 82.2 k sessions/s, at 48.5 against 34.7 ms CPU per
     1000 sessions. Peak RSS at 256 connections is 42.8 MiB against 10.7 MiB.
   - Fix: a pool of parked task threads that keep their arenas, a small stack (for example
     256 KiB via `Builder::stack_size`), and hand the environment over without blocking accept.
   - Est. connect 16 ×1.5-2 and RSS ÷3.
   - The structural fix is slice 5's event loop in `telos-rt`. Rust-target tokio shows 6-7× fewer
     context switches.
4. **`telos_rt_arena_due` on every loop back-edge (`lower.rs` `region_back_edge`).**
   - Measured: 73 FFI calls per reply, one per input byte in `session`'s inner `while`. Each one
     is a TLS lookup and a `RefCell` borrow. That is 11.2% of user in pipe, 7.3% in rr, and 16% of
     Ir.
   - Fix: emit the check inline. Fetch a pointer to the task's arena state once at region entry
     (a new `telos_rt_arena_state()`), then do a load, a subtract and a compare per back-edge. Or
     skip the check when the body did not allocate since the last check.
   - Est. −10% of user, about −3% of CPU in pipe 16 and −2-4% in rr.
5. **Allocation through FFI.**
   - Measured: 39 `telos_rt_alloc` → `arena::alloc` per reply, each through TLS, `RefCell` and
     `Vec::last_mut`. That is 16.8% of user in pipe and 14.8% in rr.
   - Fix: inline the bump fast path in the generated IR. Cursor and limit live in the per-task
     state from change 4; call the runtime only when the chunk is full.
   - Est. −8% of user, about −2.5% of CPU.
6. **Copies the generator emits (`lower.rs` copy rule).** Measured: 34 allocations and 28
   memcpy per reply from generated code. Constructs:
   - a. `copy.Input` of the fresh `tcp_read` result in `session` (`match input`): 1 allocation
     plus a copy of up to 4 KiB per read.
   - b. `copy.std.console.Console` and `copy.List<Byte>` of `line` at every
     `answer_line(stream, application, line)` call.
   - c. `copy.std.console.Reply` before `render(reply)` and `copy.Text` of `application.prompt`
     before `write_text` in `answer_line`.
   - d. `Text.to_bytes()` copies in `write_text`, `render`, `first_word` and `rest_after_word`.
     Text and Bytes share a layout, so `to_bytes` on an owned temporary can be free.
   - e. A per-byte `copy.std.protocol.telnet.Event` of `next.event`.

   Fix: treat the last use of a place as a move, and add a borrowed-parameter class for
   parameters a callee neither mutates nor lets escape. That changes the "callee owns" rule of
   `docs/llvm.md` section 3. The Rust target clones the same values (37 malloc per reply), so
   this is a structural lead the LLVM target can take. Est. allocations 39 → about 10 per reply;
   together with change 5, −10% of user, about −3% of CPU in pipe and more in rr.
7. **Read path (`telos_io::read_buffer` + `TelosInput::data`).**
   - Measured: each read callocs and zeroes 4 KiB, then copies into the arena (`memset` 2.5% of
     user in rr).
   - Fix: allocate the arena block first, `recv` into uninitialised memory, and return the tail.
   - Est. −1-3% in rr, <1% in pipe.
8. **Out-of-line `decode` with a 48-byte `Step` returned through memory.**
   - Measured: 14.6% of user (pipe). It cannot be inlined: it is recursive in the
     `CarriageReturn` arm and has external linkage.
   - Fix: emit functions no other module calls as `internal`. Link `std.ll`, `program.ll` and
     `main_*.ll` into one module (llvm-link, or `-flto=thin`) so that `decode`, `step`,
     `execute`, `parse` and `first_word` inline. Turn the self-call into a loop.
   - Est. −3-6% of user.
9. **`telos_rt_text_from_bytes_lossy`, 3 per reply.**
   - Measured: `Utf8Chunks::next` + `from_utf8` + `from_utf8_lossy` ≈ 7.5% of user, plus a
     copy. The Rust target pays the same, 4.6%.
   - Fix: the callee owns its argument, so when `str::from_utf8` succeeds, return the input
     buffer itself without copying.
   - Est. −5% of user, about −1.5% of CPU.

Expected order of effect: change 1 (the largest, both targets), then changes 2 and 3 (they decide
pipe 16 and connect 16, which the LLVM build loses today). Changes 4-9 together remove about half
of the user-space instructions per reply. Those last six keep the LLVM build ahead after slice 4,
when the substrate is shared.

## Reproduce

- Build: `tp/build_servers.sh`, then
  `cargo build --release -p telos-load` in `snap3` with `CARGO_TARGET_DIR=…/target-k`.
- Matrix: `tp/matrix.sh RESULTS.jsonl 3`. Re-runs: `tp/rerun.sh CELLS RESULTS.jsonl`. Tables:
  `python3 -I tp/summarize.py RESULTS.jsonl [RERUN.jsonl]`.
- Profiles:
  - `tp/profile.sh BUILD PORT`: perf record and perf stat. Its strace part fails under
    `ptrace_scope` 1.
  - `tp/strace.sh BUILD PORT` and `tp/futex.sh BUILD PORT`: syscalls.
  - `tp/csw.sh BUILD PORT WORKLOAD`: context switches.
  - `tp/cg.sh BUILD PORT`: callgrind. The server ignores SIGINT, so dump with
    `callgrind_control -d PID` and stop with SIGTERM.
- Reports: `perf report -i tp/prof/BUILD-pipe.perf.data --no-children --sort symbol --stdio -g none`.
  Release binaries have no frame pointers, so the call graphs are empty and only self time is
  used.
