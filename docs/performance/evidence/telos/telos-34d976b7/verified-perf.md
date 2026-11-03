# Verified: R4 perf (verifier V4)

Snapshot `snap-v/` (11:53). Harness in `snap-v/verify-v4/`: `bench.telos`, `bench2.telos`, `bench3.telos`
(one `Client` per workload, input on stdin), `gen.py` (inputs of 1, 4, 16, 64, 256 KiB), `run.sh`
(times each client). Rust target `--release`; LLVM target `--release` (`-O2`) linked against a
release `libtelos_rt.a` (`target-v/release/`). Raw timings: `verify-v4/bench1.out`, `bench2.out`,
`bench3.out`. Wall time includes process start (about 2 ms).

## Measurements (seconds; growth = time ratio for 4x input)

| workload | target | 16 KiB | 64 KiB | 256 KiB | growth 64->256 |
|---|---|---|---|---|---|
| `json.parse` of `[n,n,...]` (45541 items at 256 KiB) | rust | 0.009 | 0.097 | 6.154 | 63x |
| same | llvm | 0.056 | 0.880 | 16.992 | 19x |
| `text.find(input, "#")`, no match | rust | 0.017 | 0.125 | 2.759 | 22x |
| same | llvm | 0.003 | 0.003 | 0.007 | linear |
| `text.split(input, ",")`, 32769 pieces | rust | 0.012 | 0.179 | 3.927 | 22x |
| same | llvm | 0.029 | 0.589 | 8.863 | 15x |
| `text.repeat("x", len)` | rust | 0.009 | 0.157 | 2.759 | 18x |
| same | llvm | 0.006 | 0.182 | 15.123 | 83x |
| `byte_at(bytes, i)` helper called per byte | rust | 0.006 | 0.083 | 1.714 | 21x |
| same | llvm | 0.002 | 0.002 | 0.003 | linear |
| `List<Int>.push` n/8 times in a loop | rust | 0.002 | 0.002 | 0.002 | linear |
| same | llvm | 0.002 | 0.050 | 0.610 | 12x |
| `List<Text>.push("ab")` n/8 times | rust | 0.002 | 0.002 | 0.003 | linear |
| same | llvm | 0.016 | 0.300 | 4.867 | 16x |
| `text.to_lower(input)` (Bytes push per byte) | rust | 0.008 | 0.003 | 0.004 | linear |
| same | llvm | 0.005 | 0.689 | 6.971 | 10x |

## Verdicts

| # | finding | verdict | severity | evidence | fix direction |
|---|---|---|---|---|---|
| 1 | json parse copies the whole input per token (Rust) | CONFIRMED | high | Rust: 0.097 s at 64 KiB, 6.15 s at 256 KiB (63x for 4x input; past 128 KiB each clone also goes through mmap). Generated `out-rust/std/src/lib.rs:380-393`: `parse_value(bytes.clone(), ...)`, `skip_space(bytes.clone(), ...)` twice per element. LLVM is also quadratic (17.0 s at 256 KiB) but from NEW-1, not from parameter copies: perf on LLVM 64 KiB shows 58 % in `copy.std.json.Json` | pass read-only parameters by reference in rustgen (finding 3); fix NEW-1 for LLVM |
| 2 | text find/split/lines clone haystack and needle per position (Rust) | CONFIRMED | medium | Rust find 2.76 s, split 3.93 s at 256 KiB, quadratic; `lib.rs:956` generated `bytes_match_at(bytes.clone(), needle.clone(), at.clone())`. The reviewer's suspicion for LLVM is refuted: LLVM find is linear (0.007 s at 256 KiB, `reads_only` borrows). LLVM split is still quadratic (8.9 s) through NEW-1 (`pieces.push` in a loop) | same as 3; LLVM split via NEW-1 |
| 3 | Rust generator clones every place used as a value, including read-only arguments | CONFIRMED | high (root cause of 1, 2, 6) | `crates/telos-rustgen/src/lib.rs:984-986` (`if clone && self.is_place(expr) { ... .clone() }`), args `:980-981`. Measured: a one-line `byte_at(bytes, i)` helper called per byte costs 1.71 s on 256 KiB in Rust, 0.003 s in LLVM | compute read-only params as `telos-llvm/src/lib.rs:80-92` does and emit `&[u8]`/`&str`/`&[T]` parameters, or switch `Bytes`/`Text`/`List` to a shared buffer (`Rc<[u8]>`) |
| 4 | Rust target moves a Text out of `List.at` | CONFIRMED | medium | `verify-v4/at_move.telos` (`let line = lines.at(0)` with `lines: List<Text>`) -> `telos emit` gives `let line = lines[(0) as usize];`; `telos build --target rust` fails: `error[E0507]: cannot move out of index of Vec<String>` at `src/lib.rs:4:16`, `cargo build failed with status 101` | in `rustgen/src/lib.rs:1318` emit `{r}[i].clone()` when the element type is not Copy (or treat `Index`/`at` as a place in `is_place`) |
| 5 | Text repeat is quadratic in result length | CONFIRMED | low | Rust 2.76 s at 256 KiB (18x per 4x); generated `out = format!("{}{}", out, text)` (`lib.rs:1102`; a fresh copy, not a `.clone()` as the reviewer says). LLVM 15.1 s (83x): NEW-1 compacts `out` on every iteration on top of the concat copy (`telos-rt/src/text.rs:45` plus `TelosList::from_bytes`, `telos-rt/src/lib.rs:83-86`: two copies per `+`) | build into a `Bytes` with `append`, or a repeat primitive |
| 6 | json member/encode clone each subtree per call or level | CONFIRMED (code) | low | generated `member`: `match value.clone()`, `for pair in (members.clone())`, `pair.clone()` (`out-rust/std/src/lib.rs:435-448`), so three copies of the object per accessor on top of the caller's argument clone; `encode`: `match value.clone()`, `items.clone()`, `item.clone()` per level (`:135-157`). Not timed | same as 3; match on a reference |
| 7 | telos-rt tcp read takes a `maximum`-sized block per read, not shrinkable | CONFIRMED (code) | low | `telos-rt/src/net.rs:233-244` (`arena::alloc(size, 1)`, `trim`), `arena.rs:184` (size > `LARGE` = 16 KiB gets its own block), `arena.rs:329-335` (`trim` only moves the cursor of the current chunk). std callers pass 4096. Not timed | for reads above `LARGE`, read into a stack or reused buffer and copy `count` bytes into the arena |
| 8 | const_cycles BFS from every constant | CONFIRMED (code) | low | `telos-check/src/lib.rs:1049-1052`; O(C x (C + E)). Not timed | one Tarjan/colouring DFS over the graph |
| 9 | checker fixpoint re-checks whole program per pass | PARTIAL (real; mechanism corrected) | low | The loop (`telos-check/src/lib.rs:372-379`) repeats on changed *effects* (`:1136-1148`, `self.changed = true` when inferred `requires` grow), not inferred return types. Measured with `genchain.py`: N functions `f0 -> f1 -> ... -> fN` where only `fN` writes stdout, declared caller first: `telos check` (debug CLI) 0.16 s / 0.51 s / 1.91 s / 4.51 s for N = 100/200/400/800; same chain declared callee first: 0.05 / 0.06 / 0.08 / 0.12 s | propagate effects over the call graph in reverse topological order (SCCs) instead of whole-program passes |
| 10 | driver runs front end and std typing twice | CONFIRMED (code) | low | `telos/src/lib.rs:564-565` (`generate` then `type_program`); `generate` runs `front_end` + `type_std` (`:466-472`), `type_program` runs both again (`:451-454`); `emit_llvm` `:657-658` the same; `type_module` clones all modules (`telos-types/src/lib.rs:701`). Not timed | have `generate` return the typed program, or type once and pass it to both |

## New

| id | finding | severity | evidence | fix direction |
|---|---|---|---|---|
| NEW-1 | LLVM target: the loop region compacts on every iteration once a carried `List`/`Bytes`/`Text` exceeds about 32 KiB, so any loop that grows a list is quadratic. The reviewer's "considered and not reported" note (`region_back_edge`, "bounded by the 2xkept rule") is refuted. Mechanism: compaction deep-copies carried locals with `copy.List<T>`, which sets `cap = len` (`out2-llvm/llvm/program.ll` `copy.List<Int>`, lines 918-925); then `kept = len x size` (`telos-llvm/src/lower.rs:808-810`). The next `push` doubles the buffer (`glue.rs` `@telos.reserve`, 2 x cap), so `above = kept + 2 x kept > 2 x kept` and `telos_rt_arena_due` (`telos-rt/src/arena.rs:386-390`, limit `max(DUE, 2 x kept)`) fires again on that same iteration, deep-copying the list twice (`lower.rs:797-805`). | high (LLVM target) | `List<Text>.push` 32768 times: 4.87 s (Rust 0.003 s); `List<Int>`: 0.61 s; `to_lower` on 256 KiB: 6.97 s (Rust 0.004 s); json parse 17.0 s; split 8.9 s. perf on the `List<Text>` run: 38 % in `copy.List<Text>` plus libc memcpy for most of the rest; on json 64 KiB: 58 % in `copy.std.json.Json`. Affects `std.json` parse (items, members), `std.text` split/lines/to_lower/to_upper, `json.quote`, and any program loop that pushes. | keep capacity in the compaction copy (copy `cap`, not `len`), or count reserved capacity in `kept`, so compactions only happen at doublings (amortized linear) |
