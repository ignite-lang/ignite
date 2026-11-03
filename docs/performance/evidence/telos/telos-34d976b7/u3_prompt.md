You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md` and `docs/slice.md` section 12. Section 12 covers slice 8: inline `test "name" { … }` blocks, `test fn/type/const`, `assert cond[, msg]`, `?` in tests, and the `telos test` command.

You are U3, and the only agent working in the tree. Slice 8 is implemented on both targets, but std and the examples hold one inline test only, in `std/text.telos`. Your job is to write the Telos behaviour tests inline, next to the code they test, and to close out the slice.

1. **Write inline `test` blocks.** They should cover the behaviour that matters. Prefer pure functions; use `test fn` helpers for fixtures. Use real IO only where a test needs it: a `TcpListener` on port 0 with a spawned task works.
   - `std/text.telos`: split, split_once, find, replace, trim_*, to_lower/upper, lines, repeat, substring and digit_value, including edge cases: empty input, separator at the start or end, multi-byte UTF-8.
   - `std/json.telos`: encode with escaping, parse of every value kind, `\uXXXX` with surrogate pairs, a lone surrogate as an error, nesting limit 64, numbers up to 18 digits, fraction or exponent as an error, the accessors, and round trip.
   - `std/console.telos`.
   - `std/protocol/telnet.telos`: the pure decoder and encoder, including IAC IAC as a literal 255, DO/DONT/WILL/WONT answers, subnegotiation framing and its 1024-byte limit, CR LF / CR NUL normalization, and command bytes split across reads.
   - `std/protocol/http.telos`: request-line and header parsing, Content-Length, the 400/413/431 limits, the header-injection and Transfer-Encoding rules, and keep-alive decisions.
   - `examples/terminal/terminal.telos`: the command handlers.
   - `examples/todo/todo.telos`: `encode` and routing on fixed `Request` values.

   Where possible, port the cases `crates/telos/tests/fixtures/std_probe.telos` checks. Then delete that probe and the test that runs it in `crates/telos/tests/todo.rs`, once inline tests cover the same cases.
2. **When a test finds a bug in std or an example, fix it.** List each bug in your report.
3. **Fix the compiler when it gets in the way.** If writing tests hits a compiler limitation or bug, fix it in the right crate. You own the whole tree now. Add a regression test and list it.
4. **Update `docs/slice.md` section 12** for U2's deviations:
   - one test executable per program file, with the std modules sharing one;
   - LLVM tests need the runtime archive with `--features runner`, which `task test-telos` builds;
   - Rust-target compound equality goes through debug text;
   - `try_failed`;
   - panic messages that differ by target;
   - `*.design.telos` skipped.
5. **Acceptance.**
   - `task gate` is green. It includes `task test-telos` on both targets.
   - Report the number of inline tests per file, and the `telos test` summary line per target.
   - `examples/todo/todo.telos` stays at or under 100 lines, not counting test blocks. If tests push it over, put the todo tests in a `test`-only section at the end of the file and report the line count with and without the tests.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k` for direct cargo calls. `task` targets use the repository's `target/`.
- Prefix test runs with `ulimit -c 0;`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.

Final report, at most 15 lines:
- inline test counts per file;
- `telos test` summaries for both targets;
- bugs found in std and the examples, and how you fixed them;
- compiler fixes;
- `task gate` counts;
- anything that read badly when writing tests: a short list of syntax or feature friction.
