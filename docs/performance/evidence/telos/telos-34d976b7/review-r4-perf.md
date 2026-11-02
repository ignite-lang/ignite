# R4 review: efficiency and performance

Scope: compiler crates (telos-check, telos-types, telos-rustgen, telos-llvm, telos driver), generated Rust and LLVM output, telos-io, telos-rt, std/json.telos, std/text.telos. Read-only. No cargo build or test was run, so every cost below is a computed estimate from code, not a measurement.

Severity counts: high 1, medium 3, low 6.

## Findings, most severe first

### 1. std/json.telos parse copies the whole input per token (Rust target)

- file: `std/json.telos:310`, `:312`, `:318`; dispatch `:132`; generator `crates/telos-rustgen/src/lib.rs:985-986`
- quote: `let (item, end) = parse_value(bytes, i, depth)?` / `i = skip_space(bytes, end)`
- scenario: `bytes: Bytes` is a parameter, so each call passes `bytes` in a value position, which the generator emits as `bytes.clone()` (`lib.rs:985-986`, args at `:980-981`). Per array element there are about four full-buffer clones: `parse_value` at `:310`, the dispatch call to `parse_number` or `parse_word` in `parse_value`, `skip_space` at `:312`, and `skip_space` at `:318` after a comma. A 1 MiB array of about 100k numbers makes about 4×10^5 clones of 1 MiB, that is about 4×10^11 bytes copied (tens of seconds of memcpy, plus allocation). A 64 KiB array makes about 2.6×10^4 clones of 64 KiB, about 1.7×10^9 bytes. The cost is O(input × tokens), quadratic. The same path serves any object, so a body parsed on the Rust target is affected. The todo example's HTTP parse (`examples/todo/todo.telos:31`) cannot compile yet (`std.protocol.http` is not in the repo), but any Rust-target program that parses text from a socket or stdin hits this today.
- severity: high
- confidence: confirmed (path traced through the generator; cost computed, not measured). The LLVM target borrows read-only parameters (`crates/telos-llvm/src/lower.rs:519-531`, excluded as in-progress work), so the cost there depends on that change.

### 2. std/text.telos find, split and lines clone the haystack and needle at every position (Rust target)

- file: `std/text.telos:38`, `:53`, `:56`, `:101`; callers `find_from` `:66`, `split` `:91-112`, `lines` `:171-183`, `replace` `:122-123`
- quote: `fn bytes_find_from(bytes: Bytes, needle: Bytes, start: Int) -> Option<Int> {` / `if bytes_match_at(bytes, needle, at) {`
- scenario: `bytes_find_from` loops over every candidate offset and calls `bytes_match_at(bytes, needle, at)` at each one. The call clones both `bytes` and `needle` (`lib.rs:985-986`, args `:980-981`). A `find`, `contains`, or `split` on a 1 MiB text scans about n offsets and copies n bytes each time, about 1.1×10^12 bytes (minutes). On a 64 KiB text it is about 4×10^9 bytes (under a second). `split` does not escape it: each piece calls `bytes_find_from` from its own start, but the offsets together still total about n, so the cost stays about n². Fix: a helper that takes a reference, or a `find` primitive in telos-rt or telos-io.
- severity: medium
- confidence: confirmed (generator trace; cost computed, not measured). On the LLVM target the same helper may borrow `bytes` if `reads_only` holds; I did not trace that for this helper, so there the cost is suspected to be O(n·m).

### 3. Rust generator copies every place used as a value, including read-only arguments

- file: `crates/telos-rustgen/src/lib.rs:865-871` (`is_place`), `:985-986` (clone), `:980-981` (args), `:1274-1277` (UFCS receiver), `:1330` (match scrutinee)
- quote: `if clone && self.is_place(expr) {` / `return format!("{}.clone()", self.bare(expr));`
- scenario: any call that passes a `Bytes`, `Text` or `List` local or field to a function copies the whole value, even when the callee only reads it. A 1 MiB `Text` passed to a per-line helper 10^4 times costs about 10 GB of copies. This is the root cause of findings 1 and 2. Rust target only; the LLVM borrowed-parameter path is excluded as in-progress work.
- severity: medium
- confidence: confirmed (generator code; golden evidence in `crates/telos-rustgen/tests/golden/` shows `first_word(line.clone())` and `match input.clone()`).

### 4. Rust target emits a move out of an index for `List.at` on non-Copy elements (correctness, outside the perf brief)

- file: `crates/telos-rustgen/src/lib.rs:1318` (`at`), `:985-986` (no clone for `Method` or `Index`, which are not places)
- quote: `("at", 1) => format!("{r}[({}) as usize]", a[0]),`
- scenario: with `lines: List<Text>`, `let line = lines.at(0)` emits `let line = lines[(0) as usize];`, which moves a `String` out of a `Vec` and fails with E0507 (cannot move out of index). A valid program is rejected by the Rust target. `std/text.telos:179` uses `.at(...)` only as a receiver, so it is not affected.
- severity: medium
- confidence: confirmed by trace; the generated Rust was not compiled (cargo is not allowed in this review), so the E0507 is from rustc's rules, not from a build.

### 5. Text repeat is quadratic in the result length

- file: `std/text.telos:186-193`; runtime concat `crates/telos-rt/src/text.rs:45`
- quote: `out = out + text` (`std/text.telos:190`) / `put(out, TelosList::from_bytes(&[bytes(a), bytes(b)].concat()));`
- scenario: each `+` copies the accumulated `out`, and `telos_rt_text_concat` makes two copies per `+` (LLVM). `repeat("x", 100000)` copies about 5×10^9 bytes over 10^5 concatenations. A 1 MiB result from a one-byte text is about 5×10^11 bytes. The Rust target also clones `out` per `+` (`lib.rs:985-986`). Fix: a single-allocation repeat primitive, or build the result with `List` and join once.
- severity: low
- confidence: confirmed (code); cost computed, not measured.

### 6. json member and encode clone each subtree per call or per level

- file: `std/json.telos:364-375` (`member`), `:65-85` (`encode`)
- quote: `fn member(value: Json, name: Text) -> Option<Json> = match value {` / `for pair in members {`
- scenario: on the Rust target `member(value, ...)` clones the whole value (and `for pair in members` clones the member list) at every accessor call, so a loop of K accessor calls on a large object costs K times its size. `encode` copies the subtree text at each nesting level, which is O(output × depth), with depth capped by `MAXIMUM_DEPTH`. Low because each call is small and the depth is bounded.
- severity: low
- confidence: confirmed (code); cost computed, not measured.

### 7. telos-rt tcp read takes a dedicated block of `maximum` bytes per read, not shrinkable

- file: `crates/telos-rt/src/net.rs:233-244`; `crates/telos-rt/src/arena.rs:184` (large path), `:329-335` (`trim`)
- quote: `let block = arena::alloc(size, 1);` / `if used < size && !state.cursor.is_null() && pointer.wrapping_add(size) == state.cursor {`
- scenario: `size` is `maximum` clamped to 1 MiB. A request above `LARGE` (16 KiB) gets its own zeroed system block, and `trim` only shrinks a block that is the last one in the current chunk, so a dedicated block keeps its full `maximum` until the region or task releases it. A program that calls `stream.read(1048576)` pays one 1 MiB allocation and free per read, and the memory stays until the release. The telos-io `read_buffer` (`crates/telos-io/src/lib.rs:87-92`) allocates `maximum` per read in the same way. The std callers pass 4096 (`std/protocol/telnet.telos:42`, `:177`, `:275`, `:348`), which is under `LARGE`, so the std paths are not affected.
- severity: low
- confidence: confirmed for the allocation and trim behaviour; the accumulation scenario depends on where the read sits relative to a region, which I did not trace.

### 8. telos-check const_cycles runs a BFS from every constant

- file: `crates/telos-check/src/lib.rs:1049-1052`
- quote: `for (name, (span, _)) in graph {` / `let mut queue = std::collections::VecDeque::from([name.as_str()]);`
- scenario: the BFS restarts from every constant in the graph, so the cost is O(C × (C + E)). With 1000 constants and 5000 references that is about 6×10^6 steps, milliseconds. Low because the constant graph is small in practice.
- severity: low
- confidence: confirmed (code); cost computed, not measured.

### 9. telos-check fixpoint re-checks the whole program on every pass

- file: `crates/telos-check/src/lib.rs:372-379`
- quote: `checker.check_program(program);` (inside `loop {`, repeated while `checker.changed`)
- scenario: each pass walks the whole program. The number of passes grows with the length of a chain of inferred return types declared in reverse dependency order, so 500 such functions in a 50k-node program cost about 2.5×10^7 node visits. Low; the pass count was not measured.
- severity: low
- confidence: suspected (pass count not measured).

### 10. Driver runs the front end and std typing twice; type_module deep-copies all module exports

- file: `crates/telos/src/lib.rs:564-565` (build), `:657-658` (emit_llvm), `:169-174` (`symbols()`); `crates/telos-types/src/lib.rs:701`
- quote: `let emitted = generate(file, &library, substrate)?;` / `let mut all = modules.clone();`
- scenario: `build` calls `generate`, which already runs the front end and std typing, then calls `type_program` again. `emit_llvm` discards the Rust output of `generate` before `type_program`. `type_module` clones every module's exports for each typed module, so it costs M × (all exports). `symbols()` clones every module's symbols per call. Compile-time only, in the low range.
- severity: low
- confidence: confirmed (code); cost not measured.

## Considered and not reported

- Exhaustiveness (`crates/telos-types/src/exhaustive.rs:121-200`): the split recurses into each case, so cost can grow exponentially with nested patterns mixing constructors and catch-alls. Realistic matches are small; contrived only.
- `PENDING` scan (`crates/telos-io/src/net.rs:259-264`): linear in the number of streams with pending data on one thread, normally one.
- Stdout extra copy (`crates/telos-rt/src/stdio.rs:37-40`): linear, one copy per write.
- LLVM `region_back_edge` (`crates/telos-llvm/src/lower.rs:778-815`): carried copies run only when `arena_due`, bounded by the 2×kept rule.
- Excluded by the brief: one write per reply, write coalescing, TCP_NODELAY, the global HANDLES mutex, thread reuse, inlining `arena_due`, borrowed parameters in LLVM lowering.
