You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/slice.md` sections 4, 5, 8, 13.11, 14 (especially 14.4 to 14.7, 14.10 and 14.14) and 15. Section 15 is the slice-11 contract, reviewed and accepted by the steering session;
- `docs/vision.md` sections 4.1 (row 11), 7 (T4) and 10.2 (the a21 hole); read only, never edit it;
- `docs/design.md` sections 3.5, 6.1 to 6.3 and 11.3;
- `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P1, the slice-11 frontend. You own `crates/ignite-syntax/**`, `crates/ignite-check/**` and `crates/ignite-types/**` with their fixtures and tests, and `docs/slice.md` for the whole slice (section 15 and the section 8 diagnostics paragraph). For step 1 only you also create `crates/ignite-eval` and the `plan` and `explain` subcommand stubs in `crates/ignite/src/main.rs` and `lib.rs`; after step 1 those files belong to P2 and P3 (15.11).

| agent | owns |
|---|---|
| P0 | measures slice 10's carry-overs on a copy of commit `60aef38`; edits nothing |
| P2 | `crates/ignite-eval/src/eval.rs`, `value.rs` |
| P3 | `crates/ignite-eval/src/model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite/src/**`; `crates/ignite/tests/**` |
| P4 | `std/deploy.ignite`, examples, goldens of the examples, `AGENTS.md`, `docs/friction.md`, `docs/design.md` notes |

## Step 1: skeleton first

- Typed API of 15.9, add-only: `TypedProgram::systems()` with `MemberInfo` and `PropertySource`, `TypedProgram::assemblies()`, `TypedProgram::effects(expr)`, and access for `ignite-eval` to every module's typed program and AST through `TypedModules`.
- New crate `crates/ignite-eval` (workspace member, `autotests = false` with one `tests/main.rs` per `AGENTS.md`) with the API of 15.9 and the files `eval.rs`, `value.rs`, `model.rs`, `resolve.rs`, `explain.rs`. Each fn of the 15.13 table is `unimplemented!("slice 11: …")`.
- `crates/ignite`: `plan` and `explain` subcommands with the arguments of 15.7 and 15.8, and `plan()`/`explain()` in `lib.rs` as placeholders.
- If the skeleton's API differs from 15.9 or 15.13, update section 15 to match and report each difference.
- When `cargo build --workspace --all-targets` succeeds, send the coordinator a SendMessage: "P1 skeleton landed", with the placeholder count and one line per API difference. P2, P3 and P4 start then.

## Step 2: the frontend of section 15

- 15.2: E116 for an effectful property expression, at the expression, from the type checker. A property expression includes the default expressions of the kind fields a declaration leaves out. A bare member name and a resource name stay legal.
- The audit probe `docs/audit/2026-11-01/effects/a21_system_property_effect.ignite` now gives E116: update its `.expected` file and the row in `docs/audit/2026-11-01/README.md`.
- E116 fixture `e116_effectful_property` under `crates/ignite-types/tests/fixtures/`.
- Keep `ignite check` green on every std file, both examples and the design file.
- Answer P2's and P3's API requests by SendMessage quickly (the coordinator gives you their ids). Record each API change in section 15 (a new 15.14 "Implementation record", as 14.14 did).

## Gate

- `cargo test -p ignite-syntax -p ignite-check -p ignite-types` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your crates.
- `ignite check` passes on every std file, both examples and `examples/terminal/terminal.design.ignite`.
- Invariant 9: the frontend time of `ignite check examples/todo/todo.ignite` and of `ignite check examples/terminal/terminal.design.ignite` before and after your work.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-p1`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- No git command that changes state: no add, commit, mv, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; no heredocs or `python3 -c`; never delete a `target/` directory; leave the `terminald` on port 2323 alone; kill your own processes by explicit PID before you finish.

## Final report to the coordinator, at most 12 lines

- section 15 items done;
- API differences from 15.9 and 15.13;
- test counts;
- the frontend times before and after;
- deviations from section 15.
