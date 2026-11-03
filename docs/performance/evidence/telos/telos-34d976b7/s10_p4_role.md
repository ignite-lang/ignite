You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read these first:
- `AGENTS.md`, including the "Gate time" section;
- `docs/friction.md`;
- `docs/slice.md` sections 6.1 and 14, all of 14.0 to 14.14 (your contract);
- `docs/vision.md` section 7 (T1, T5, T6);
- `docs/Telos_Design.md` sections 3.3 and 11.3;
- `examples/terminal/terminal.design.telos`.

You are P4, the last slice-10 agent. P1 (frontend), P2 (Rust target and `telos-io`) and P3 (LLVM target and `telos-rt`) have passed their gates, and you own the whole tree now. If a section-14 feature does not work as the contract says, fix it in the crate concerned and add a regression test. Name every such fix in your report.

## 1. Std modules (14.6)

Write the kinds in Telos, each with inline `test` blocks:

| module | contents |
|---|---|
| `std/console.telos` | `CommandConsole`, `ConsoleCommands` |
| `std/protocol/telnet.telos` | `TelnetServer`, `TelnetClient`, `TelnetProfile`, `SessionLimits` |
| `std/protocol/http.telos` | `HttpServer` |
| `std/contract.telos` | `Availability`, `ThresholdCompliance`, `SLA`, `Window`, `Consequence` |
| `std/policy.telos` | `Restart`, `Backoff`, `DrainRollout` |
| `std/probe.telos` | `TelnetPrompt`, `ProbeAttempt` |
| `std/deploy.telos` | `Deployment`, `Binding`, plus the fns `localSocket`, `nativeProcess`, `workstationExecutable` and `scheduledProbeHost` |
| `std/cli.telos` | `Application`, `Command`, `Argument`, `ArgKind` |

- The executable kinds get `fn K.main(self: K) -> Result<(), IoError>`: `TelnetServer`, `TelnetClient` and `HttpServer`. Their accept loop and per-connection `spawn` are written in Telos. Exit codes go through `IoError.with_exit_code(n)` (14.5, O12).
- `SessionLimits.idle_timeout` replaces `IDLE_TIMEOUT_MS`.
- `Endpoint` in `std/net.telos` was written by P1. You may extend it; do not move it.
- Every closed-set field the design writes as a string stays `Text` (14.6).

## 2. Both examples in the component form (14.5)

- `examples/terminal/terminal.telos`: `ingress`, `console: CommandConsole` with `module("…")`, `server: TelnetServer` with `executable = "terminald"`, and `client: TelnetClient` with `executable = "tn"`.
- `examples/todo/todo.telos`: `HttpServer` with `executable = "todod"`.
- Behaviour is unchanged. The terminal suite, the todo suite and the inline tests pass on both targets, with the same counts as before or with each difference explained.

## 3. Service/Client cutover

- Remove the compiler-known `Service` and `Client` lowering from both backends, together with its checker path.
- The E020 fixtures that named `Service` or `Client` get one of two treatments: rewritten against a kind without `main` where the rule still exists, or retired with their `docs/slice.md` section 8 row updated.
- Section 6.1 of `docs/slice.md` is rewritten to point at 14.5.
- Agree the order of the cutover with nobody: you are alone in the tree.

## 4. The design file (O5)

- `telos check examples/terminal/terminal.design.telos` exits 0.
- Allowed edits are only those listed in 14.13 O5 and its amendments:
  - the package prefix in `module("…")`;
  - the `std.telnet` handler path;
  - `type =` becomes `kind = endpoint(Telnet)`;
  - the imports of `workstationExecutable` and `scheduledProbeHost`, and of `std.protocol.telnet.Telnet`.
- Mirror each edit in `docs/Telos_Design.md` §11.3. Add a §3.3 note that the quantity type for byte sizes is `ByteSize`, because `Bytes` is `List<Byte>`.
- Add the design file to `task test-telos`'s check step, or to a cargo test, so it cannot rot. It is still excluded from builds.
- P1 found that, with the O5 edits applied in a scratch copy, the only diagnostics left are E031 for the std kinds you add. P1's fixture `valid_system` mirrors the design file with local kinds. Once your std kinds exist, delete the local kinds from that fixture, or keep it and say why.
- `module("…")` with an expected `Option<I>` gives `Option<I>`, as for `command_schema` (P1's 14.14 items 7 to 16). Read those items before writing std.

## 5. Acceptance

Verify every line of `docs/slice.md` 14.0, items 1 to 6, yourself. Report each as pass or fail, with the command you ran. P1 and P2 own 14.0.4 and 14.0.6, but nobody has re-checked them after those agents:
- **14.0.4:** R2 to R5 are fixed, and `docs/audit/2026-11-01/io/iotest` `drop` and `cross` behave as the audit README's "expected after the fix" column says. Build iotest with its own `--target-dir` under the scratch directory.
- **14.0.6:** each of E100 to E109 has a fixture that triggers it, and E104 has three, one per message text.

Headline item 14.0.3:
- `terminald` survives a session whose handler fails an `assert`, overflows or indexes out of range, on both targets.
- P2 and P3 each have an end-to-end test on a fixture copy. Make that test run against the real `examples/terminal/terminal.telos` path where possible. If the example has no failing command, keep the fixture, and state which form you kept.
- Each failure report has the 14.8 format and names the Telos file and line.

## 6. Docs and records

- `docs/friction.md`: mark the rows solved by slice 10, and add the friction you hit.
- `AGENTS.md`: update the layout table with the new std modules. Add a slice-10 row to the "Gate time" section with:
  - `task gate` wall time, cold and warm;
  - `cargo test --workspace --no-run`;
  - the `todod` build time on both targets.

  Use the same commands as the 9b rows. Measure only "after"; do not build a before-tree. The "before" numbers are the existing 9b and 9c rows and the program-build line below the table:

  | measure | before |
  |---|---|
  | gate, cold | 86.5 s |
  | gate, warm | 25.9 s |
  | todod, Rust | 3.0 s dev, 3.9 s release |
  | todod, LLVM | 0.8 s dev, 0.9 s release |

  A rise of more than 10 % is a deviation: measure where it comes from.
- `docs/vision.md` 4.1 row 10: do not edit; the steering session owns that file. Put the numbers in your report.

## Gate

- `cargo fmt --check` is clean.
- clippy `-D warnings` is clean on the workspace.
- `cargo test --workspace` is green.
- `task test-telos` is green on both targets.
- `task gate` is green.
- `task compile`, `task compile-llvm`, `task compile-todo` and `task compile-todo-llvm` succeed.

## Rules

- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k` for direct cargo calls.
- Every cargo call sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- Prefix test runs with `ulimit -c 0;`.
- Run one cargo or task run at a time. Anything longer than about 30 s goes to the background; wait on its own PID or on a final `EXIT=` line you write into its output file yourself. Do not stack runs.
- The 100,000-term build-and-run tests are `#[ignore]`; do not run them.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- No git command that changes state: no add, commit, checkout, restore, stash or reset.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone. Use free ports in tests.
- No work files in /tmp; use the scratch directory.
- Kill your own processes by explicit PID before you finish.

## Final report

At most 15 lines:
- the std modules with their line counts (code and tests separately);
- the example line counts before and after;
- the cutover: fixtures rewritten or retired;
- the design-file edits;
- the 14.0.3 evidence on each target;
- compiler fixes;
- the gate counts and the slice-10 gate-time row;
- friction rows solved and opened;
- deviations from section 14.
