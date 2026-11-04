# Store reads without a second heap copy (story:store-borrowing-reads)

2026-11-04. Before is `2fd3397c` (`main`), after is the same tree with the change to
`crates/ignite-io/src/store.rs` (`read`, `read_values`, `replace_with`) and
`crates/ignite-rt/src/store.rs` (`store_get`, `store_values` and `store_update` use them).
Program: ember, the RESP2 server of the examples repository, `ignite build --release`, both
targets. Each tree built its own compiler, runtime archive and `ignite-io`.

## Massif of the ember flush (the acceptance)

`valgrind --tool=massif --threshold=0.5`, workload two rounds of emberload `4 2500 50 set`
then `FLUSHDB`, LLVM target. Peak snapshot, entries of the store and its glue:

| peak snapshot | before | after |
|---|---|---|
| heap at peak | 34,900,514 B | 23,468,201 B |
| `store_values` → `copy_to_heap.ember.Bucket` (heap copy of every value) | 10,000,001 + 640,064 + 240,024 B | absent |
| `store_values`'s `Vec` of heap copies | 480,048 B | absent |
| `store_values` → `copy_from_heap.ember.Bucket` (arena copy) | 10,682,368 B | 10,682,368 B |
| `store_put` → `copy_to_heap.ember.Bucket` (the stored values) | 10,000,001 B | 10,000,001 B |

At peak the values are held once by the store and once in the flushing task's arena; the heap
copy in between is gone. The Rust target's peak is unchanged (22,334,750 to 22,308,496 B): its
generated code calls the owned forms `get`, `values` and `update`, which copy as before.

## dhat, same workload

| target | before | after |
|---|---|---|
| LLVM, total | 118,999,708 B in 185,189 blocks | 96,162,754 B in 105,193 blocks |
| LLVM, at t-gmax | 34,888,426 B in 81,974 blocks | 23,462,353 B in 41,985 blocks |
| Rust, total | 264,556,875 B in 1,148,417 blocks | 264,470,849 B in 1,148,339 blocks |

## Ember tournament

Interleaved A/B, 5 rounds, pair order alternating, load average 7.5 to 15.4; medians, and the
median of the per-round after/before ratios. Memory: the tournament's memory phase (`16 20000
50`, then 10 rounds of `4 2500 50 set` and `FLUSHDB`), settled is VmRSS 1.5 s after the last
flush. Allocations: valgrind memcheck over `4 2000 10; 2 1000 10 set; FLUSHDB`.

| target | measure | before | after | ratio |
|---|---|---|---|---|
| LLVM | heap allocs | 172,621 | 68,596 | 0.397 |
| LLVM | heap bytes | 77,563,580 | 72,904,519 | 0.940 |
| LLVM | peak RSS (VmHWM) KiB | 100,132 | 28,776 | 0.287 |
| LLVM | RSS right after the last flush KiB | 100,916 | 29,824 | 0.295 |
| LLVM | RSS 1.5 s after the last flush KiB | 4,832 | 4,780 | 0.989 |
| LLVM | ops/s, 1 client | 56,980 | 52,631 | 0.980 |
| LLVM | ops/s, 16 clients | 359,147 | 394,088 | 1.087 |
| LLVM | ops/s, 16 clients, pipeline 50 | 709,534 | 802,005 | 1.111 |
| Rust | heap allocs | 748,449 | 748,460 | 1.000 |
| Rust | peak RSS KiB | 48,312 | 48,184 | 0.995 |
| Rust | ops/s, 16 clients, pipeline 50 | 544,217 | 541,455 | 0.993 |

- The LLVM peak RSS fell by 71 MB, more than the 11 MB fewer heap bytes at the massif peak.
  Inferred, not measured: the heap copies, freed between the stored values, kept glibc's
  per-thread arenas from shrinking until the rate-limited trim ran; the settled RSS is the same
  in both trees.
- The Rust rows are a control: the same allocation count and memory within noise.
