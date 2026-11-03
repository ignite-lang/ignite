# Slice 14 contract review

Status: reviewed, findings resolved, approved for implementation in advance by Timo (2026-11-03).

Independent review of the drafted contract, `docs/slice.md` section 18 (uncommitted on
`docs/slice14-contract`, base `c33442a`, later `340f782`). 1 blocker, 6 should-fix, 15 notes.
The resolution of each finding is at the end of this file.

Reviewed against `docs/vision.md` §4 (wave B), §6, §7 (T1 to T4) and invariants 1 to 9;
`docs/protocols.md` §1.2, §2.2, §3.3, §3.6, §5.1, §5.3, §6, §7 row P1; and the code at `c33442a`
(std, `crates/ignite-{syntax,check,types,rustgen,llvm,io,rt,eval}`, `crates/ignite`,
`docs/layering.md`). One claim was checked at run time; every other finding is read from source.

## Findings

### Blockers

1. **Blocker. The transcript cannot live in `examples/terminal/transcript.ignite` (D14, 18.12,
   18.0 row 3, 18.16 U5).**
   - Evidence:
     - Sibling modules are the files beside the program whose package is `<program
       package>.<stem>`: `parse_siblings` in `crates/ignite/src/lib.rs`, section 14.4 item 13.
     - A program with `package examples.terminal.transcript` therefore has no siblings, and its
       `import examples.terminal.commands.respond` is E031. Observed in a scratch copy of
       `examples/terminal/` with the `ignite` binary of 2026-11-02 16:48. Its sibling and E031
       code equals `c33442a`'s: `git diff 11b7784 c33442a` touches neither.

       ```
       terminal/transcript.ignite:3:8: error[E031]: cannot import from `examples.terminal.commands`: only standard-library (`std.*`) modules and the sibling modules of the program's package can be imported
       ```

     - The same file is a sibling of `terminal.ignite` (stem `transcript`, package
       `examples.terminal.transcript`). Every `check`, `build` and `plan --for-apply` of the
       terminal would load it, and the terminal's source-bound inputs would change. That
       contradicts "`terminal.ignite` … stay unchanged" in D14.
     - `task test-ignite` passes every file under `examples/`, so the gate fails too.
   - Required change:
     - Decide the location now, and drop the conditional fallback. The fallback,
       `examples/terminal/commands.ignite`, works: as a program its package is
       `examples.terminal.commands`, and it may import `std.*`.
     - Set row 3's command to `ignite test examples/terminal/commands.ignite --target all`.
     - Add `commands.ignite` to U5's files, to 18.12 and to the AGENTS layout row.
     - Name the constraint: `llvm_build::rebuild_compiles_only_what_changed`,
       `apply::` and `apply_recovery::` copy and edit this file. They replace
       `Status => Reply { lines = ["ready"] }`, so the test must not repeat that text.

### Should-fix

2. **Should-fix. Row 6's boundary "16 do not" is ambiguous (18.0 row 6).**
   - Evidence: `decode` counts the IAC of `IAC SE` toward the limit. See the test "a
     subnegotiation is limited to 1024 bytes including the IAC of IAC SE" in
     `std/protocol/telnet.ignite`: 1023 data bytes then `IAC SE` pass, and 1024 data bytes then
     `IAC` is `SubnegotiationTooLong`.
   - Consequence: under a 16-byte limit, `IAC SB` + 16 data bytes + `IAC SE` ends the session.
   - Required change: state the bytes of both cases. For example: `IAC SB`, 15 bytes, `IAC SE`,
     then a line that is answered; and `IAC SB`, 16 bytes, `IAC SE`, which ends the session after
     the prompt. Or say explicitly that the count includes the IAC of `IAC SE`.

3. **Should-fix. Row 5 names no reply bytes (18.0 row 5).**
   - Evidence:
     - The echo console replies with `line.to_text()`.
     - `BytesToText` is lossy on both targets: `String::from_utf8_lossy` in `ignite-rustgen`,
       `text_from_bytes_lossy` in `ignite-llvm`. `terminal.rs` already expects `\u{FFFD}`.
     - So the reply to two data bytes 255 is `EF BF BD EF BF BD 0D 0A` plus the prompt.
       `protocols.md` §5.1's expected `[b'a', IAC, …]` cannot occur.
   - Required change: give the exact expected bytes, or use a console that answers the line's
     byte count. The length discrimination via `maximum_line = 2` is sound and should stay.

4. **Should-fix. 18.13 omits tables the rules of 18.3 and 18.4 need (`crates/ignite-check`).**
   - Imports: `is_builtin_name` is `BUILTIN_TYPES || is_builtin_variant || BUILTIN_FUNCTIONS`.
     `Pipe` (only a static owner) and `ByteStream` (only in the new `BUILTIN_INTERFACES`)
     therefore fail `import std.net.{ByteStream, Pipe}` with E031.
   - E004: the declaration check consults `BUILTIN_TYPES`, `BUILTIN_FUNCTIONS` and static owners.
     Declaring `ByteStream` is not E004 unless it also consults `BUILTIN_INTERFACES`.
   - `received_batches` must join `BUILTIN_METHODS` (else it resolves as UFCS) and
     `EFFECTFUL_METHODS` (else no `read` effect is required).
   - `("Pipe", "pair", 0)` must join `BUILTIN_STATIC_ARITIES`.
   - `PROTOCOL_ROLES: &[(&str, usize)]` is a public const and cannot express "2 or 3" (D12). The
     frozen-API rule is "add, do not rename", so name the addition, for example a new
     `pub const SERVE_ARITIES: &[usize] = &[2, 3]`, and keep the old const.
   - `ignite-types`: `ModuleTypes::query_snapshot` keeps only records, enums, defaults and
     generics. Its doc comment says a new retained query must extend the snapshot and the
     ownership regression. `TypedProgram::bound` is such a query, so add `bounds` to the
     snapshot.
   - Required change: list these in 18.13.

5. **Should-fix. The Rust call form in 18.10 does not compile for borrowed parameters.**
   - Evidence: the generator passes non-role parameters by reference, for example
     `identity<T: …>(value: &T)` in `tests/golden/generics.rs` and `relay(stream:
     &ignite_io::TcpStream, …)` in `modules.rs`.
   - `ignite_io::ByteStream::read(&stream, n)` with `stream: &S` passes `&&S`. `Self` is then
     inferred as `&S`, and the call is E0277 (no impl for `&S`). Rust applies no deref coercion
     when the trait's `Self` is an inference variable.
   - Required change: emit `<S as ignite_io::ByteStream>::read(&stream, n)`. The expected type
     `&S` is then known, so `&&S` coerces. Change the example in 18.10 to match.

6. **Should-fix. D6's unbounded queue conflicts with invariant 5 without saying so.**
   - Evidence: `docs/vision.md` invariant 5 reads "No silent loss: bounded queues, …". 18.15 D6
     makes the pipe's receive queue unbounded. The Sources list of section 18 cites invariants
     6, 7 and 9, not 5.
   - Required change: cite invariant 5 in D6 and record the exception and its scope. The scope is
     test-only by E051. D16 compiles the queue into every binary, so the exception also needs a
     reason why that is acceptable.

7. **Should-fix. The baseline commit `c33442a` is stale for rows 15 and 18.18.**
   - Evidence: `main` is at `340f782`. Since `c33442a` it adds `tools/ignite-perf` to the
     workspace: `Cargo.toml` +1 line, `Cargo.lock` +9 lines, a new `[[package]]`.
   - Consequences:
     - Once the slice branch takes `main`, row 15's `git diff c33442a -- Cargo.lock` shows a new
       `[[package]]` that this slice did not add, so the row fails.
     - 18.18's before tree (`git archive c33442a`) lacks a workspace member that the after tree
       has, which skews fmt, clippy and test.
   - Required change: rebase the contract branch on `main`, or define the base as the merge base
     at close. Set rows 9 and 15 and 18.18's before tree from that base.

### Notes

8. **Note. The 18.2 instance-naming citation (and row 11) points at the wrong function.**
   - Evidence: an instance is named by `instance_symbol` in `crates/ignite-llvm/src/lower.rs`,
     which joins `mangle` of each argument. The cited lines `layout.rs:339`, `:571`, `:588`
     and `:601` are layout and `mangle` only.
   - Required change: cite `instance_symbol`, and say "named by `instance_symbol`" in row 11.

9. **Note. E041 and `can_hold_handle` are misattributed (18.2 row on methods, 18.3 Effects).**
   - E041 on a type parameter comes from `ignite-types`: `builtins::method` has no arm for
     `Ty::Param`. The fixture is `crates/ignite-types/tests/fixtures/e041_param_method`.
     `ignite-check`'s `builtin_applies` only decides builtin versus UFCS.
   - `can_hold_handle` already returns `true` for every type parameter
     (`self.type_params.contains(name)`), and for unknown names. Only `builtin_applies` needs the
     bounded case.
   - Required change: correct both statements.

10. **Note. "Exactly these four methods" is not true (18.3 Rules).**
    - Evidence: `builtins::method` gives `copy` to every receiver except `Never`, including
      `Ty::Param`.
    - Required change: say "these four and `copy`".

11. **Note. "No TCP code path changes" (18.8) cannot hold literally.**
    - Evidence: `flush()` and `PENDING` (`Pending(Vec<Weak<Stream>>)`) live in
      `crates/ignite-io/src/net.rs`. Covering pipes changes that code, whether as one list or two.
    - Required change: say "no TCP behaviour changes".

12. **Note. Row 17 has no command.**
    - Required change: name a check, for example `grep -n 'ignite-codegen/0.1.0-2'
      crates/ignite/src/apply/plan.rs`, or the existing
      `apply_admission::` assertion on `CODEGEN_REVISION`.

13. **Note. Row 16 undercounts the todo model's diff.**
    - Evidence: a defaulted field appears in three places, as `sessions` does in
      `terminal.model.json`: in `defaults`, as a `sources` entry pointing into
      `std/protocol/http.ignite`, and in `properties`.
    - Required change: list all three for `limits`.

14. **Note. Row 3 does not match `terminal.rs`.**
    - Evidence: `crates/ignite/tests/terminal.rs` tests `frobnicate x`, not `foo`.
    - Required change: either quote that table or state the `foo` bytes
      (`unknown command: foo\r\nterminal> `).

15. **Note. Edits to earlier sections have no owner (18.16).**
    - Evidence: the slice changes the grammar of section 3 (fn type parameters), the role
      table and E030 rule of 4.2, the substrate surface of section 7 (`PipeStream`, the trait),
      and the generics rules of 11.1. U1 owns only the section 8 rows.
    - Required change: assign these edits, or state that 18 supersedes those sections for
      fns and roles.

16. **Note. The order of 18.16 lets U3 and U4 start before U2 has merged.**
    - Evidence: their first tests need U2. The golden builds with cargo against
      `ignite_io::ByteStream`, and the IR links against `ignite_rt_pipe_*`.
    - Required change: say "start when U1 has merged; close after U2 has merged", or start them
      after U1 and U2.

17. **Note. Two deviations from `protocols.md` §3.6 are missing from 18.14.**
    - §3.6 decides that "Roles read limits only from this record" and rejects module constants.
    - D17 keeps the constant `MAXIMUM_SUBNEGOTIATION` for the client role, and `READ_SIZE` and
      `MAXIMUM_LENGTH_DIGITS` stay constants.
    - D17 and 18.5 give reasons, but 18.14 lists only `maximum_line`.
    - Required change: add an 18.14 item for these deviations.

18. **Note. Vision and slice records the slice makes stale, which U5 does not own.**
    - Invariant 3's list of handle types gains `PipeStream`.
    - §4.1's row `14+ | B, C, D, E, G, H` becomes slice 14 = B.
    - X16 / §10's bound "`std/protocol/telnet.ignite` at most 400 lines excluding tests" is
      already exceeded: about 655 non-test lines at `c33442a`, an awk count, so approximate.
      The slice grows the file further.
    - Required change: add `docs/vision.md` to U5's files for the first two, and record the
      third in 18.14.

19. **Note. No planning-store artifact exists for the slice.**
    - Evidence: `AGENTS.md` says live work from slice 13 on is tracked in `.engineering/`.
      `.engineering/planning/` holds only the slice-13 epic and stories.
    - Required change: name the epic and stories in 18.16, or say who creates them before
      dispatch.

20. **Note. Some 18.2 rows cite only line numbers.**
    - Evidence: vision T3 says to cite symbols. These rows give no symbol: E051
      (`ignite-check/src/lib.rs:3156`), E004 (`:931`), the E030 message (`:1346`), the
      `ignite-io` flush points (`lib.rs:144`, `:248`, `:258`, `task.rs:132`), the `ignite-rt`
      table (`net.rs:57`) and `telnet.ignite:118` and `:89`.
    - The cited lines were spot-checked against `c33442a` and are accurate.
    - Required change: add the enclosing symbol to each of these rows.

21. **Note. Small gaps in 18.12 and 18.7.**
    - 18.12 makes `session` generic, but `session` also needs a `limits` parameter (or the
      subnegotiation limit), because `decode_within` reads `limits.maximum_subnegotiation`. The
      HTTP list says "and `limits`"; the Telnet list does not.
    - `e051_pipe_pair` needs a message. Today's E051 text names only `Store.new()`.

22. **Note. When a pipe end is dropped differs between the targets, so inline tests should not
    rely on it (18.4, 18.11).**
    - Rust: a spawn clones its arguments (`let __spawn_0 = stream.clone()` in
      `tests/golden/generics.rs`), so the spawner's binding keeps the end alive until its scope
      ends.
    - LLVM: an end is released only at the end of the task it was moved into (`spawn_with`). An
      end held by a test fn lives until the process exits.
    - In both cases a peer sees `Eof` only after an explicit `close`. The helpers of 18.12 close,
      because `Telnet.serve` and `Http.serve` close.
    - Required change: state this in 18.4 so that new pipe tests close explicitly.

## Verified, no finding

- Every symbol named in 18.2 exists at `c33442a`, and the line numbers spot-checked
  (30 citations) match.
  - Instances of Rust's missing turbofish on plain calls and spawns: `fn_call` and `spawn`. It
    is present only in `fn_value`, `assoc_call`, associated fn values and record defaults.
  - `PARAM_BOUNDS`.
  - The TCP-only check in `spawn_with`.
  - `method_effect`.
  - The `Telnet.serve`/`serve_with` split.
  - `SessionLimits` and the HTTP constants.
  - `CODEGEN_REVISION = "ignite-codegen/0.1.0-1"`.
- The codes are free. Nothing emits E090 to E099, and section 8 defines none of them. E096 keeps
  `protocols.md` §3.3's meaning, and P4's "stream-kind mismatch is E096" stays consistent. E099
  is inside T4's protocols range.
- Builtin-name collisions: the only declaration of `Pipe`, `PipeStream` or `ByteStream` in the
  repository is `e016_effects`' `protocol Pipe`, which 18.7 renames.
- Layering: `Pipe`, `PipeStream` and `ByteStream` in `std.net` (base) and in the substrate name
  no application protocol. Rule 5 holds.
- Every `dyn` in the generator's source and goldens is `dyn Fn`, so row 10 can pass.
- No e2e fixture or golden pins positions in `std/protocol/*.ignite` other than
  `terminal.model.json`. No test hard-codes a model or plan digest. No code outside the two std
  files names `serve_with` or the five HTTP constants.
- Row 7's single batch follows from the current code.
  - `read_head` and `read_body` read only when the buffer lacks a head or body.
  - `session` writes three replies without reading.
  - `serve`'s `close` delivers them, and a delivery is atomic.
- `protocols.md` §7 row P1: every acceptance item maps to an 18.0 row. The item "the Rust golden
  shows `Telnet::serve::<ignite_io::TcpStream>` and no `dyn`" is soundly reinterpreted by D9,
  D10 and 18.14 items 2 and 3. `ByteStream.info` (§1.2) and the `HttpLimits` split (§3.6) are
  deferred with reasons (18.14 item 8, D5).
- Shape: section 18 follows sections 16 and 17. It has an acceptance matrix with commands and
  owners, current state, rules, typed API, diagnostics, work split, decisions, placeholder arms
  and gate time. Its unit table has no concurrent file overlap: U1 and U2 touch disjoint
  crates, U3 and U4 touch disjoint generators, and U1's arms in the generators precede them.

## Resolution

All changes are in `docs/slice.md` section 18 and `PLAN.md`, base `340f782`.

1. Fixed: the transcript is an inline test at the end of `examples/terminal/commands.ignite`
   (D14, 18.12, row 3, U5's files and its AGENTS layout row). It must not contain `lines =
   ["ready"]`; 18.2 and 18.12 name the editing tests.
2. Fixed: row 6 says the count includes the IAC of `IAC SE`, and states both cases (15 bytes
   answered, 16 bytes end the session).
3. Fixed: row 5 gives the exact bytes, `EF BF BD EF BF BD 0D 0A` between the prompts, then
   `line too long\r\n` and end of stream; `maximum_line = 2` stays.
4. Fixed: 18.13 lists `is_builtin_name`, E004 over `BUILTIN_INTERFACES`, `received_batches` in
   `BUILTIN_METHODS` and `EFFECTFUL_METHODS`, `("Pipe", "pair", 0)`, the new `SERVE_ARITIES`
   beside an unchanged `PROTOCOL_ROLES`, and `bounds` in `query_snapshot`.
5. Fixed: 18.10 emits `<S as ignite_io::ByteStream>::read(&stream, n)` and says why.
6. Fixed by bounding the queue: `PIPE_CAPACITY` 16 MiB; an exceeding delivery fails whole with
   the kept error `pipe queue full` and never blocks (D6, 18.4, 18.8, a new `ignite-io` test).
   Invariant 5 is cited, so no exception is needed.
7. Fixed: base `340f782` in the preamble, rows 9 and 15 and 18.18; both trees include
   `tools/ignite-perf`; the merge base at close applies if `main` moves.
8. Fixed: 18.2, row 11 and 18.11 cite `instance_symbol`.
9. Fixed: 18.2 and 18.3 attribute E041 to `builtins::method` and say `can_hold_handle` needs no
   change.
10. Fixed: "these four methods and `copy`".
11. Fixed: 18.8 says "no TCP behaviour changes" and names the `flush()`/`PENDING` change.
12. Fixed: row 17 has the `grep` and `apply_admission::`.
13. Fixed: row 16 lists `limits` in `defaults`, `sources` and `properties`.
14. Fixed: row 3 uses `frobnicate x`, matching `terminal.rs`.
15. Fixed: section 18 states that it supersedes those sections for fns and roles; U1 owns the
    forward references in sections 3, 4.2 and 11.1 (with section 8's rows), U2 the one in
    section 7.
16. Fixed: U3 and U4 start when U1 has merged and close after U2 has merged.
17. Fixed: 18.14 item 13 records the three kept constants as deviations from §3.6.
18. Fixed: `docs/vision.md` (invariant 3, §4.1 row) joins U5's files via 18.14 item 15; the
    telnet line bound is recorded in 18.14 item 14.
19. Fixed: 18.16 says the coordinator creates `epic:slice-14-protocols-p1` and one story per
    unit before dispatch; `PLAN.md` names the epic.
20. Fixed: every 18.2 row now names its enclosing symbol.
21. Fixed: `session` takes `limits: SessionLimits` (18.12); the E051 message for `Pipe.pair()`
    is in 18.7.
22. Fixed: 18.4 states the drop-timing difference and that pipe tests close explicitly.
