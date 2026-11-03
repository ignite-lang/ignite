You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a compile-time evaluator (`crates/ignite-eval`), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/slice.md` sections 6.1, 8, 14.4 to 14.8, 15 (with 15.14, the implementation record) and 16. Section 16 is the slice-12 contract, reviewed and accepted by the steering session with amendments A1 to A15 applied;
- `docs/vision.md` sections 4.1 (row 12) and 7 (T1, T4); read only, never edit it;
- `docs/design.md` sections 4.1, 7.1, 11.7 and 11.8;
- `std/contract.ignite`, `std/policy.ignite`, `std/probe.ignite`, `std/deploy.ignite`, `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P1, the slice-12 frontend and skeleton. You own `crates/ignite-syntax/**`, `crates/ignite-check/**` and `crates/ignite-types/**` with their fixtures and tests; `crates/ignite-eval/src/resolve.rs` and the new `crates/ignite-eval/src/validate.rs`; and `docs/slice.md` for the whole slice (section 16, the section 8 diagnostics paragraph, the 16.21 record). For step 1 only you also create the skeleton files listed below; after step 1 they belong to their owners in 16.18.

| agent | owns |
|---|---|
| P2 | `crates/ignite-io/**`, `crates/ignite-rt/**`, the lowering of the 16.13 builtins in `ignite-rustgen` and `ignite-llvm`, `crates/ignite/src/run/{process,probe,clock,journal,control}.rs` |
| P3 | `crates/ignite-eval/src/{model,value,eval,explain}.rs`, `crates/ignite/src/run/{mod,engine,replay}.rs`, `crates/ignite/src/lib.rs`, `main.rs`, `crates/ignite/tests/**` |
| P4 | `std/**` (after your `std/control.ignite` types), `examples/**`, `AGENTS.md`, `docs/friction.md`, `docs/design.md` notes, acceptance |

## Step 1: skeleton first

- `std/control.ignite`: the type declarations of 16.6 only (`Machine`, `Step`, `Event`, `Effect`, `Standing`, `Fired`, `Segment`, and the payload types `Supervisor`, `ProbeScheduler`, `Evaluator`, `Rollout` as records with the fields 16.7 to 16.12 need), plus `fn step(machine: Machine, event: Event) -> Step` with a body that returns the machine unchanged and no effects. P4 writes the real bodies. After step 1 the file changes only by message between P3 and P4.
- `ignite-types`: the signatures of `Clock.monotonic() -> Duration` and `Endpoint.of(ref: Ref<K>) -> IoResult<Endpoint>` (16.13), and the hook signature checks for `K.machine(self, id: Text) -> std.control.Machine` and `K.validate(self) -> List<Text>`.
- The placeholder arms of 16.20, each `unimplemented!("slice 12: …")`: `ignite-io/src/clock.rs` `monotonic`, `ignite-io/src/net.rs` `endpoint_of`, `ignite-io/src/lib.rs` `drain`; `ignite/src/run/{process,probe,clock,journal,engine,replay}.rs`; `ignite/src/lib.rs` `run`, `replay`; `ignite-eval/src/value.rs` `Value::decode`; `ignite-eval/src/model.rs` `machines`, `observations`; `ignite-eval/src/validate.rs` `validate`. The builtins lower to calls of these stubs in both backends only as far as `--all-targets` needs to build; P2 writes the lowering.
- `crates/ignite`: the `run` and `replay` subcommands with the arguments of 16.3 (`RunOptions`, `ReplayOptions` of 16.16), calling the placeholders.
- `nix` moves from dev-dependency to dependency of `crates/ignite` (O43); say so in your report.
- If the skeleton's API differs from 16.16 or 16.20, update section 16 to match and report each difference.
- When `cargo build --workspace --all-targets` succeeds and the existing tests are still green, send SendMessage to `main`: "P1 skeleton landed", with the placeholder count and one line per API difference. P2, P3 and P4 start then.

## Step 2: the check-time diagnostics of 16.17

- E120 to E124 and E129 in `validate.rs` and `resolve.rs`, evaluated with the existing evaluator; `validate` and `machine` are hooks found by name on the kind, as `admits` is (O18). The compiler knows no outcome or zone names (invariant 6): E120 prints the text `K.validate` returns.
- `ignite check` and `ignite plan` report them for every assembly (O20).
- A14: phase "check" means `ignite check examples/terminal/terminal.design.ignite` stays exit 0 with no output. Run `cargo test -p ignite --test main cli::check_design_file_is_silent` first after the validators land, before anything else. Until P4's `validate` hooks exist, the design file must stay silent too.
- Fixtures `e120_validate_refused` to `e124_window_too_short` and `e129_missing_entry` under `crates/ignite/tests/fixtures/` (the directory is P3's: create the fixture files, ask P3 by SendMessage for the test lines that run them).
- Answer P2's, P3's and P4's API requests by SendMessage quickly (the coordinator gives you their ids). Record each API change in 16.21, as 15.14 did.

## Gate

- `cargo test -p ignite-syntax -p ignite-check -p ignite-types -p ignite-eval` is green.
- `cargo build --workspace --all-targets` succeeds.
- clippy `-D warnings` and fmt are clean on your crates and files.
- `ignite check` passes on every std file, both examples and the design file, the design file with no output.
- Invariant 9: `ignite check` and `ignite plan` of the design file before (on `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before`, do not modify it) and after your work, interleaved as in 15.1.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1`.
- Everything else is in the rules file.

## Final report to the coordinator, at most 12 lines

- step 1: placeholder count, API differences from 16.16 and 16.20;
- the codes done, with fixture paths;
- test counts;
- check and plan times before and after;
- deviations from section 16.
