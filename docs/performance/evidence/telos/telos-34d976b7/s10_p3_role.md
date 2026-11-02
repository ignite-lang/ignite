You are P3. You own `crates/telos-llvm/**`, `crates/telos-rt/**` and `docs/llvm.md`, with target dir target-h.

Your items:
- **14.1 quantities:** lowered as `i64`, with the T6 overflow intrinsics and debug text identical to the Rust target.
- **14.2 named arguments:** reordered, keeping source evaluation order.
- **14.3 `Map<K, V>`:**
  - a sorted array of pairs with binary search, plus copy and drop glue and arena or region rules consistent with `docs/llvm.md` §4;
  - equality, debug text and key-order `for`, identical to the Rust target;
  - a B-tree is a labelled later milestone, not this slice.
- **14.5 executable components:** generated `main`s per `TypedProgram::executables()`, matching P2's order and exit codes. Keep the `Service`/`Client` path until the P4 cutover.
- **`IoError.with_exit_code(n)` builtin,** through the `telos-rt` adapter over P2's `telos-io` function.
- **14.8 T1, LLVM half:**
  - `setjmp` never sits in a Rust frame, because `returns_twice` is undefined in Rust. It lives in the emitted IR's task-entry wrapper (a `setjmp` call with the `returns_twice` attribute, then the task body) or in a small C file built by `telos-rt`.
  - `telos_rt_fail` prints the report with P2's format, releases the task's arena regions and handles, then `longjmp`s to that wrapper, which calls `task_end`.
  - A Rust panic inside `telos-io` or `telos-rt` is caught by `catch_unwind` at the task thread and reported the same way; it is never `longjmp`ed over.
  - The main task and tests exit 101.
  - No landing pads in the emitted IR.
  - Show with a test that a task failing 10,000 times in a loop of spawned tasks leaks no arena memory and no handles.
  - Replace the 9c overflow report (rustc panic text, no position) with the 14.8 format.
- **`docs/llvm.md`:** §4 (failure containment) and the Map representation.
