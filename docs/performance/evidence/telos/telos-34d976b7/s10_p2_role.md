You are P2. You own `crates/telos-rustgen/**` and `crates/telos-io/**`, with target dir target-j.

Your items:
- **14.1 quantities:** lowered as `i64`, with checked arithmetic and debug text.
- **14.2 named arguments:** reordered, keeping source evaluation order.
- **14.3 `Map<K, V>`:** a `BTreeMap`, with value semantics, equality, debug text and key-order `for`.
- **14.4 / 14.5 executable components:**
  - generated `main`s per `TypedProgram::executables()`: stores, then listener binding with `--port` and `listening on`, then the component value with embedded components, then `K.main(value)`, then exit-code mapping;
  - remove the `Service` and `Client` lowering only once P4 has moved the examples. Until then keep both paths. Agree the cutover with P4 through `main`.
- **`IoError.with_exit_code(n)` builtin (O12).**
- **14.8 T1, Rust half:**
  - `telos_io::fail(op, file, line, col, detail)` and the report format;
  - `checked_*` arithmetic in place of rustc's overflow panic, so the report carries the Telos position;
  - `catch_unwind` in `telos_io::spawn`, so a failing task logs `task ended with error: …` and the process continues;
  - the main task and tests exit 101.

  The C ABI of these substrate functions is P3's to adapt in `telos-rt`. Tell P3 the Rust signatures as soon as they exist.
- **14.9 R2 to R5 in `telos-io`,** with the audit `io/iotest` `drop` and `cross` behaving as the audit README's "expected after the fix" column says. Build iotest with its own `--target-dir` under the scratch directory.
