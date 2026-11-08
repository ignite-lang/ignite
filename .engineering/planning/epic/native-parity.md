---
format: aep.planning-md/3
id: epic:native-parity
kind: epic
status: draft
title: Ignite programs within 1.1x of plain Rust
revision: 1
---
# Epic: native-parity

## Outcome

Every native benchmark cell (`docs/quality.md` rows 3.3, 3.7 to 3.9) and the codecs compute benchmark are within 1.1x of plain Rust on the LLVM target: throughput at least 0.909 of plain Rust, p99 latency, peak RSS and compute time at most 1.1x.

## Context

- Goal set by Timo, 2026-11-08: "max 1.1x native rust".
- Ratios on the EC2 host (5 passes, LLVM / Rust target throughput): terminal rr-1 0.906/0.886, rr-64 0.858/0.824, connect-8 0.766/0.946; todo get-1 0.899/0.809, get-64 0.869/0.703, patch-16 0.576/0.567 (p99 2.19x); RSS 1.2 to 2.9x; codecs (SHA-1 and base64, 1 MiB) LLVM 2.94x, Rust target 1.57x.
- Diagnosis 2026-11-08 (local, `perf stat` per reply, `strace -c`, `perf record`; scripts in the session scratch `perf-diagnosis/`). Cost per reply against plain Rust, user instructions / task-clock, LLVM: todo get-64 3.90/1.52, get-1 3.73/1.52, patch-16 3.25/1.46, terminal rr-1 3.27/1.24, rr-64 3.03/1.11-1.24, connect-8 3.84/1.16, codecs 2.18/2.54-2.70.
- Fairness: todo get and patch do the same work as the reference (the reference also writes every change to a versioned file, no fsync); terminal differs by one `poll(POLLPRI)` per read; codecs is fair as written but the language lacks `u32`, rotate and a capacity hint.
- Findings by layer, with evidence:
  - Directory API: patch makes 76 syscalls per reply against 16 (`openat` + `close` 55.7 against 4.0); 78 % of patch CPU is kernel.
  - Runtime: `ignite_rt_arena_state` behind the panic guard is 11.5 to 12 % of get-64 user time, `install_hook` 3.1 to 3.4 %; a probe without them cut instructions 15.5 % (get-64) and 26 % (patch-16). Stream `lookup` 4.75 % of rr-64. Codecs: 30.6k page faults against 1.0k, 200 `brk`.
  - std text: naive search 19.8 % of get-64 user time, the UTF-8 re-check 9.5 %.
  - LLVM codegen: no alias annotations; a per-iteration region check in loops (probe: -6.8 % codecs instructions); failure paths not cold.
  - Telnet: `encode_input` 9.9 % and `decode_within` 9.6 % of rr-64 user time.
  - Memory: 64 threads x 128 KiB of read buffer and spare arena chunk = the 8 MiB RSS gap of get-64.
  - Rust target: `malloc`/`free` 24 to 30 % of get-64 user time (literal `String`s, `to_bytes` clones, `Arc` closures).
  - Pipeline: `clang -O2` per unit, no LTO; O3 or LTO about 5 % each.
  - Thread per connection: 1.13 context switches per reply against 0.024 for tokio.
- Expected after the first six stories: patch-16 about 1.1x, get-1 about 0.95, terminal about 1.08x CPU per reply; get-64 needs readiness IO; codecs needs `u32`, rotate, fixed-size lists.

## Stories

- Wave P1: `story:fs-resolve-beneath`, `story:rt-hot-path`, `story:text-fast-search`, `story:llvm-alias-and-cold`, `story:telnet-hot-path`, `story:rustgen-fewer-allocations`.
- Later: `story:readiness-io`, `story:fixed-width-ints`, `story:spawn-without-handshake`, `story:llvm-lto-runtime`.
