You work in /home/timo/projects/telos, a Rust compiler for the Telos language. Read `AGENTS.md`, then `docs/slice.md` sections 2 to 5, 8 and 11.7.

You are F2: frontend fixes for verified review findings. You own `crates/telos-syntax/**`, `crates/telos-check/**` and `crates/telos-types/**`, the fixtures and tests of those crates, and `docs/slice.md` sections 2 to 5 and 8. Other agents are working at the same time:
- P on `telos-io`, `telos-rt`, `telos-llvm` and `std/protocol/telnet.telos`;
- T2 on `telos-rustgen`, then `telos-llvm`;
- T3 on `std/**`, `examples/todo/**` and `crates/telos/tests/todo.rs`.

Don't edit their files. If a fix needs a backend change, write the request into your report.

The verified findings are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/`, in `verified-lexer.md`, `verified-parser.md`, `verified-ast.md` and `verified-perf.md` (row #9 only). Each row has its reproduction; the inputs are under `snap-v/verify-*` and `snap-v3/verify-*`. Fix every CONFIRMED and PARTIAL row in your crates, following the decisions below. The coordinator has taken these decisions; follow them and record each one in `docs/slice.md`:

- D1 **Constant patterns:** a bare name in a pattern that resolves to a constant in scope is a constant pattern, whatever its case. Any other bare name is a binding. Fix the contradiction between section 3 (line ~156) and section 4.3.
- D2 **`>=` after type arguments:** in type context, `>=` and `>>` split, so `x: Option<Int>= None` and `List<List<Int>>` parse.
- D3 **Effect roots:**
  - Roots flow through `if` and `match` results, record literals and field access, tuples, list literals, `List.at`, and the result of any call, pure or effectful, including `requires []`. A call's result carries the roots of every argument whose type can contain a substrate handle.
  - Rewrite section 5.1's root list to match. Its opening sentence ("Every operation an effectful function performs needs its effect") is the rule; the list must not contradict it.
  - Fixtures for each verified hole: record literal, `if`, `match`, `ls.at(0)`, an effectful callee returning its parameter, and a `requires []` callee.
- D4 **Shared bare payload variant:** a bare variant that two enums in scope define is E003, unless the expected type decides it (slice 6). Add a fixture.
- D5 **Depth:**
  - The parser limits nesting to 256 levels (brackets, prefix operators, blocks) and reports a new code, E054 "nesting deeper than 256", at the first level over the limit.
  - Long binary and interpolation chains must not overflow the stack in the checker or typer: make those walks iterative over left-deep chains, or run the whole compilation on a thread with a 256 MiB stack in the driver entry point.
  - Acceptance: 10,000 `+` terms and 10,000 `{a}` interpolations check without a crash, on the debug CLI, within 2 s.
  - If the driver thread is your chosen fix, `crates/telos/src/main.rs` is the one file outside your crates you may edit, and only for that.
- D6 **Self-containing types:** a record or enum that contains itself other than through `List` is a new code, E055 "recursive type without List indirection", with a fixture. Check that the existing generic fixtures, such as `Tree<T>`, still pass. If one recurses directly, report it instead of changing it.
- D7 **Lexer messages:**
  - An unclosed `{` in a string reports "unclosed `{` in interpolation".
  - A backslash at the end of a line reports an unterminated string.
  - The unknown-escape message shows one backslash.
  - Byte literals with `}` or `"` work inside interpolation braces.
  - `\'` works in byte strings.
  - `-9223372036854775808` is a valid literal.
  - Lexer errors no longer hide parser errors: the parser runs on the tokens up to the first lexer error.
- D8 **Other fixes:**
  - Diagnostics rendering is linear: build a line-start index once. 20,000 errors render in under 1 s.
  - Postfix operators and `else` do not attach across a newline inside `( )` and `[ ]` (section 3 rule).
  - `(A,)` as a type is E001.
  - A nested `fn` in a block gives one error.
  - `Never` operands of arithmetic and comparison give no E040.
  - Exhaustiveness checking is polynomial for Bool and literal columns; 20 Bool columns with 40 arms check in under 1 s.
  - The effect fixed point does not depend on declaration order (process in reverse topological order of the call graph); 800 chained fns check in under 0.5 s in either order.

Each fix gets a fixture or test that failed before it. Each new code (E054, E055) gets a row in section 8.

Gate: `cargo test -p telos-syntax -p telos-check -p telos-types -p telos` is green. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` are clean. If they fail only in other agents' files, report that and don't fix it. `telos check` passes on `examples/terminal/terminal.telos`, `std/text.telos` and `std/json.telos`.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k` for direct cargo calls.
- Prefix test runs with `ulimit -c 0;`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

Final report, at most 12 lines:
- fixed rows, as counts per file;
- any row not fixed, with the reason;
- backend requests;
- test counts.
