You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md`, `docs/friction.md`, `docs/slice.md` sections 1 to 5, 8, 11.7 and 12. Then read the coordinator's slice-9 decisions at `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/slice9_decisions.md`.

You are N1, the slice-9 frontend. You own `crates/telos-syntax/**`, `crates/telos-check/**` and `crates/telos-types/**`, their fixtures and tests, and `docs/slice.md`. You also write the new section 13, the slice-9 contract. Three other agents work alongside you:
- N2 does the Rust backend: `crates/telos-rustgen`.
- N3 does the LLVM backend and runtime: `crates/telos-llvm`, `crates/telos-rt`, `crates/telos-io`.
- N4 later rewrites std and the examples.

Do not edit their files. The one exception: placeholder arms in the backends, as in step 1.

## Step 1: skeleton and contract, first

- **AST and typed-API additions** for every row:
  - closures, with captures computed by the typer and exposed per closure: the captured names, their types and their effects;
  - range patterns;
  - tuple field access;
  - raw strings (lexer only);
  - associated fns;
  - glob imports;
  - Never-typed `break`/`continue`;
  - structural equality: which types are comparable, and an API that tells a backend which equality glue a type needs;
  - debug text;
  - recursive enum payloads: per enum payload, whether it needs boxing.

  Add only; the public APIs keep their frozen shapes.
- **`docs/slice.md` section 13** holds the contract for N2 and N3:
  - every row's syntax, typing and lowering obligations;
  - the boxing rule;
  - the closure representation each backend must implement;
  - the exact debug-text format;
  - the equality semantics;
  - a list of the `unimplemented!("slice 9: …")` placeholder arms you add so the workspace builds.
- When the skeleton builds with `--all-targets`, send SendMessage to `main`: "N1 skeleton landed". N2 and N3 start then.

## Step 2: frontend for every row in the decisions file

- Each new rule gets a fixture.
- Each changed or retired diagnostic gets its section 8 row updated.
- Restore the `Tree<T>` fixture with direct recursion through an enum (row 15). E055 stays for record-only self-containment.
- Row 14: the checker and typer handle a 100,000-term `+` chain without a stack overflow.
- Coordinate with N2 and N3 by SendMessage: agent ids will be given to you. Answer their API requests quickly.

## Gate

- `cargo test -p telos-syntax -p telos-check -p telos-types` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean.
- `telos check` passes on every std file and both examples.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k`.
- Prefix test runs with `ulimit -c 0;`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

## Final report

At most 12 lines:
- rows done;
- contract points N2 and N3 must follow;
- test counts;
- deviations from the decisions file.
