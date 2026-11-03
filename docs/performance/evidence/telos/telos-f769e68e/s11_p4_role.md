You work in /home/timo/projects/telos, the Ignite compiler: a Rust compiler with two targets (Rust and LLVM), a standard library in Ignite (`std/`) and a substrate in Rust. Read first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/friction.md`;
- `docs/slice.md` sections 14.6, 14.7, 14.14 and 15 (your contract, reviewed and accepted by the steering session);
- `docs/vision.md` sections 4.1 (row 11) and 10.2; read only: you do not edit `docs/vision.md`;
- `docs/design.md` sections 6.2, 10.1 and 11.3;
- `std/deploy.ignite`, `std/probe.ignite`, `examples/terminal/terminal.design.ignite`;
- the rules file `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s11_rules.md`; follow it.

You are P4: std, examples, records and acceptance of slice 11. You start when the coordinator tells you P1's skeleton has landed.

| agent | owns |
|---|---|
| P1 | `ignite-syntax`, `ignite-check`, `ignite-types`, `docs/slice.md` |
| P2 | `crates/ignite-eval/src/eval.rs`, `value.rs` and its test files |
| P3 | `crates/ignite-eval/src/model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite/src/**`; `crates/ignite/tests/**` (P2's files excepted) |
| P4 (you) | `std/**`, `examples/**`, `AGENTS.md`, `docs/friction.md`, the notes in `docs/design.md` named below |

Golden files under `crates/ignite/tests/golden/` are in P3's directory: you write the example goldens there by asking P3 for the test that blesses them, or P3 adds them on your request. Use SendMessage with the ids the coordinator gives you.

## 1. `std/deploy.ignite` first (15.6)

- `type Member { declaration: Text, kind: Text, executable: Bool, outputs: List<Text> }`.
- `fn admits(binding: Binding, member: Member) -> Option<Text>`: `LocalSocket` admits a `TcpListener` resource; `NativeProcess` an executable component; `WorkstationExecutable` any component; `ScheduledProbeHost` a component with at least one output. A refusal returns the reason text that E112 prints.
- `fn includes(binding: Binding) -> List<Ref>` exists; keep its signature, which 15.6 requires.
- Inline tests for every variant, admitted and refused.
- Tell P3 by SendMessage when this has landed; P3's resolution depends on it.

## 2. Examples and goldens

- Model goldens for `examples/terminal/terminal.ignite` and `examples/todo/todo.ignite` (no assembly, so `"assembly": null`), next to P3's design-file golden.
- The examples' behaviour stays unchanged: terminal, todo and inline test counts as before slice 11, or each difference explained.

## 3. Docs

- Add a note to `docs/design.md` section 6.2 with this content:
  - the executable specification is the system model of `docs/slice.md` section 15 (D7's rename, X3);
  - its `bindings` is the model's `assembly.resolution`;
  - `machines`, `observations`, `artifacts`, `stateSchema` and `admissionChecks` arrive in slices 12 and 13;
  - until wave H the public schema is the 15.4 table plus the golden `crates/ignite/tests/golden/terminal.design.model.json`; there is no JSON Schema file in this slice.
- A note to `docs/design.md` section 10.1 that `plan` and `explain` exist as `docs/slice.md` 15.7 and 15.8 define them.
- `AGENTS.md`: a layout row for `crates/ignite-eval`; the `ignite plan` and `ignite explain` rows in the commands table if they help a reader; a slice-11 row in "Gate time": `task gate` cold and warm, `cargo test --workspace --no-run`, the `todod` build time on both targets, and `ignite check` / `ignite plan` of the design file. Use the same commands as the existing rows. Measure before and after interleaved on the same machine state (15.1, steering decision 2026-11-02 02:02): for each timed unit (warm gate, each file of `crates/ignite/tests/`, `todod` per compile step on both targets, `ignite check` of the design file), one run on a `git archive 60aef38` copy of the tree with its own target dir, then one run on the finished slice-11 tree, alternating, at least 3 pairs. Record the 1-minute load for every run, discard a pair whose two loads differ by more than 5, and report the median of the pair ratios next to the absolute times. Write the method into the row's notes. `ignite plan` has no "before"; report its absolute time with load. P0's slice-10 attribution, which the coordinator passes to you, goes in the notes too.
- `docs/friction.md`: mark the rows slice 11 solves; add the friction you hit.

## 4. Acceptance: 15.0 items 4 to 11

Verify each yourself and report it as pass or fail with the command you ran:
- 4: two `ignite plan` runs from two working directories, byte-identical and equal to the golden;
- 5: no expression text in the model; the encoding walk passes;
- 6: each of the five members once in `assembly.resolution`; `ignite check` of the design file exits 0;
- 7: the three explain goldens;
- 8: the equivalence test on both targets;
- 9: every Rust and LLVM golden byte-identical, except LLVM goldens P3 re-blessed for reachable-only std emission, each with its diff explained in P3's report;
- 10: invariant 9 numbers, with every rise over 10 % explained;
- 11: a fixture for each of E110 to E119 (list the paths).

## Gate

- `cargo fmt --check` is clean; clippy `-D warnings` is clean on the workspace.
- `cargo test --workspace` is green.
- `task test-ignite` is green on both targets.
- `task gate` is green.
- `task compile`, `task compile-llvm`, `task compile-todo` and `task compile-todo-llvm` succeed.

## Rules

- `CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-p4`.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`. Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- No git command that changes state: no add, commit, mv, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- No work files in /tmp or /dev/shm; no heredocs or `python3 -c`; never delete a `target/` directory; leave the `terminald` on port 2323 alone; kill your own processes by explicit PID before you finish.

## Final report to the coordinator, at most 15 lines

- `std/deploy.ignite` lines (code and tests separately);
- example goldens added;
- docs notes and `AGENTS.md` rows written;
- 15.0 items 4 to 11, pass or fail, each with its command;
- the slice-11 gate-time row;
- friction rows solved and opened;
- deviations from section 15.
