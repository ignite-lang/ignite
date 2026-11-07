---
format: aep.planning-md/3
id: review-result:adversary-node-ids-pass-1
kind: review-result
status: active
title: Adversary, stable-node-ids, pass 1
relations:
- reviews: story:stable-node-ids
revision: 1
---
unit: story:stable-node-ids, worktree node-ids at a2e04f97 (base 64585854), plus two new test files, nothing committed
verdict: INFEASIBLE
cases: executed 229→229 (`ignite-syntax` + `ignite-types` main binaries, `--skip node_ids_adversary` 155+74; with mine and `--skip stack_100_ifs` 164+74), red 2
origin: introduced 1 / pre-existing 1 / undecided 0
wrote-outside-worktree: 1 path (scratch dir, large build output deleted)
needs-coordinator: `stack_100_ifs` overflows the stack and aborts the whole `ignite-syntax` test binary, which hides the other cases in that binary. Decide whether to keep it or drop it before any gate run.

1. `git --no-pager diff --stat`, plus the untracked file:
```
 crates/ignite-syntax/tests/main.rs | 1 +
 1 file changed, 1 insertion(+)
?? crates/ignite-syntax/tests/node_ids_adversary.rs
```
Both are test files. I changed no implementation file.

2. Cases added, all in `crates/ignite-syntax/tests/node_ids_adversary.rs`:
   - `distinct_paths_never_share_node_ids` (line 38). It searches for two sibling paths whose `FileId::of_path` hashes are equal, parses a different program under each, and asserts that the two sets of node ids do not overlap. **Red now.** Output from running it alone:
     ```
     `/work/project/examples/app/m512789.ignite` and `/work/project/examples/app/m749192.ignite` share node ids ["id: NodeId { file: FileId(4145756289), index: 1 }", "id: NodeId { file: FileId(4145756289), index: 0 }"]
     test result: FAILED. 0 passed; 1 failed; ... finished in 2.13s
     ```
   - `stack_100_{parens,lists,calls,ifs,matches,records,closures,negations}`. Each parses 100 nesting levels of one form on a 2 MiB thread. I ran each alone on head, and ran copies on the base export:
     - head: `ifs` overflows the stack, the other 7 pass.
     - base: `ifs` and `records` overflow, the other 6 pass.
     - So the change does not make stack use worse; `records` got better. `stack_100_ifs` is **red now** and pre-existing: `thread '<unknown>' has overflowed its stack / fatal runtime error: stack overflow, aborting`.
   - `every_repository_file_has_distinct_node_ids` (line 140). It parses every `.ignite` file under `std/`, `examples/` and `crates/` and asserts that no node id appears twice in a file. **Green.** On a 2 MiB thread it overflowed (debug output of a deep fixture), so it now runs on a 256 MiB thread.

3. Suite run, after the cases existed:
   - `cargo test -p ignite-syntax -p ignite-types --no-fail-fast`: exit 101. The syntax binary aborted with `SIGABRT` on `stack_100_ifs`. The types binary passed: `74 passed; 0 failed`.
   - Same command with `-- --skip node_ids_adversary`: exit 0, `155 passed` and `74 passed`.
   - Syntax binary with `--skip stack_100_ifs`: `163 passed; 1 failed` (the one failure is `distinct_paths_never_share_node_ids`).

4. Judgement findings (tree a2e04f97):
   - `crates/ignite-syntax/src/ast.rs:21`, INFEASIBLE, introduced. `FileId` is a 32-bit FNV-1a hash of the path, so two different files can get the same `FileId`, and then the same node ids from index 0. Nothing detects this.
     - What reaches it today: nothing harmful. Every typed table is per module, and every lookup I traced (evaluator `Cx`, rustgen, llvm, `typed_json`) uses the module's own `TypedProgram`. A collision among about 50 files in one build has a chance of roughly 3e-7.
     - The risk is the story's goal: a cache or serialised form keyed by `NodeId` across many files would mix up nodes. A wider or collision-checked file id would close it, for example a 64-bit hash, or an interned file table that numbers files per compilation.
   - `crates/ignite/src/lib.rs:539` and `:947`, INFEASIBLE, introduced, severity note. `FileId` is hashed from `file.display()` as the user typed it, so the id depends on the working directory. `a.ignite` in two different directories gets one id; one file reached through `x/a.ignite`, `./x/a.ignite` or an absolute path gets three. The standard library is canonicalised, programs and `--stdin-for` paths are not. Nothing reads `FileId` across runs yet. The doc line "the same path gives the same id in every run, so ids can be cached" holds only for the exact same string.
   - `crates/ignite-syntax/src/parser.rs:2567`, INFEASIBLE, pre-existing. 100 nested `if` expressions overflow a 2 MiB thread on base and on head. The CLI runs on a larger stack (`crates/ignite/src/main.rs:479`), so a user does not hit it. The `primary` doc comment claims only that 100 levels of `(` fit.

5. Attacked and could not break:
   - Duplicate ids within one parse: none in any repository file, including byte strings, `l[i] = v`, nested interpolation and field shorthand.
   - Fresh ids against parser ids: they live in separate file spaces (`SYNTHETIC` versus a real hash, where a hash of `u32::MAX` is moved to `u32::MAX - 1`).
   - Subtrees cloned or synthesised before typing: the checker's synthetic `Type` and `Path` nodes get fresh ids, and no backend builds an `Expr`.
   - Typer tables keyed by `Span`: these spans now include the file. Every caller looks up with a node's own span.
   - Address keys left in `ignite-types`: none. `ignite-check` still keys `tail_roots` by address, but that table never leaves the checker.
   - Outputs identical between base and head, byte for byte:
     - `check --json` and `check --json --stdin-for <absolute path>` for all 68 std and example files.
     - `check --stdin-for` of an edited `std/control.ignite`.
     - `emit --typed-json` (sample, todo, `std.sli`), `plan` and `explain` of the design file.
     - `emit` Rust for terminal, todo, design, `control_trace` and reader.
     - The LLVM `.ll` files of terminal, todo and `control_trace`.
   - Speed, 15 interleaved pairs at load about 35 (debug builds), mean seconds base against head:
     - `check` design file: 0.2947 / 0.2951
     - `plan` design file: 0.3634 / 0.3606
     - `check` todo: 0.4758 / 0.4802
   - The unit's clone test can fail: it looks up the original program's nodes in the clone's typing, which would return nothing if the tables were still keyed by address.

6. Paths written outside the worktree: `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-node-ids/adversary/` (2 MB left: build, suite and perf logs, `perf.sh`, `bad.ignite`, `out/`). I created and then deleted under it: `base/` (an export of 64585854), `base-target/` (707 MB), `head-target/` (731 MB) and `xdg/` (runtime cache).

7.
```findings
- file: crates/ignite-syntax/src/ast.rs
  line: 21
  category: property
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: FileId is a 32-bit FNV-1a path hash with no collision check, so two distinct files can share every node id (red in node_ids_adversary::distinct_paths_never_share_node_ids); harmless while typed tables stay per module, unsafe for the cross-file cache the story targets
- file: crates/ignite/src/lib.rs
  line: 539
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: FileId hashes the uncanonicalised display path, so one file reached by different paths gets different ids and same-named files in different working directories get one id
- file: crates/ignite-syntax/src/parser.rs
  line: 2567
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: pre-existing
  message: 100 nested if-expressions overflow a 2 MiB thread on base and head (node_ids_adversary::stack_100_ifs aborts the test binary); the CLI's large stack means no user reaches it
```
