---
format: aep.planning-md/3
id: review-result:adversary-rustgen-names-pass-1
kind: review-result
status: active
title: Adversary, rustgen-reserved-names, pass 1
relations:
- reviews: story:rustgen-reserved-names
revision: 1
---
unit: story:rustgen-reserved-names, branch fix/rustgen-reserved-names at 2f1b0adf plus 3 uncommitted test cases in the worktree
verdict: NEEDS-CHANGE
cases: executed 136→139 (ignite-rustgen suite), red 3
origin: introduced 2 / pre-existing 3 / undecided 0
wrote-outside-worktree: 1 path
needs-coordinator: two of the three red cases come from code that predates this change, so you decide whether they hold this unit or go to the backlog

**Summary:** three new test cases fail on the Rust target. LLVM builds all three programs. All three failures also happen at the base commit (64585854). Two problems in `docs/slice.md` are new in this change: one wrong claim and one broken table. The goldens change only by renames, and both targets agree on every program in `names.rs`.

1. Diff of the worktree. The only file is a test file:
```
 crates/ignite-rustgen/tests/names.rs | 88 ++++++++++++++++++++++++++++++++++++
 1 file changed, 88 insertions(+)
```

2. Cases added to `crates/ignite-rustgen/tests/names.rs`. All three are red now. Output of running these three cases alone, before the suite:
   - `adversary_a_sibling_module_named_option_builds` (:505) and `adversary_a_sibling_module_named_result_builds`: a sibling module named `Option` or `Result`, beside fns that return `Option<Int>` and `Result<Int, Text>`, must build with cargo.
   - `adversary_a_listener_named_port_beside_a_fn_named_port_underscore_builds` (:523): a listener named `port`, and a component property that calls a program fn `port_()`, must build.
```
error[E0573]: expected type, found module `Result`
  --> src/lib.rs:13:28
test names::adversary_a_sibling_module_named_result_builds ... FAILED
error[E0618]: expected function, found `ignite_io::TcpListener`
 --> src/bin/srv.rs:9:53
test names::adversary_a_listener_named_port_beside_a_fn_named_port_underscore_builds ... FAILED
error[E0573]: expected type, found module `Option`
 --> src/lib.rs:4:25
test names::adversary_a_sibling_module_named_option_builds ... FAILED
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 137 filtered out; finished in 1.28s
```

3. Suite runs, made after the cases existed.
   - `CARGO_INCREMENTAL=0 cargo test -p ignite-rustgen --no-fail-fast`: `test result: FAILED. 136 passed; 3 failed; 1 ignored; ... finished in 6.69s`, exit 101. The 3 failures are the cases above.
   - The same command with `--skip` on my three cases: `test result: ok. 136 passed; 0 failed; 1 ignored; 3 filtered out`, exit 0.

4. Findings, covering 2f1b0adf plus the three cases.
   - **Sibling module named `Option` or `Result`** (`crates/ignite-rustgen/src/lib.rs:1603`, `builtin_enum_path`). Confirmed, pre-existing.
     - What was measured: `rust_type_in` and `rust_ty` spell the builtin enums as bare `Option<..>` and `Result<..>`. E004 refuses a type, enum or protocol named `Option`, but not a module. So `pub mod Option` at the crate root takes the name, and the bin's `use crate::*` brings it in as well.
     - What reaches it: any program with a file `Option.ignite` or `Result.ignite`. `ignite check` accepts it, and `ignite test --target llvm` passes the same program. The base compiler fails the same way (6× E0573).
     - Siblings named `String`, `Vec`, `Box`, `Clone`, `Fn`, `i64`, `bool`, `core`, `alloc`, `ignite_std` and `main` all build.
     - Likely fix: always write `::std::option::Option` and `::std::result::Result`, or escape `Option` and `Result` as module names.
   - **Resource variable spelled like a program fn** (`crates/ignite-rustgen/src/lib.rs:6398`, `resource_variable`). Confirmed, pre-existing.
     - What was measured: `resource_variable` adds a `_` to a resource named `port`, `application`, `stream`, `error` or `*_endpoint`. `ident` leaves a program fn `port_` as `port_`, so the listener local is `port_`, and a property that calls `port_()` calls the `TcpListener`.
     - What reaches it: `ignite check` passes and LLVM builds. The base compiler fails the same way (E0618).
     - Since `__port` replaced `port`, that escape list no longer protects anything. Likely fix: drop the list, or escape through `ident`'s scheme.
   - **Wrong claim in the contract** (`docs/slice.md:730-733`). Needs change, introduced.
     - The new text says `main`'s locals cannot be spelled like any program name, and that the remaining trailing `_` is one to one "as `ident` escapes `self`". The `port`/`port_` case above shows it is not.
     - Nothing breaks at runtime; it is a wrong statement in the contract.
   - **Lowering table split** (`docs/slice.md:508-511`). Needs change, introduced.
     - The new paragraph sits between two rows of the lowering table. Rows from `while`/`loop` onward come after a paragraph and no longer render as a table. At the base those rows were contiguous.

5. Attacked and could not break:
   - Goldens: every one of the 520 changed lines pairs up after undoing the renames (the `::` crate-root prefixes, `__port`, `__endpoint_*`, `pub struct P {}`). No other change.
   - Both targets: all 13 programs in `names.rs` pass `ignite test --target all`, or build on both `llvm` and `rust` for the system programs.
   - Type parameters named `Option` and `Result` in fns and records, used in closures, equality checks, record literals and lists, build on Rust.
   - A resource named like a fn called in a property is refused by the checker (E040), so it never reaches the generator.
   - `__endpoint_` names are one to one, because `_` is always written `_0`.
   - The generator has no unit structs, consts, statics or glob-imported variants left that a binding could turn into a pattern.
   - Programs cannot declare macros. `show` and `fresh` in the goldens are user fns, not generator helpers.

6. Paths written outside the worktree:
   - `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-rustgen-names/adversary/`, 414 MB in total: probe programs and their build outputs, the base source copy, and the suite logs.
   - I deleted the base compiler's build directory (`base-target`, 698 MB) and two base output directories.
   - Some probe builds ran with `CARGO_TARGET_DIR` set to the worktree's own `target/`. That directory is git-ignored and inside the worktree.

7.
```findings
- file: crates/ignite-rustgen/src/lib.rs
  line: 1603
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a sibling module named Option or Result shadows the bare prelude Option/Result the generator writes, so the program crate fails E0573 on Rust while LLVM builds it
- file: crates/ignite-rustgen/src/lib.rs
  line: 6398
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: resource_variable turns a listener named port into the local port_, which shadows a program fn port_ called in a component property (E0618 on Rust, LLVM builds)
- file: docs/slice.md
  line: 730
  category: contract-drift
  severity: warning
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the new text says no program name can be spelled like main's locals and that the remaining trailing-underscore escape is one to one, which the port/port_ case disproves
- file: docs/slice.md
  line: 508
  category: judgement
  severity: note
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the new paragraph is inserted between two rows of the lowering table, so the rows from while/loop onward no longer render as a table
```
