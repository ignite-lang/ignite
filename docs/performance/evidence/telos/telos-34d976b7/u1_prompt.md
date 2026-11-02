You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md`, `docs/slice.md` sections 1 to 5, 8 and 11.7, and the approved slice-8 plan at `/home/timo/.claude/plans/deep-marinating-snowflake.md`. Implement that plan's decisions D1 to D8 and its Surface section.

You are U1, the slice-8 frontend: tests as a first-class primitive, written inline next to the code they test. You own:
- `crates/telos-syntax/**`, `crates/telos-check/**` and `crates/telos-types/**`, with their fixtures and tests;
- `docs/slice.md`: the new section 12, the slice-8 contract, plus sections 2 to 5 and 8 where slice 8 changes them.

U2 starts after your step 1 and does the substrate, the backends and the driver (`telos-io`, `telos-rt`, `telos-rustgen`, `telos-llvm`, `crates/telos`). P may still be finishing a small edit in `std/protocol/telnet.telos`. Don't edit any of those files.

## Step 1: skeleton and contract

Land this first, then message the coordinator with `SendMessage` to `main`: "U1 skeleton landed". After that, U2 builds against it.

- **AST:**
  - `Decl::Test(TestDecl { name: String, name_span, body: Block, span })`;
  - `test_only: bool` on `FnDecl`, `RecordDecl`, `EnumDecl` and `ConstDecl`;
  - `StmtKind::Assert { cond: Expr, message: Option<Expr> }`.
- **Typed API** in `telos-types`:
  - `TypedProgram::tests() -> &[TestInfo]`, where `TestInfo` holds the module, the name, the full name `"<module>/<name>"` and a reference or index to the decl;
  - whatever the backends need to type the test bodies and asserts. For example, per `==`/`!=` assert, the operand type and whether it has `to_text`.

  Add only: the public APIs are the frozen skeletons' shapes.
- **`docs/slice.md` section 12**, the contract U2 reads:
  - the syntax;
  - test-only visibility;
  - assert semantics, with the exact failure message format;
  - `?` in tests;
  - effect roots;
  - E050 to E053;
  - the runner ABI U2 implements. It runs each test in a child process via `--run-one <full name>`. Exit codes per test: 0 pass, 101 assert failure or panic, other codes reported as "exited with code N".
  - the output format of `telos test`, modelled on `cargo test`;
  - the `telos test` exit codes: 0 when every test passed or none exist, 1 when a test failed, 2 for a compile error.
- **Placeholder arms:** wherever the backends match on the new AST or typed nodes, add `unimplemented!("slice 8: …")` arms so the workspace builds. List them in section 12.

## Step 2: frontend

- **Parsing:**
  - `test "name" { … }`;
  - `test fn`, `test type` and `test const`;
  - `assert cond` and `assert cond, "message {x}"`.

  `test` and `assert` become keywords. Grep the std, examples and fixtures for identifiers that collide; the plan found none.
- **Diagnostics**, each with a fixture:
  - E050 duplicate test name in a module;
  - E051 non-test code using a test-only item;
  - E052 `assert` condition not Bool;
  - E053 empty or interpolated test name.
- **`?` in a test body** accepts any `Result` or `Option`.
- **Tests are effect roots** with inferred `requires`, and may use `spawn`.
- **`telos check`** type-checks tests. The driver's `check` path must include them; if that needs a `crates/telos` change, describe it in section 12 for U2.

## Gate

- `cargo test -p telos-syntax -p telos-check -p telos-types` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean.
- `telos check` passes on `std/*.telos`, `std/protocol/*.telos`, `examples/terminal/terminal.telos` and `examples/todo/todo.telos`, plus a new fixture module with tests next to its code.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k`.
- Prefix test runs with `ulimit -c 0;`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

## Final report

At most 10 lines:
- what landed;
- the section-12 points U2 must follow;
- test counts;
- deviations from the plan.
