# Review of `docs/slice.md` section 22 (slice 18)

Independent review, 2026-11-05, of section 22 (lines 7362 to 7938) on `feat/slice18-one-service`.
The reviewer had not seen the author's reasoning. The review checks the section against
`epic:slice-18-one-service-operated`, `story:slice-18-contract`, `story:apply-recovery-way-out`,
`docs/research/review-2026-11-04.md` sections 1, 2 and 6, and the earlier sections the section
cites (15.4, 15.5, 16.5 to 16.17, 17, 21.3 to 21.6). Code was read at base `c19ab173`; nothing
was built or run. Citations are `path:line` at that commit.

## Blockers

**R1. Blocker. Row 8 contradicts row 11 and the epic's own wording.**
- Row 8 requires `git grep -n -e HttpHealth -e HealthCheck -e 'examples\.todo\.health' -- crates
  std tools` to print nothing.
- Row 11 requires the golden `crates/ignite/tests/golden/todo.model.json` to hold `"element":
  "examples.todo.health.HealthCheck"`. That file is under `crates/`, so row 8 prints it.
- The same applies to every test U4 and U6 write that names the probe. That includes
  `crates/ignite/tests/todo.rs`, `slice18.rs`, and the `health` executable that rows 16 to 18
  wait for.
- The epic's acceptance ("no file under `crates/` or `std/` names the probe or its outcome")
  has the same flaw. Its intent is "no general layer".
- Resolution: limit row 8 to the general layers: `-- 'crates/*/src' std 'tools/*/src'`. Record
  that goldens and tests under `crates/*/tests` may name the example. Ask the steering session
  to reword the epic line to match.

**R2. Blocker. Row 16 cannot see `InsufficientEvidence` in a `standing` record.**
- An evaluator publishes a standing only at an emission (`emission`, `std/control.ignite:631`
  to `:645`). `Start` publishes none for a rolling window (`arm_period`, `:595` to `:607`).
- At the first emission, `horizon` is that emission's slot (`:635` to `:638`). Then
  `expected_slots(h, 10 s, h, slot + 5 s)` is 1 (`:793` to `:800`). Completeness is
  1/1 = 100 % (`standing`, `:827` to `:830`), and `meets(99 %, 1, 1)` holds, so the first
  standing is `Compliant` (`:836` to `:844`).
- With 31 good slots and then 3 bad ones, the collapsed sequence is `Compliant, Violated`. The
  row requires `InsufficientEvidence, Compliant, Violated`.
- The only `InsufficientEvidence` today with zero attempts comes from `status`, which computes
  it. No record carries it (`run::zero_attempts_give_insufficient_evidence`,
  `crates/ignite/tests/run.rs:554` to `:612`). The epic's "reaches each `Status`" is therefore
  not met.
- Resolution: drive missing evidence between emissions. After the first good attempt, make
  several attempts `AttemptFailed`: replace the run's copied `health` executable in
  `<state>/bin/<generation>/` with one that exits 1, and restore it later. Missing attempts
  emit nothing but raise `expected`, so the next good emission publishes
  `InsufficientEvidence`. Require the collapsed sequence to be exactly `Compliant,
  InsufficientEvidence, Compliant, Violated`. The window must let the gap expire before the
  SIGSTOP.

**R3. Blocker. Row 25: a corrective apply is refused with exit 2, not exit 1.**
- Row 25 requires `ignite apply <corrective>` to exit 1 with `operation requires recovery;
  inspect its durable status`.
- That text and exit 1 come only from a retry of a plan that already has a receipt
  (`crates/ignite/src/apply/host/admission.rs:232` to `:251`).
- A new corrective plan reaches `corrective`, which is false because `correctable()` is false
  for a non-resolvable later reason (`controller.rs:410` to `:426`). With an active operation it
  is then refused with exit 2 `host has an active or unresolved operation` (`admission.rs:279`
  to `:281`).
- F16's "blocks every apply but a retry" is wrong too: the retry is refused as well (exit 1).
- Resolution: in row 25, expect exit 2 and `host has an active or unresolved operation` for the
  corrective. Also expect exit 1 and `operation requires recovery; inspect its durable status`
  for a retry of the failed plan, and assert both. Correct F16 to "refuses every apply:
  correctives with exit 2, a retry with exit 1".

**R4. Blocker. Section 22.3's decoding cannot see an outcome type outside `std`, and the files
that would fix it are not owned.**
- The driver gets one `TypedProgram`, the one of `std.control`. `ignite run` takes
  `modules.get(CONTROL_MODULE).types` (`crates/ignite/src/lib.rs:1579` to `:1591`) and passes
  it to `Live::new` and `replay::replay` (`:1613`, `:1659` to `:1668`). The apply path does the
  same (`crates/ignite/src/apply/revision.rs:111` to `:115`, `:129`, `:186`). `ignite replay`
  does the same (`lib.rs:1726` to `:1733`).
- `Value::decode` looks records up with `TypedProgram::record_fields`. That finds only "a
  record of this module or of a module it was typed against" (`crates/ignite-types/src/lib.rs:1440`
  to `:1448`).
- `std.control` is not typed against `examples.todo.health`, so decoding a `HealthCheck` line
  fails with an unknown record. Every attempt becomes `AttemptFailed`, and rows 16 to 18 never
  see a sample.
- The Telnet case works only because `std.sli` is an import of `std.control`. `Core::new`
  (`engine.rs:239`) receives no types at all, yet 22.3 decodes inside the `Emit` arm.
- U1 owns `run/{engine,live,probe,replay}.rs`. The call sites in `crates/ignite/src/lib.rs`
  (`run`, `replay`) and in `crates/ignite/src/apply/revision.rs` are owned by no unit.
- Resolution: state in 22.3 that the driver decodes with the typed program of the element
  type's module. Name the source: `Loaded::modules()`, looked up by the qualified name in
  `observations[].element`. State that `Core` and `replay` receive the same table. Add
  `crates/ignite/src/lib.rs` (`run`, `replay`) and `crates/ignite/src/apply/revision.rs` to
  U1's files; no W2 unit touches them. Add a unit test that decodes a record of a non-std
  module.

## Majors

**R5. Major. Section 22.9: `ignite recover` stops instances that a live supervisor restarts.**
- Row 25's drive path depends on a supervisor of the committed revision that restarts
  revision 1 after a SIGKILL, so that the restart finds the artifact mismatch (22.9, item 4).
  That supervisor is therefore being driven when `recover` runs.
- 22.9 drains and stops "every owned and retiring instance … through their pidfds", then
  "waits until each ended". It does not stop the revision controller (the `Live` run behind
  `runtime`) first.
- The supervisor sees `Exited` or `Signalled` and restarts under `Restart`. This is a second
  start of the very kind the epic forbids, and `recover`'s fresh start then races it for the
  port.
- Resolution: specify the order: stop the run driver of every revision (as `stop` does,
  `crates/ignite/src/apply/host/mod.rs:174`), then signal, wait, journal, and start afresh.
  Add to row 25 that between `operatorRecovered` and the fresh start the host journal holds no
  `Started` record.

**R6. Major. Section 22.2 and 22.4: a rollout can silently overwrite an acknowledged write.**
- 16.12 lets the old process keep serving its open sessions after `Released`, and starts the
  new one at once (`docs/slice.md` 16.12, items 2 to 5). An HTTP keep-alive connection can
  POST or PATCH on the old instance after the new one has loaded.
- Ids come from each process's own `next_id()`. The old instance can create id `k` while the
  new one, which loaded before that write, also creates `k`. Both write `todos/k/1.json`, and
  `write_file` replaces (21.4), so the first 201 is lost.
- The same happens to a PATCH: both processes write version `n+1` of one id.
- 22.2 says the old instance's write "is on disk and visible to the new instance at its next
  start". That is false for these cases.
- Resolution (either one):
  - Make the version files collision-free and the load merge them. For example, name them
    `<version>-<generation>.json` and choose ids from a range per instance. Add a row that
    POSTs on the draining connection after the new instance answered.
  - Or say plainly that an acknowledged write in the drain window can be lost, and give the
    todo a rollout that ends the old sessions before the new instance serves.
    `on_deadline = "disconnect_remaining"` with a short `deadline` alone does not do that,
    because `Spawn(new)` follows `Released`.

**R7. Major. Sections 22.4 and D7: the data's location depends on the host's working directory,
which nothing records.**
- `Directory.path` is "relative to the executable's working directory at launch" (21.4). Under
  `apply` that is `ignite host`'s, "which the saved plan does not record". 21.16 Q2 concludes
  that "a declaration meant for `apply` uses an absolute `path`".
- Section 22 declares the `apply` service with a relative path. If the host is restarted from
  another directory, `create = true` creates an empty `todo-data`, and the service answers
  `GET /todos` with `[]` and no diagnostic. That is a silent data loss as seen from the client,
  on the slice's headline row 17.
- Resolution: choose one.
  - Have `ignite host` record its working directory in the state (identity or `status.json`)
    and refuse to start from another (exit 2).
  - Or resolve a relative `Directory.path` of an `apply` revision against the state directory.
  - Or keep relative and add the limitation to 22.2 and to `AGENTS.md`.

  Reconcile the choice with 21.16 Q2 either way.

**R8. Major. E151's list of undecodable types is not the decoder's.**
- 22.3 lists "a fn type, …, `Signal<T>`, `Ref<K>`" as having "no 15.4 decoding", and row 4's
  message says `fn() -> Int` is a type "which no probe line encodes (15.4)".
- 15.4 encodes a named top-level fn as `{"fn": …, "type": "Fn"}`, `Ref` and `Signal` likewise.
  `Value::decode` decodes all three (`crates/ignite-eval/src/value.rs:520` to `:551`).
- It refuses only `TcpListener`, `Capability`, `TcpStream`, `PipeStream`, `IoError`, `Store`,
  `Never` and `Param` (`:552` to `:559`).
- A unit that builds E151 from the decoder would not fire on row 4's fixture. A unit that
  builds it from the text would ship a message that is false.
- Resolution: make E151 a policy, not a claim about decoding. Admit as outcome types `Bool`,
  `Int`, `Text`, `Byte`, quantities, and lists, options, tuples, records and enums of those.
  Reword the message, for example ``signal `S.p.beats` carries `fn() -> Int`; a probe outcome
  holds no fn, handle, reference or signal``. Change section 8's paragraph and 22.10 to match.

**R9. Major. E151 is keyed on a binding variant's name, which the compiler must not know.**
- E151 is "a signal of a member bound to a `scheduledProbeHost`" (22.3, 22.10, section 8).
- 16.17 "As landed" says "the compiler knows no kind, binding variant, outcome or zone name".
  E121, E122 and E129 rest on the binding module's `entries` fn
  (`crates/ignite-eval/src/resolve.rs:56` to `:65`, `std/deploy.ignite:97` to `:100`), and so
  does vision invariant 6.
- Resolution: define E151 in terms the compiler already has. For example: a signal that the
  assembly runs (its producer's binding has entries) and that a member with a machine
  subscribes to (`ModelMachine::subscribes`, `crates/ignite-eval/src/model.rs:349` to `:375`).
  Say that E151 fires with E150's subscriber rule.

**R10. Major. U2 breaks two goldens it does not own, inside W2.**
- The model lists every defaulted field and its value: `"defaults": ["limits"]` and the full
  `limits` record (`crates/ignite/tests/golden/todo.model.json:6` to `:52`).
- U2 adds `recovery` and `rollout` with defaults to `HttpServer`. That changes
  `todo.model.json` (`plan::todo_example_model_matches_golden`) and `studio.model.json`
  (`plan::studio_model_matches_golden`, `crates/ignite/tests/plan.rs:291` to `:306`, over
  `fixtures/slice17/studio.ignite`, which declares `HttpServer`).
- U2 owns neither golden. `todo.model.json` is U4's (W3), so U2's merge gate (row 31) is red.
- Resolution: give U2 both goldens, with the edit limited to the new defaults. U4 then rewrites
  `todo.model.json` in W3. Say so under "Shared files".

## Minors

**R11. Minor. Section 22.9 adds a receipt state `abandoned` that nothing declares.**
- 17 does not list it, and neither does 22.10.
- A retry of the abandoned plan falls through to `operation requires recovery`
  (`admission.rs:244` to `:249`), although no recovery is pending.
- `apply`'s message prints `operation <id> is abandoned` (`crates/ignite/src/apply/mod.rs:369`
  to `:384`).
- Resolution: add `abandoned` to the receipt states of 17, with its retry answer (exit 2
  `operation <id> was abandoned by ignite recover; prepare a new plan`). Add an assertion to
  row 25.

**R12. Minor. The status line claims a resolved review.** Lines 7364 to 7366 say the review "is
resolved in this text; 22.17 maps each blocker", but 22.17 (lines 7935 to 7938) is empty.
Resolution: say "review pending" until the resolutions are written.

**R13. Minor. The sources cite "16.17 (hooks)".** Hooks are 16.16 ("Typed API", and `AGENTS.md`
"the hooks `machine` and `validate` of 16.16"). 16.17 is the diagnostics. Resolution: cite 16.16
and 16.17.

**R14. Minor. Row 10's command does not run.** `cargo test -p ignite --test main
plan::fixtures_match_expected plan::store_is_hosted_by_its_server` passes two positional test
names. Cargo accepts one `TESTNAME` before `--`. Resolution: write `cargo test -p ignite --test
main -- plan::fixtures_match_expected plan::store_is_hosted_by_its_server`, or two commands.

**R15. Minor. Section 22.6: the "inferred" fallback is settled by the code, and the import list
is too strict.**
- `Endpoint.of` takes the kind-erased `Ref`: `("Endpoint", "of") => sig(EndpointOf,
  vec![Ty::Ref(None)], …)` (`crates/ignite-types/src/builtins.rs:297`).
- A `Ref<K>` argument is compatible with it (`crates/ignite-types/src/typer.rs:107`). E122
  matches `Ty::Ref(_)` (`crates/ignite-eval/src/validate.rs:509`).
- `HttpHealth.main` also needs at least `std.io.IoError` for its result type, and text
  helpers to parse the status line. The claim "imports from `std.net`, `std.control` and
  `std.sli` only" will not hold.
- Resolution: drop the `model.ignite` fallback, cite the builtin, and list the imports as
  "general std modules only, never `examples.todo`".

**R16. Minor. `classify` is not checked as a hook.** E109 checks only `machine` and
`validate` (`hook_signature`, `crates/ignite-types/src/typer/system.rs:1662` to `:1673`).
An effectful or misshapen `classify` therefore reaches run time, or E150's message. Resolution:
extend E109 to `fn K.classify(self: K, sample: T) -> Bool`, pure, and give it a fixture. That is
a file in `ignite-types` and needs an owner (U1).

**R17. Minor. Section 22.7: the order of the torn-line repair, and read-only verification.**
- `History::load` truncates the torn bytes and syncs at once (`journal.rs:208` to `:211`).
- A host that dies before appending `journalRepaired` loses every trace of the drop.
- Offline `inspect` cannot reuse `load` as it stands, because `load` writes. The offline
  answer for a journal that is only torn is also unspecified.
- Resolution:
  - Compute the digest at load, append `journalRepaired` as the first write after `listen`,
    and accept that a crash in between loses the record. Or append it before truncating, as a
    record whose `previousDigest` is the last good one.
  - Split a read-only verifier out of `load`.
  - Say that offline `inspect` of a torn-only journal reports `"intact": true` with a pending
    repair.

**R18. Minor. Section 22.7: the revision-journal comparison is not byte-level, and its natural
home is not U3's file.**
- The host journal keeps the parsed record, `("record", record.clone())`
  (`controller.rs:1316` to `:1328`), not its line.
- Replay of retained revisions happens in `verify_history` (`recovery.rs:218` to `:283`, U5's
  file in W3) and `revision::verify` (unowned).
- Resolution: define the check as "the compact encoding of the copy equals the line". Say it
  runs in `host/mod.rs` before `verify_history`, or give U3 the call in `recovery.rs` and
  order U5 after it (it already is).

**R19. Minor. E126 already names the format version.**
- `check_header` reports ``the journal has formatVersion `{version}`, this driver writes `…` ``
  (`crates/ignite/src/lib.rs:1459` to `:1475`), so 22.10's "makes E126 name …" changes nothing.
- The existing `e126_journal_mismatch` already writes `ignite-journal/0.1`
  (`crates/ignite/tests/run.rs:1010` to `:1029`). After the bump its `.expected` changes from
  the model text to the version text.
- Row 6 also asks for `ignite replay`, which the test does not run. Neither the `.expected`
  file nor that test is in U1's list.
- Resolution: say "unchanged; the bump makes the fixture hit the version branch". Give U1 the
  fixture and the test, and extend the test to `replay`.

**R20. Minor. S5 misses pages that state the old payloads.**
- Missed pages: `site/book/src/reference/std/control.md:68`, `:71` and `:106`
  (`AttemptDone(Int, ProbeAttempt)`, `Emitted`, `Emit`); `reference/std/index.md:24`;
  `reference/std/telnet.md`; `design/intent.md:143`.
- Also missed: the doc comment at `crates/ignite/src/lib.rs:755` ("`ProbeAttempt`, the probe
  event's payload").
- Resolution: add them to S5 and to U1's files.

**R21. Minor. Row 16 details.**
- `ignite run` takes a file, so write `<copy>/todo.ignite`.
- A SIGSTOPped `todod` does not handle the `stop` SIGTERM until it is continued. The test
  must send SIGCONT before `stop`, or the stop waits for its deadline.
- Resolution: state both.

**R22. Minor. Row 14 says "unchanged".** The helpers `answer` and `status` take
`Store<Todo>` (`examples/todo/todo.ignite:97` to `:104`), and the handler's state type changes
to `Todos`. The tests keep their assertions, not their text. Resolution: "with the same
assertions".

**R23. Minor. Row 27 times one more executable in the after tree.** After the slice,
`ignite build examples/todo/todo.ignite` also builds the probe `health` (implicit executable
name = member name, as `promptProbe` in `terminal.design.model.json:211`). The `todod` unit is
then two links on the after side and one on the before side. Resolution: say that the unit
builds both, and give the split per executable, or build `todod` alone.

**R24. Minor. Row 25's drive path should name the replaced file.**
- A mid-run restart reuses `revisions/<r>/bin/<generation>/<exe>`.
- A fresh start recopies the plan artifact over it (`copy_into`,
  `crates/ignite/src/run/live.rs:1587` to `:1599`). That is what lets `recover`'s restart
  succeed.
- If the test replaces the plan artifact instead, the restart from `recover` mismatches again
  (`controller.rs:1506` to `:1541`), and row 25 cannot exit 0.
- Resolution: name `revisions/<r>/bin/<generation>/terminald` (the row runs on the terminal
  design) and cite `copy_into`.

**R25. Minor. Section 22.3: "the element type of each signal" leaves two cases open.**
- `ModelObservation::element` is a type text (`model.rs:238` to `:240`). A generic element
  (`List<…>`, `Option<…>`) needs parsing.
- A probe kind may declare several `Signal` outputs.
- Resolution: decode by the observation of the scheduler's `signal` field. Limit element types
  to what R8's admitted set can name, and say how the text becomes a `Ty`.

**R26. Minor. Section 22.4 behaviour gaps.**
- `write_file` needs its parent (21.4), so the handler must `create_directory("todos/<id>")`
  before writing a new id. Otherwise every POST answers 500.
- After a failed write the in-memory value is already updated. Later reads show it, and a
  later write of that id persists it, although the client got 500 `not stored`.
- Resolution: state the directory creation. Either roll the memory back through a second
  `update`, or document that a 500 means "not durable", not "not applied".

**R27. Minor. The epic's timing line is not met as written.** The epic asks that the todo's
edit-build-restart cycle be "timed before and after the slice". D12 times the todo after only,
and the terminal design on both trees, for a sound reason (F8 to F10). Resolution: record the
deviation in the epic's acceptance (steering session) or in 22.2, so that U8's records do not
read as a miss.

**R28. Minor. Section 22.9 does not say what a crash during `recover` leaves.**
- `recover` signals first and journals `operatorRecovered` afterwards.
- A host killed in between restarts with the operation still `recovery_required` and its
  instances gone. That is safe, but it is unstated, and no row covers it.
- Resolution: state that `recover` is repeatable and that the restarted host reconciles as
  after any crash (17.5). Optionally journal an intent before the first signal and reuse
  `IGNITE_DEBUG_HOST_FAULT` for one row.

## Checked and consistent

- 22.1 citations F1 to F19 match the code at `c19ab173`. A few cite the comment above the
  item, for example F7 at `std/control.ignite:24`, the enum at `:28`, and F13's
  `History::load` at `:196`.
- Also correct:
  - `examples/todo/todo.ignite:7`, `:20`, `:80` to `:89`, `:84`;
  - `engine.rs:26` (`JOURNAL_VERSION`), `plan.rs:21` (`REDUCER_EVALUATOR`);
  - `live.rs:883` to `:899`, `controller.rs:1533` to `:1541`;
  - `apply.rs:756`, `apply_recovery.rs:472` (it holds the `after_dispatch` step at `:498`);
  - the F18 working-directory lines.
- 22.10 cites `crates/ignite/tests/plan.rs:430`. That line is the `#[test]` attribute, and the
  fn is at `:431`.
- Every cited existing test exists: `plan::design_model_is_byte_identical_across_processes_and_directories`,
  `plan::fixtures_match_expected`, `plan::todo_example_model_matches_golden`,
  `run::terminal_runs_under_ignite_run{,_on_the_llvm_target}`, `run::e126_journal_mismatch`,
  `control_trace::reducers_agree_across_the_evaluator_and_both_targets`,
  `todo::{rust,llvm}::{concurrent_posts,concurrent_patches}`,
  `apply_recovery::a_changed_reducer_revision_starts_the_host_without_driving_and_asks_to_replan`,
  `apply::tampered_host_history_is_rejected_before_startup`,
  `apply::host::journal::tests::a_torn_final_line_is_dropped_and_the_chain_continues`,
  `apply::{rust,llvm}_saved_apply_retry_source_admission_and_retained_restart`.
- The language features used exist:
  - generic kinds inferred in declarations (`fields_literal`,
    `crates/ignite-types/src/typer/system.rs:677` to `:766`);
  - calls of fn-typed fields (`application.handle(…)`, `std/protocol/http.ignite:655`) and the
    evaluator's call of `Value::Fn` (`crates/ignite-eval/src/eval.rs:489`);
  - generic hook receivers (`hook_signature` accepts `GenericRecord`, `system.rs:1678` to
    `:1682`);
  - `Store.upsert` and `update` atomic with a pure `f` (11.3);
  - `Directory.read_directory`, `create_directory`, `remove` (21.4).
- Replay stays exact if R4 is fixed: `classify` is pure, is evaluated with the run's `--fuel`
  in both `run` and `replay` (`ReplayOptions::fuel`, `crates/ignite/src/lib.rs:170`), and reads
  the model that the header check pins (`check_header`).
- The Telnet case keeps working: `ProbeAttempt` stays in `std.sli`, which `std.control`'s typed
  program knows, and the two predicates keep the old meanings.
- 22.4 is correct under `concurrent_posts` (16 × 100 POSTs, ids `1..=1600`,
  `crates/ignite/tests/todo.rs:356` to `:385`) and `concurrent_patches` (atomic `update` per
  PATCH, removal only of lower versions, highest version wins on load), within one process.
  R6 is the cross-process case.
- Rows 17 and 19: an implementation-only change gives `replace` only to members with an
  artifact, and does not propagate to the `Store` members (`plan.rs:551` to `:557`, `:586` to
  `:597`). A stale source is refused with exit 2 `input content changed: <path>`
  (`verify_file`, `plan.rs:1089` to `:1095`), which names the file.
- The assembly of 22.4 resolves once U2 lands:
  - E110: `data` is a capability and is hosted implicitly; `todos` and `loaded` are hosted
    by U2.
  - E112: `admits`.
  - E121 and E129: `entries` gives `main` and `machine`, both on `HttpServer` after U2 and on
    `HttpHealth`.
  - E122: exactly one `TcpListener` among the server's property refs.
  - E123: 2 s < 10 s.
  - E124: 5 min ≥ 10 s.
  - The probe's implicit executable is named `health`.
- Journal integrity (22.7) is sound in its three refusals. The digest covers the canonical
  record (`chained`, `journal.rs:129` to `:141`), so any changed value byte fails. A refusal
  in `History::load` comes before `listen` and before any process start (`host/mod.rs:84` to
  `:120`), so row 21's "byte-identical" holds: the lock file is opened without truncation. No
  refusal can start an operation twice. The torn-line drop cannot hide a synced record
  (`append_to` syncs before any crash point, `journal.rs:162` to `:184`).
- The reducer revision bump keeps old host states startable: retained revisions under another
  reducer revision are not replayed (`verify_history`, `recovery.rs:259` to `:262`). A 0.1
  revision journal is therefore never read as 0.2.
- E150 and E151 collide with nothing. No `E15x` exists in `crates`, `std` or earlier
  sections. Section 8's paragraph (`docs/slice.md:998` to `:1003`) matches 22.10.
- The work split:
  - Every row has an owner.
  - No wave has more than 3 units.
  - Within each wave the owned files are disjoint, apart from R4, R10 and R18.
  - Each code unit names a first failing test.
  - Only one unit per wave adds a `mod` line.

## Resolutions

Every finding is folded into `docs/slice.md` section 22; 22.17 has the one-line map. What
changed for each blocker:

- R1: row 8's grep now covers `crates/*/src`, `std` and `tools/*/src` only, and the row says that
  tests and goldens under `crates/*/tests` may name the example. The steering session is asked to
  reword the epic's line.
- R2: row 16 now has four phases: 3 good slots; 4 slots whose probe executable
  (`<state>/bin/<generation>/health`) is replaced by one that exits 1; the executable restored
  and 31 good slots; then SIGSTOP and 3 slots. The required collapsed sequence is `Compliant`,
  `InsufficientEvidence`, `Compliant`, `Violated`. The row cites why the first standing is
  `Compliant`.
- R3: row 25 expects exit 2 `host has an active or unresolved operation` for a corrective, and
  exit 1 `operation requires recovery; inspect its durable status` for a retry of the failed
  plan. F16 now says that every apply is refused, each with its own exit.
- R4: 22.3 cites the single `std.control` typed program (`lib.rs:1579` to `:1591`,
  `revision.rs:111` to `:115`) and `record_fields`. It specifies that `Live::new`, `Core` and
  `replay` receive the loaded modules and decode by the element type's module. U1 now owns the
  call sites in `crates/ignite/src/lib.rs` and `crates/ignite/src/apply/revision.rs`.

Majors: R5 stops the run drivers first and adds a `recoverStarted` record, with a row-25
assertion. R6 states the drain-window loss plainly in 22.2, without a fix in this slice. R7 adds
`<state>/workdir` and row 33. R8 and R9 redefine E151 as an outcome-type policy over `entries`
and subscribers. R10 gives U2 the default-only golden edits. The minors are mapped in 22.17.

After the rebase onto `f3abeb20` (slice 17 U6, 21.18): `HttpServer` already has `recovery`,
`rollout`, `machine` and `validate`, and both goldens were updated by slice 17. R10 is void, D5 is
settled, and U2 is renamed `story:store-hosted` and owns only Store hosting in `resolve.rs`.
