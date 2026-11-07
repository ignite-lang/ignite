---
format: aep.planning-md/3
id: review-result:adversary-http-chunked-pass-2
kind: review-result
status: active
title: Adversary, http-chunked-adversary-findings, pass 2
relations:
- reviews: story:http-chunked-adversary-findings
revision: 1
---
unit: story:http-chunked-adversary-findings, worktree http-chunked at d87d4c3a plus 2 uncommitted test files
verdict: NEEDS-CHANGE
cases: executed 78→78 inline per target; cargo `http_chunked_adversary::` 0→1, red 1 (against the mutant std)
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (scratch unit-http-chunked/adv2)
needs-coordinator: none

The round-2 move holds on both targets and the three-read bound is true and tight (worst split 196,612 bytes against 196,614), but the 8 s timing test does not catch a reintroduced copy on Rust.

## Case

- `crates/ignite/tests/http_chunked_adversary.rs::the_chunk_decoder_copies_no_bytes_parameter_on_either_target`: reads the generated code of the todo example. Rust: `read_chunk_data` takes `data: Vec<u8>`, no `data.clone()`/`buffer.clone()`/`body.clone()` in the decoder's functions, `read_chunked` passes `data` by value. LLVM: no `copy.List<Byte>` from a `%p` parameter in those functions. Red against the mutant (`http.ignite:1321` `input.len()` → `data.len()`): `read_chunk_data borrows its input buffer`; green on HEAD.

## Suite (load 33)

- `http_chunked_adversary::` 1 passed; http.ignite 78 passed per target; `conformance::http_matrix_lists_the_chunked_and_host_gaps` passed.

## Findings

- A1 `std/protocol/http.ignite:2331`: on the mutant std, `--target rust` passes the 8 s timing test 3 of 3 in 3.56 to 3.81 s at load 35 to 37; the move is guarded only on LLVM. Fix: keep the timing test for LLVM, add the structural check.
- A2 `docs/slice.md:6538`: the bound counts values in use; LLVM's `ignite.arena_due` (`crates/ignite-llvm/src/glue.rs:82`) releases a loop arena only past max(4 × kept, 64 KiB), and Rust's `extend_from_slice(&checked::slice(&input, ..))` builds a temporary of less than two reads. RSS not measured.

## Not broken

No per-chunk copy on either target; Content-Length path moves `data`; pipelined requests and leftover bytes (6 probes, both targets); R6_2_5 matrix.

## Coordinator disposition

A1: the adversary's test committed as `296433bc`; A2: 19.22 states both effects in the same commit.

```json
[
  {"id": "A1", "severity": "medium", "title": "the 8 s timing test does not catch a reintroduced per-chunk copy of the input buffer on the Rust target", "evidence": "std/protocol/http.ignite:2331; mutant passes on Rust 3 of 3 in 3.56 to 3.81 s at load 35 to 37", "test": "crates/ignite/tests/http_chunked_adversary.rs::the_chunk_decoder_copies_no_bytes_parameter_on_either_target", "pre_existing": false},
  {"id": "A2", "severity": "low", "title": "19.22's bound counts values still in use; LLVM arenas keep up to 4x that as garbage until release, and Rust briefly copies the chunk", "evidence": "docs/slice.md:6538; crates/ignite-llvm/src/glue.rs:82", "test": "none", "pre_existing": false}
]
```
