---
format: aep.planning-md/3
id: review-result:adversary-node-ids-pass-2
kind: review-result
status: active
title: Adversary, stable-node-ids, pass 2
relations:
- reviews: story:stable-node-ids
revision: 1
---
unit: story:stable-node-ids, worktree node-ids at 36beb23b (base 64585854) plus 2 new test files and 2 `mod` lines; nothing committed
verdict: INFEASIBLE
cases: executed 240→244 (`ignite-syntax` 164 + `ignite-types` 74 + `ignite` `file_ids::` 2 before, counted from the run after my cases existed with my files deselected; 166 + 74 + 4 after), red 4
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 path (the scratch directory, 176 KB left)
needs-coordinator: the commit removed pass 1's red case `stack_100_ifs` and did not mark it `#[ignore]`. It is still red on head: 100 nested `if` need 26.6 KB per level, about 2.6 MB, on a 2 MiB stack. Please confirm you decided to drop it.

Pass-1 finding 1 is fixed: `FileId` is now 64 bits. Finding 2 is fixed for files that exist on disk. All 4 red cases are cases I built myself; I found no workflow today that reaches them.

**1. Diff stat**
```
 crates/ignite-syntax/tests/main.rs | 1 +
 crates/ignite/tests/main.rs        | 1 +
?? crates/ignite-syntax/tests/node_ids_adversary2.rs
?? crates/ignite/tests/file_ids_adversary.rs
```
All four are test files. I changed no implementation file.

**2. Cases added.** I wrote them before running anything, then ran each file alone. All 4 are red now.

- `crates/ignite-syntax/tests/node_ids_adversary2.rs`
  - `two_sibling_paths_have_distinct_file_ids`: the two paths are a real 64-bit FNV-1a collision. A parallel rho search over the re-pinned test's path shape found it in 30 s on 12 threads.
    ```
    panicked at crates/ignite-syntax/tests/node_ids_adversary2.rs:18:5:
    assertion `left != right` failed: `/work/project/examples/app/m28f332a082b5e328.ignite` and `/work/project/examples/app/m3b4caa5b359d6895.ignite`
      left: FileId(10128729288328614347)
     right: FileId(10128729288328614347)
    ```
  - `two_sibling_files_share_no_node_id`: the same two paths, parsed:
    ```
    shared node ids ["id: NodeId { file: FileId(10128729288328614347), index: 0 }", "id: NodeId { file: FileId(10128729288328614347), index: 1 }"]
    ```
- `crates/ignite/tests/file_ids_adversary.rs`
  - `stdin_for_a_missing_file_has_one_id_for_every_spelling`: `check_text` of a file that does not exist, under 3 spellings of its path, gives 3 different ids.
    ```
    [("tests/fixtures/not-yet-saved/a.ignite", FileId(4748750029624423490)), ("./tests/fixtures/not-yet-saved/a.ignite", FileId(7494362841173166591)), ("/home/timo/.../crates/ignite/tests/fixtures/not-yet-saved/a.ignite", FileId(14135151579355647570))]
    ```
  - `two_modules_of_one_compilation_share_no_node_id`: `copy.ignite` is a symlink to `util.ignite`. `ignite::load` gives two modules, `app.util` and `app.copy`, with identical node ids.
    ```
    5 node ids shared, first: id: NodeId { file: FileId(2861894265654731612), index: 0 } in app.copy and app.util
    ```

**3. Suite run, after the cases existed**
- `cargo test -p ignite-syntax -p ignite-types --no-fail-fast`: exit 101. Syntax: `164 passed; 2 failed`. Types: `74 passed; 0 failed`.
- The same command with `-- --skip node_ids_adversary2`: exit 0, `164 passed` and `74 passed`.
- `cargo test -p ignite --test main --no-fail-fast file_ids`: exit 101, `2 passed; 2 failed` (the two failures are mine).

**4. Judgement findings** (tree 36beb23b)

- **Is the 2,000,000-path bound a fair re-pin?** No. `crates/ignite-syntax/tests/node_ids_adversary.rs:46`, INFEASIBLE, introduced, note.
  - Measured: with a 64-bit id, a search of 2M paths finds a pair with a chance of about 1e-7. When it finds none, the test returns without asserting anything, so it cannot fail, and it costs a 2M-entry HashMap every run.
  - A collision is not a search problem: one took 30 s to find (case 1 above).
  - What reaches it: nothing by accident. The doc's 3e-8 figure is honest for random paths. But a file chosen to collide can make two files share every node id.
  - Fix: pin the found pair as a fixture. If a collision must be impossible rather than unlikely, number files per compilation with an interned table.
- **Symlinked modules.** `crates/ignite/src/lib.rs:545`, INFEASIBLE, introduced, note.
  - Measured: one file through a symlink gives one id, as the doc intends. But that one file can load as two modules with different paths (`app.util` and `app.copy`). These are typed apart and their types are named differently, yet every node id is the same.
  - Harmless today, because the typed tables are per module. A cache keyed by `NodeId` across a compilation, which is the story's outcome, would mix the two up.
  - What reaches it: only a symlinked sibling file. Nothing found.
- **`--stdin-for` of a file that does not exist.** `crates/ignite/src/lib.rs:547`, INFEASIBLE, introduced, note.
  - Measured: when `canonicalize` fails, the id falls back to the path as typed. So the id depends on the working directory and on how the path is spelled. The same fallback applies when a file is deleted between the read and `canonicalize`.
  - What reaches it: an editor buffer not yet saved, if the editor passes relative or differently spelled paths. The LSP is not shown to do that.
  - Fix: fall back to `std::path::absolute`, or canonicalise the parent directory and join the file name.

**5. Attacked and could not break**
- **Parser `*_rest` split:**
  - `emit --typed-json` and `check --json` of all 499 `.ignite` files under std, examples and crates are byte-identical between base and head (998 of 998), with the same exit codes.
  - A hand-written precedence and associativity probe (mixed arithmetic, comparisons, logic, operators across line breaks) also matches.
  - So does a probe of the non-chaining errors (`==` after `==`, `<` after `<`, `>=` after `<=`, `!=` after `!=`, `==` with `<`).
- **Stack depth.** Minimum stack for 250 levels, debug build, base KiB → head KiB:

  | form | base | head |
  |---|---|---|
  | parens | 4474 | 2699 |
  | lists | 4474 | 2731 |
  | calls | 3595 | 3263 |
  | if | 7622 | 6494 |
  | match | 5030 | 3367 |
  | records | 5194 | 3715 |
  | closures | 4974 | 3131 |
  | `1 + (` | 4474 | 2991 |
  | `1 < (` | 4474 | 3103 |
  | `2 * (` | 4474 | 2991 |
  | tuples | 4474 | 2699 |
  | index | 3123 | 2659 |
  | method args | 3595 | 3263 |
  | return | 4474 | 2631 |
  | unary `-` and `!` | 327 | 411 |

  Unary is the only form that got worse (+26 %). It still fits 2 MiB, and `MAX_NESTING` (256) caps it on both trees: 257 levels give E054 on both.
- **`NodeId` hash:** `file ^ index·K`, with K odd, is distinct for every index of one file. The splitmix finaliser is a bijection, and `Eq` agrees with `Hash`.
- **`SYNTHETIC` and `NONE`:** a real path's hash is moved off both values.
- **A std module and a program at one canonical path** (a std file under `--stdin-for`): both copies get one `FileId`. But `Modules` keeps only the program's copy (the second module with a path replaces the first, `ignite-eval` lib.rs:73), so no lookup crosses copies. Pass 1 found the output byte-identical.
- **Library callers:** `ignite-check` and `ignite-types` never make a `FileId`. A caller gets the id of the path string it hands to `SourceFile::new`, as the `with_identity` doc says. The only other `SourceFile::new` uses are `rfc check` and a test in `apply/plan.rs`.

**6. Paths written outside the worktree**
- `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-node-ids/adversary2/`, 176 KB left:
  - logs, `compare.sh`, `prec/`, `probe-src/`, `probe-base/`, `probe-head/`, `rho/`, `rho.out`, the `stack-*.txt` results.
  - I created and then deleted under it: `base/` and `head/` (exports), `base-target/` and `head-target/` (698 MB each), `wt-target/` (908 MB), the probe and rho targets, and core dumps.
- I also wrote `crates/ignite/tests/fixtures/` nothing; the symlink fixture is built under the build target's `CARGO_TARGET_TMPDIR`, which was in `wt-target`, now deleted.

**7. Findings**
```findings
- file: crates/ignite-syntax/tests/node_ids_adversary.rs
  line: 46
  category: property
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: the 2,000,000-path bound makes the collision test unable to fail on a 64-bit FileId, while a rho search finds a real 64-bit FNV-1a sibling-path collision in 30 s (red in node_ids_adversary2::two_sibling_paths_have_distinct_file_ids and ::two_sibling_files_share_no_node_id)
- file: crates/ignite/src/lib.rs
  line: 545
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: through a symlinked sibling, one canonical file loads as two modules (app.util, app.copy) typed apart but sharing every node id, so a compilation-wide NodeId cache would conflate them (red in file_ids_adversary::two_modules_of_one_compilation_share_no_node_id)
- file: crates/ignite/src/lib.rs
  line: 547
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: when canonicalize fails (a --stdin-for buffer not yet saved, or a file deleted after the read) the id falls back to the path as typed, so three spellings of one path give three FileIds (red in file_ids_adversary::stdin_for_a_missing_file_has_one_id_for_every_spelling)
```
