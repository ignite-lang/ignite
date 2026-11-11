# Diagnosis: row 1.6, corpus units the evaluator cannot run (362, v0.15.0 236)

Written by the coordinator from the agent's final message (the agent could not write this file).
Evidence in this directory: `scan.rs` and `scan.tsv` (typed call-graph scan of 856 units),
`table.py` -> `table.txt`, `order.py` -> `order.txt`, `calib.py` -> `calib.txt` (prototype run),
`proto_patch.py`, `risk_recommended.txt`, `probe_nodes.txt`, `probe_fuel.txt`, `probe_nodes2.txt`,
`reasons.py`.

## Counts (static estimate)

The scan walks the typed AST of every inline test transitively. Checks: all 362 listed units
scanned; 0 listed units with a recorded builtin the scan missed; 0 of the 494 units that run
today have a static need. The scan follows every branch, so it can only ask for more than a run
needs. Limits and model deadlocks are not in the number.

Family reached, units: pipe 188, spawn 184, channel 142, role 86, store 60, clock 54, census 44,
tcp 41, wall clock 29, proc 15, main 11, dir 11, limit 9, entropy 4, caps 4, stdio 4, exit 3.

Cumulative build order, runnable -> row 1.6:
module field 1 -> 361; + store 25 -> 337; + caps 29 -> 333; + pipe 55 -> 307; + spawn + role
122 -> 240; + channel 189 -> 173; + monotonic clock and sleep 205 -> 157; + wall clock 226 -> 136;
+ census 263 -> 99.
No combination without spawn gets below 251. Smallest set at or below 236: pipe + spawn + role +
channel (208).

Calibration (dynamic): a prototype of store, capabilities, role calls and the module-field fix
(161 lines in `eval.rs`: store 81, capabilities 30, role calls 45, module field 5) ran 29 of 29
predicted units; 28 pass, `tests/store.ignite::an overflow in update ends its test` fails with
`+ failed at ...store.ignite:44:23: 9223372036854775807 + 1` (the runner compares that with the
Rust target's report). `call_role` compiled but was not exercised.

## How the evaluator handles builtins today (read in the tree)

- `enum Value` (`crates/ignite-eval/src/value.rs:30`) has no handle variant and no heap.
- `crates/ignite-eval/src/eval.rs`: `call_expr` (1048) and `method` (1082) route
  `Callee::Builtin` to `builtin_static` (1129) or `builtin_method` (1184), then `mutating` (1537)
  or `builtin_value` (1239, `&self`).
- "has no value at compile time": `eval.rs:1179`, `:1525`, `:719` (`spawn`), `:1073` and `:1122`
  (role calls).
- Limits: `Fuel { units, nodes }` 1,000,000 each (`eval.rs:42-57`); the runner uses 10,000,000
  units, nodes 1,000,000 (`crates/ignite/tests/differential.rs:60`, `:645`), `SLOW` 60 s (`:71`).
- `cannot()` at `differential.rs:614`: a `Failed` report starting with `evaluation failed: ` and
  every `Limit`.

## Models (designs; only store and capabilities were built)

- Store (`ignite-io` `store.rs:315`): list of (last id, ordered map); ids 1, 2, 3; `values` in
  ascending id; `update` returns the new value or `None` without calling `f`. Not modelled: lock
  and shard contention.
- Capabilities (`environment.rs`): the argument itself.
- Role call: call the role's `FnDecl` in `Decl::Protocol`.
- Pipe (`pipe.rs:54`): two ends, each with a send buffer delivered as one batch at a flush point
  (before a read, on close, at task end, at end of run, at 64 KiB, on drop of the last handle,
  from `Clock.sleep`, a waiting channel operation, `Runtime.settle`, `Process.flush`); 16 MiB
  bound, peer-gone and closed errors, idle timeout then Eof, urgent byte, `received_batches`.
- Spawn (`task.rs:152`): cooperative run-to-block scheduler, ready tasks in spawn order; a
  failure ends only the task; remaining tasks dropped when the test body returns. A task that
  spins ends on fuel and stays cannot-run; a model deadlock is reported as `evaluation failed`.
- Channel (`channel.rs:31`): bounded queue; capacity below 1 fails the task; closed by `close` or
  by drop of the last end.
- `Clock.monotonic` / `Clock.sleep`: a virtual clock that advances only when every task is
  blocked, to the earliest deadline.
- Out: census, entropy, `Directory.temporary`, TCP/TLS, child processes, stdio, `Process.exit`,
  executables whose `main` does IO.
- Cross-cutting: active only under `run_test` (`eval.rs:304`), so `ignite check` and `ignite
  plan` keep "no value at compile time"; handle drops need `TypedProgram::moves`
  (`crates/ignite-types/src/lib.rs:1852`), which the evaluator ignores today.

## Plan (steps, units, row after, line estimate)

1. module field in `field_of` (`eval.rs:1706`): 1, 361, 6 lines.
2. handle value and store heap, test mode only: 24, 337, 200.
3. capabilities: 4, 333, 40.
4. pipe on one task: 26, 307, 250.
5. tasks, virtual time, last-use drops, role calls: 67, 240, 400 to 500.
6. channel: 67, 173, 150.
7. monotonic clock and sleep: 16, 157, 60.

Cost estimate: a new `crates/ignite-eval/src/substrate.rs` (700 to 900 lines), 350 to 450 lines
in `eval.rs`, a `Value::Handle` variant with arms at `value.rs:171`, `:193`, `:229`, `:331`,
`model.rs:809`, `:1690`, `explain.rs:259`, `:305`.
Risk to row 1.1: every newly runnable unit is newly compared with the targets (205). Of the 200
outside steps 1 and 3: 140 reach an idle timeout or `receive_within`, 135 spawn tasks, 21 use
urgent data, 20 assert `received_batches` exactly. Units by file: http 57, telnet 19, jsonrpc 17,
resp 14, `http_app_adversary` 11, sync 10. More than 60 of the 189 units after step 6 would have
to stop on a limit for the target to be missed.
Not checked: agreement with the targets; the protocol suites were not run.

## Units whose reason is a limit

- Three million-byte digest units: about 136 M fuel units and a node limit between 2 M and 3 M
  (`probe_fuel.txt`, `probe_nodes2.txt`); a per-unit override in `differential.rs`; whether they
  fit in 60 s is not measured.
- Two http units at Nodes(1000000) also need pipe, task and channel models.
- Two "never ends" units stay. Two `slice16/moves_scale.ignite` units need last-use moves.
