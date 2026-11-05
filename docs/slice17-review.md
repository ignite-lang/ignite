# Review of `docs/slice.md` section 21 (slice 17)

Independent review, 2026-11-04, by a reviewer that had not seen the author's reasoning, of
`feat/slice17-processes-files` at `212d2eb` (the probe results were filled in during the
review, after `82ab6b3`), against `docs/vision.md` §3, §6 and §7 and the stories
`story:process-spawn`, `story:file-system`, `story:program-arguments`, `story:random-bytes`
and `epic:playground-substrate`. Code was read at base `c539392`, nothing was built. The
resolution of each finding is in `docs/slice.md` 21.17.

## Blockers

**R1. Blocker. Rows 8, 15 and 21: the E016 text cannot occur.**
- The rows require "exactly `E016 effect missing from requires: files.write`".
- The checker's E016 text is ``missing effect `X` for `Y`; add it to the `requires` of `F` ``.
  Examples are `crates/ignite-check/tests/fixtures/e016_adv_*.expected` and section 5.1.
- Section 21.9 changes no E016 text, so a `.expected` file cannot match these rows.
- Resolution: quote the real form, for example ``missing effect `files.write` for
  `files.write_file`; add it to the `requires` of `<fn>` ``, and the same shape in rows 15 and
  21.

**R2. Blocker. Row 17: no API gives a child's pid.**
- `orphans.ignite` "starts `/bin/sleep 30`, prints the child's pid".
- Section 21.5 gives `ChildProcess` only `wait` and `kill`, and `ChildExit` has no pid.
- Output is only available after `wait`, because streaming is out of scope (21.2).
- Resolution: add a pure `child.id() -> Int`, or have the child write `$$` to a pidfile named
  by a `Setting` (`/bin/sh -c 'echo $$ > F; exec sleep 30'`) and let the test read the file.

**R3. Blocker. Row 11 and section 21.5: on LLVM, `wait` and `kill` give different results
from Rust.**
- Section 21.5 says "`wait` frees a child's slot". A later lookup of a freed handle returns
  `handle N was released` (`crates/ignite-rt/src/net.rs`, entry lookup).
- So a second `wait` on LLVM is not "child already waited", and `kill` after `wait` is `Err`
  where 21.5 promises `Ok`.
- On Rust the `Arc` keeps the object, so the targets diverge. This breaks invariant 7 and makes
  row 11 fail on LLVM.
- Resolution: state that the `ignite-rt` child adapter maps a released `Child` handle to
  `Err "child already waited"` for `wait` and to `Ok` for `kill`. The generation check makes
  this unambiguous. Add the `kill`-after-`wait` case to row 11.

**R4. Blocker. Row 9, and every inline-test row, would get E042 by section 21.3's own rule.**
- Section 21.3 says a receiver whose type `ignite-check` cannot see is E042.
- `static_type` (`crates/ignite-check/src/lib.rs`) returns `None` for:
  - a path-static result, so row 9's literal `Launcher.new().execute(..)` is unseen;
  - an unannotated `let` bound from `Directory.temporary()?`, `Entropy.new()` or
    `tools.start(..)?`.
- The typer's `effect_site` then reports E042 "annotate the binding". Existing tests annotate
  for the same reason: `let store: Store<Todo> = Store.new()` in `slice9/regions.ignite`.
- Resolution (either one):
  - Extend `static_type` with arms for the four test-only statics and for `Launcher.start`, and
    say so in 21.3 and U1's scope.
  - Or rewrite the rows with annotated bindings, and add a probe p5 (`let s = Store.new();
    s.put(..)` in a test) to confirm the current behaviour.

## Major

**R5. Major. Section 21.3: `Directory.absolute_path()` is marked pure.**
- Its value is fixed only at launch (flag value, then canonicalised).
- The reason 21.3 gives for making `Setting.value` effectful ("keeps it out of compile-time
  evaluation, E116") applies here too.
- As written, `Invocation { directory = files.absolute_path() }` in a property expression is
  pure, but `ignite-eval` cannot compute it. Design §3.5 says compile-time evaluation "accepts
  only pure functions".
- Resolution: give it op `read`.

**R6. Major. Section 21.12 W3: two units edit the same files without the split saying so.**
- `ignite_io::exit` and `run` live in `crates/ignite-io/src/lib.rs`. U3 owns "the kill at
  orderly exit in `ignite_io::exit`, `run`", yet the split says "`lib.rs` of both crates is
  U1's and none of them edits it".
- U3 moves `run_child` out of `crates/ignite-io/src/test.rs`. U2's `Directory.temporary()`
  cleanup "when the test child ends" lands in `test.rs` or `lib.rs`. Both are W3 units.
- `args` is a module inside `lib.rs`, so U4 also edits it (W4, which is fine, but it should be
  stated).
- The LLVM flag adapter beside `ignite_rt_args_port` (`crates/ignite-rt/src/process.rs`) has no
  owner.
- Resolution: name these files as shared in W3 with an order, or have U1 add the hook points
  (an exit-hook registry in `lib.rs`, a cleanup hook in `test.rs`).

**R7. Major. F11, row 29 and the W3 note on `Cargo.toml` misstate the `nix` features.**
- The workspace dependency is `nix = { version = "0.31", features = ["term", "fs"] }` (root
  `Cargo.toml`), so `fs` is already on for `ignite-io`.
- In nix 0.31.3, `signal = ["process"]`, so `process` is already on too.
- `Dir` needs feature `dir`, which nobody adds.
- `cargo tree -e normal --prefix none` lists crates, not features, so row 29 cannot observe a
  feature change.
- Resolution: features to add are `dir` and `poll` outside `runner`. Observe them with
  `cargo tree -p ignite-io -e features`. Drop the claim that U2 and U3 rebase one line.

**R8. Major. Row 23: the command does not run its first fixture.**
- `e051_capability_outside_state` is an `ignite-check` fixture, but the command is
  `cargo test -p ignite-types --test main fixtures::`.
- Also, ignite-types fixtures run against the fixture standard library, which has no
  `std/protocol` (section 8 note on `valid_system`). So `e017_ambient_in_handler` needs a
  locally declared `HttpApp`-shaped type.
- Resolution: list both commands and state the local declaration.

**R9. Major. Vision §3 invariant 3 and section 5.1's handle list go stale, which affects
slice 16.**
- Invariant 3 enumerates the handle types (`TcpStream`, `PipeStream`, `TcpListener`,
  `Store<T>`, …). Section 5.1's roots rule lists `TcpStream`, `TcpListener`, `Store`, `Stdin`,
  `Stdout`.
- Section 21 adds five handle types and has no stale point for either list.
- `origin/feat/slice16-language-gaps` §20.3.1 leaves "a handle type (invariant 3)" untracked by
  the move analysis. When it merges, the new five must be in that predicate or they will be
  treated as aggregates.
- Resolution: add S-rows for invariant 3 (U6) and the 5.1 list (U1), plus a merge note for
  slice 16's move predicate.

**R10. Major. Section 21.5 and row 14: "only descriptors 0 to 2" has no mechanism.**
- F9 relies on close-on-exec, but that only covers descriptors the runtime opens itself.
- Any non-CLOEXEC descriptor the server inherited at launch passes through Rust's `Command` to
  the child. Possible sources are a shell `exec 5>`, the test harness, or a pipe-based cargo
  jobserver (inferred, not verified).
- So row 14 depends on how the server was launched.
- Resolution: close everything from 3 upward in the child (`close_range(3, ~0, 0)` via
  `pre_exec`, which needs an `unsafe` block in `ignite-io`), or weaken the promise to "no
  descriptor the runtime opened". Have the row's launcher deliberately leak an fd 5 and assert
  it is absent.

**R11. Major. Children are orphaned on common non-orderly exits.**
- The drain's second SIGTERM ends the process with `raise` and `std::process::exit`
  (`drain::install`), without killing children.
- `ignite run`'s rollout deadline kills the server with SIGKILL.
- In both cases the per-child watcher dies, so timeouts stop being enforced and the children
  run without bound.
- Section 21.15 defers `PR_SET_PDEATHSIG` because it "needs the watcher to start the child".
  But D7 already gives one watcher thread per child, so that obstacle is gone.
- Resolution:
  - Kill all live child groups in the second-SIGTERM path.
  - Have the watcher spawn its child with PDEATHSIG = SIGKILL (this reaches the direct child
    only).
  - Add both to row 17.

**R12. Major. LLVM target: `start` without `wait` is unbounded.**
- `maximum_children` counts only "from start to reaping".
- An unwaited `ChildProcess` keeps its table slot and up to twice `output_limit` until exit
  (21.5, 21.15). A handler calling `start` in a loop grows without bound, against invariant 5.
- Resolution: count a started child against `maximum_children` until it is waited or its
  handle is released. That turns the leak into `too many children`.

**R13. Major. Security: `read_file` and `write_file` on a non-regular file.**
- A FIFO inside the root blocks `open(O_RDONLY)` forever and hangs the task.
- Children are unconfined (21.2), and the playground runs user programs, so a launched program
  can `mkfifo` inside the root.
- Resolution: open with `O_NONBLOCK`, `fstat`, refuse anything that is not `S_ISREG`
  (`path <p>: not a regular file`), then clear `O_NONBLOCK`. Add a FIFO case to row 3.

**R14. Major. Security: killing the group after reaping risks hitting an unrelated group.**
- Section 21.5's watcher "reaps the child, then kills what is left of the group".
- Once the leader is reaped and the group is empty, its pid (which is the pgid) can be reused
  by a new group leader, such as this launcher's next `process_group(0)` child. `killpg` would
  then kill that group.
- `kill()` after the child ended has the same hazard.
- Resolution: `waitid(WEXITED | WNOWAIT)`, then `killpg`, then reap. Make `kill` a no-op once
  the child is reaped.

**R15. Major. Row 4 cannot pass as written.**
- Swapping a non-empty directory for a symlink is not atomic with plain `rename`. In the
  window, `read_file("d/inner")` returns ENOENT, which is neither "the inside bytes" nor "the
  symlink error".
- With `O_NOFOLLOW | O_DIRECTORY`, the kernel's errno (ELOOP or ENOTDIR) must be mapped to
  `symbolic link <component>`.
- Resolution: require `renameat2(RENAME_EXCHANGE)` in the test, or require "never the outside
  bytes; any `Err`". Specify the errno mapping.

**R16. Major. No generator goldens, against invariant 7 and AGENTS.md.**
- New resource lowering, flag reading in the generated `main` and method lowering on both
  backends have no rustgen or LLVM golden row. Slice 14 had rows 14 and 17.
- The rows also do not require `every_type_code_has_a_fixture` to list `e140` and `e141`.
- Resolution: add a `golden::capabilities` row per backend, with a cargo build and a clang
  compile, and the code-list update.

**R17. Major. E142 coverage is inconsistent.**
- Section 21.9 and section 8 say E142 comes from `ignite run` and `ignite plan --for-apply`.
  Row 26 tests only `run`, and with `studio.ignite`, not the fixture
  `e142_setting_without_default` that 21.9 names.
- Resolution: add a `plan --for-apply` case and use the named fixture.

## Minor

**R18. Minor. F5, D3 and row 30 give a stale reason.**
- They cite `list`, `run` and `entries`, but no new method has those names.
- The real collisions are `start` (a fn in
  `crates/ignite-check/tests/fixtures/valid_protocol.ignite` and
  `e013_spawn_not_effectful_call.ignite`), `remove` (already a `Map` and `Store` builtin),
  `value`, `wait`, `kill` and `execute`.
- Resolution: row 30 should check those names, plus the ignite-check fixtures.

**R19. Minor. Misattributed quote.** Section 21.3 quotes "authority over a listener is
authority over its connections" as section 5.1. It is vision §6 L4, and 5.1 does not state the
rule. Cite L4, or add the rule to 5.1.

**R20. Minor. Row 18's stderr text.** `ignite_io::run` logs `error: {e}`, so stderr is
`error: --tool: required argument missing`. The same applies to `directory <path>: …` in
section 21.4.

**R21. Minor. Row 6 and section 21.4: the path in the root error.** `<p>` for
`remove_tree(".")` would be `.`, but row 6 expects `path : the root itself`. State whether
`<p>` is normalised.

**R22. Minor. Row 15 needs a `requires` clause.** A fn without `requires` infers its effects
and never gets E016 (section 5). State that the fixture fn declares `requires []`. This is also
how the story's "without declaring the effect" is met.

**R23. Minor. Vision §3's L0 substrate row** has no files, children or entropy. Add an S-row
for U6.

**R24. Minor. Section 21.6 and Q2: working directory under apply.** Under `ignite apply` the
executable inherits `ignite host`'s working directory, not "the run's". A relative
`Directory.path` then depends on where the host was started, which a saved plan does not
capture.

**R25. Minor. Section 21.6: flag scanning.** Scanning each flag independently with the
`--port` rule means `--mode --tool /x` gives mode `--tool` and also tool `/x`. Refuse a value
that starts with `--` (`needs a value`).

**R26. Minor. Unbounded sizes.**
- `Invocation.output_limit` has no maximum (times 1,024 children).
- `read_directory` has no entry cap.
- `remove_tree` recursion depth holds one fd per level, so a deep tree hits EMFILE or exhausts
  the stack.
- Resolution: set bounds and give each one an error text.

**R27. Minor. Row 26 writes into the repository.** `ignite run studio_defaults.ignite` with
`create = true` makes `workspace/` in the test's working directory (`crates/ignite`). Run it in
a temporary working directory.

**R28. Minor. F12 and U3: test-runner regression.** If `run_child` moves onto
`ignite_io::child` with `process_group(0)`, test children leave the terminal's foreground
group, so Ctrl-C no longer reaches them. Keep the runner's group behaviour as a parameter.

**R29. Minor. Section 21.8: `Directory.temporary()` cleanup.** A test child killed by the
runner's timeout (SIGKILL) cannot clean up. Specify cleanup on the runner side: a per-test
parent directory removed after the child is reaped.

**R30. Minor. Section 21.5: "default dispositions" is overstated.** Rust's `Command` resets
the signal mask and SIGPIPE only. Other ignored dispositions are inherited (for example SIGINT
ignored under `nohup` or a background shell job). Reword, or reset them explicitly.

**R31. Minor. Rows 10 and 17: zombies.** `pgrep` matches zombies, and killed orphans are
reaped by a subreaper or init. Accept "absent or state Z" from `/proc/<pid>/stat`.

**R32. Minor. U1's ownership list is incomplete.**
- It omits `Ty` in `crates/ignite-types/src/lib.rs` and `effect_site` and E140 in `typer.rs`.
- It omits `Builtin` and value arms in `crates/ignite-eval/src/{eval,value}.rs`, the `Handle`
  enum in `crates/ignite-rt/src/net.rs`, and LLVM `region.rs`, `layout.rs` and `glue.rs`.
- U4 changes backend generation in W4 but does not bump `CODEGEN_REVISION`. Row 32 should also
  run the `apply_admission::` tests, as slice 14's row 17 did.

**R33. Minor. Row 31's command cannot observe its claim.** `cargo test` cannot show "no other
expected file changes". Use `git diff --stat c539392 -- '**/*.expected'`.

**R34. Minor. Environment and redaction (design §9.3).**
- `inherit_environment = true` hands the server's environment, including any tokens, to every
  child.
- The model prints a `Setting`'s `default` unredacted.
- Resolution: document both, or default `inherit_environment` to false and pass `PATH`.

**R35. Minor. Row 3: symlink refusal is tested only through Rust unit tests in `ignite-io`.**
The LLVM adapter path is not exercised, and the story says "on both targets". Add one inline
case that makes a link via `Launcher` (`ln -s`).

**R36. Minor. D2: the op `read` is shared.** `Directory`, `Entropy` and `Setting` all use op
`read`, so `requires [b.read]` on a record holding all three grants all three. D2's separation
holds only per field. Consider distinct op names, for example `entropy.draw`.

**R37. Minor. Scope: `Setting` and `story:program-arguments`.** The story excludes "an
argument-parsing library beyond what `std.cli` declares". `Setting` is a new mechanism outside
`std.cli`, so the steering session should confirm D10.

## Checked and consistent

- No code collision: E140 to E149 are unused on `main`, on `origin/feat/slice15-protocols-p2`
  and on `origin/feat/slice16-language-gaps`.
- All cited symbols exist: `store_op`, `store_effect`, `EFFECTFUL_METHODS`, `can_hold_handle`,
  `AMBIENT_HEADS`, `resource`, `lower_resources`, `run_child`, `drain::install`,
  `args::port_from_args`, `CODEGEN_REVISION`.
- No parameter named `clock` exists in `std/`, `examples/` or the fixtures, so row 35 breaks
  nothing.
- Every acceptance line of the four stories maps to a row, apart from R1, R8, R17 and R35.
