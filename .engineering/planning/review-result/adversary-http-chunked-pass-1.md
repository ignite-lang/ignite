---
format: aep.planning-md/3
id: review-result:adversary-http-chunked-pass-1
kind: review-result
status: active
title: Adversary, http-chunked-adversary-findings, pass 1
relations:
- reviews: story:http-chunked-adversary-findings
revision: 1
---
unit: story:http-chunked-adversary-findings, worktree http-chunked at 54cbcadc plus 2 uncommitted inline tests
verdict: NEEDS-CHANGE
cases: executed 76→78 per target, red 2 (LLVM), 1 (Rust; the timing case is borderline there)
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (scratch dir unit-http-chunked/adv1)
needs-coordinator: whether the timing case should stay in the gate at its 2.5 s bound

## Cases (std/protocol/http.ignite, uncommitted)

- "adversary: a chunk read whole but for its LF keeps less than two reads of input and the line" (:2308): 65535 consumed bytes (compaction only at `at >= READ_SIZE`, :1373), the line `FFF0\r\n`, 65520 bytes of data and a lone `\r`, then one read of 65536 bytes starting with `\n`; asserts input kept < `2 * READ_SIZE + line.len()`. Red on both targets: `the decoder holds 196598 bytes of input besides the body`.
- "adversary: the body limit in one-byte chunks is decoded in under 2.5 s" (:2331): 1,048,576 one-byte chunks through `next_of`. At load 29: Rust 2518 ms (borderline), LLVM 19331 ms; on a b23f827e std copy Rust 0.71 s, LLVM 1.37 s.

## Suite

- `ignite test std/protocol/http.ignite --target rust`: 77 passed; 1 failed, exit 1.
- `--target llvm`: 76 passed; 2 failed, exit 1.

## Findings

- F1 `std/protocol/http.ignite:1319` `var input = data`: `read_chunk_data` borrows `data` and copies it per chunk (generated Rust `let mut input = data.clone();`, LLVM `copy.List<Byte>`). One-byte chunks, base Rust/LLVM → head Rust/LLVM: 64k 37/73 → 181/831 ms; 256k 158/300 → 687/3550 ms; 1M 657/1153 → 3124/20885 ms. Reaches any chunked request to `Http.serve` (todod). Also contradicts 19.22's "a chunk is never held twice". Fix: move `data` in, or do the split-path work in `read_chunked`.
- F2 `docs/slice.md:6540` against `http.ignite:1373`: the new bound "less than two reads and the chunk-size line" does not hold; up to about three reads (196,598 bytes). Code unchanged from PR C; the bound text is new.

## Not broken

Chunk split across reads, CR LF split across reads, zero-length last chunk with trailers and extensions, hex overflow, body at and over the limit, split-path mutants (dropped leftover, `from = 0`, `<=` → `<`, append whole read). R6_2_5: claimed nowhere else; level `MustNot` matches. Not run: `conformance::`, the goldens.

```json
[
  {"id": "F1", "severity": "high", "title": "read_chunk_data copies the input buffer for every chunk: the body limit in 1-byte chunks went from 0.66 s / 1.15 s to 3.1 s / 20.9 s (Rust / LLVM)", "evidence": "std/protocol/http.ignite:1319; ignite test --target llvm: 1048576 one-byte chunks took 18854 ms; b23f827e: 1.37 s LLVM, 0.71 s Rust", "test": "adversary: the body limit in one-byte chunks is decoded in under 2.5 s, std/protocol/http.ignite:2331", "pre_existing": false},
  {"id": "F2", "severity": "low", "title": "the 19.22 bound 'input of less than two reads and the chunk-size line' does not hold: up to about three reads are held", "evidence": "docs/slice.md:6540; std/protocol/http.ignite:1373; the decoder holds 196598 bytes of input besides the body", "test": "adversary: a chunk read whole but for its LF keeps less than two reads of input and the line, std/protocol/http.ignite:2308", "pre_existing": false}
]
```
