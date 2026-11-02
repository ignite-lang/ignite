You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read these first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/slice.md` sections 3, 4, 6.1, 8, 12.5, 13 and 14. Section 14 is your contract; it was reviewed and accepted by the steering session;
- `docs/vision.md` section 7 (T1, T4, T5, T6);
- `docs/Telos_Design.md` section 11.3;
- `examples/terminal/terminal.design.telos`.

You are P1, the slice-10 frontend. You own `crates/telos-syntax/**`, `crates/telos-check/**` and `crates/telos-types/**` with their fixtures and tests, and `docs/slice.md` (section 14 and the section 8 diagnostics table). The other agents:

| agent | owns |
|---|---|
| P2 | `crates/telos-rustgen/**` and `crates/telos-io/**` |
| P3 | `crates/telos-llvm/**`, `crates/telos-rt/**` and `docs/llvm.md` |
| P4 | std and examples, later |

Do not edit their files. The one exception is the placeholder arms of step 1.

## Step 1: skeleton first

- AST and typed-API additions for section 14, add-only (14.10):
  - quantity literals;
  - named arguments;
  - brace literals typed by the expected type;
  - `Map<K, V>`;
  - `Ref<K>` and bare `Ref`;
  - `Signal<T>` outputs;
  - `module("…")` records;
  - qualified enum variants `Type.Variant`;
  - `contract`, `policy` and `assembly` declarations;
  - `TypedProgram::executables()`;
  - `TypedProgram::failure_site()` for every expression that can fail (T1 report positions).
- `unimplemented!("slice 10: …")` placeholder arms in the backends, so the workspace builds with `--all-targets`. List them in a new subsection 14.14 of `docs/slice.md`.
- If the skeleton's API differs from 14.10, update section 14 to match. Report each difference.
- When the skeleton builds with `--all-targets`, send `main` a SendMessage: "P1 skeleton landed", with the placeholder count and a one-line summary of each API difference from 14.10. P2 and P3 start then.

## Step 2: frontend for all of section 14

- 14.1 quantities. 14.2 named arguments.
- 14.3 brace literals and Map typing.
- 14.4 resolution of system names and references, outputs, `module("p")` loading of sibling modules of the program's package, and qualified enum variants.
- 14.5 detection of executable kinds (E109).
- 14.7 the declarations. New keywords: `contract`, `policy`, `assembly`. Check `std/` and `examples/` for identifiers with these names, and report any you find.
- 14.8 `failure_site` for `assert`, `?`, `+ - * / %`, index, match, unwrap and slice.
- Diagnostics E100 to E109. Each gets a fixture; E104 gets three, with the three message texts. Update the section 8 table.
- `telos check examples/terminal/terminal.design.telos` must reach exit 0, apart from what P4 adds to std. Std types are P4's. To check your frontend before P4 runs, use a test fixture that declares the std kinds locally. The design file edits allowed are listed in 14.13 O5; tell `main` if you need any other edit.
- Coordinate with P2 and P3 by SendMessage; the coordinator gives you their ids. Answer their API requests quickly.

## Gate

- `cargo test -p telos-syntax -p telos-check -p telos-types` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your crates.
- `telos check` passes on every std file and both examples. Today's examples still use `Service`/`Client`; keep them checking until P4 moves them. Removing `Service` and `Client` belongs to P4's gate.
- Invariant 9: measure the frontend time of `telos check examples/todo/todo.telos` and of the 100,000-term chain program before and after your work, and report both.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- No git command that changes state: no add, commit, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.
- No work files in /tmp; use the scratch directory.
- Kill your own processes by explicit PID before you finish.

## Final report

At most 12 lines:
- section 14 items done;
- API differences from 14.10;
- test counts;
- the frontend times before and after;
- deviations from section 14.
