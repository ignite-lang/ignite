---
format: aep.planning-md/3
id: review-result:adversary-rustgen-names-pass-2
kind: review-result
status: active
title: Adversary, rustgen-reserved-names, pass 2
relations:
- reviews: story:rustgen-reserved-names
revision: 1
---
unit: story:rustgen-reserved-names, branch fix/rustgen-reserved-names at 7a6b75d6 plus 2 uncommitted test cases in the worktree
verdict: CONFIRMED
cases: executed 139→141 (ignite-rustgen suite), red 2
origin: introduced 0 / pre-existing 2 / undecided 0
wrote-outside-worktree: 1 path
needs-coordinator: both red cases also fail at the base 64585854, so you decide whether they hold this unit or go to the backlog

**Summary:** All four pass-1 findings are fixed. My two new cases fail on the Rust target and LLVM builds both programs. Both failures also happen at the base 64585854. I found nothing new that this round's correction introduced.

1. Diff of the worktree. The only file is a test file:
```
 crates/ignite-rustgen/tests/names.rs | 84 ++++++++++++++++++++++++++++++++++++
 1 file changed, 84 insertions(+)
```

2. Two cases added at the end of `crates/ignite-rustgen/tests/names.rs`. Both are red now.
   - `adversary_a_record_named_like_the_program_crate_builds`: package `examples.crt` declares `type crt`, and a component property calls the sibling fn `seven()`. The program must build with cargo.
   - `adversary_a_resource_named_like_a_ufcs_callee_builds`: the resources `to_upper` and `twice`, and a component property that calls `"abc".to_upper()` and `3.twice()` with method syntax. The program must build with cargo.
   - Output of `cargo test -p ignite-rustgen --test main -- <the two cases>`, run before the suite:
```
error[E0659]: `crt` is ambiguous
error[E0659]: `crt` is ambiguous
test names::adversary_a_record_named_like_the_program_crate_builds ... FAILED
error[E0618]: expected function, found `ignite_io::TcpListener`
test names::adversary_a_resource_named_like_a_ufcs_callee_builds ... FAILED
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 140 filtered out; finished in 0.73s
```

3. Suite run, made after the cases existed: `CARGO_INCREMENTAL=0 cargo test -p ignite-rustgen --no-fail-fast`.
   - Result: `test result: FAILED. 139 passed; 2 failed; 1 ignored; 0 measured; 0 filtered out; finished in 2.96s`, exit 101.
   - The 2 failures are my cases.
   - The three pass-1 cases (`sibling_module_named_option`, `sibling_module_named_result`, `listener_named_port_beside_a_fn_named_port_underscore`) pass.

4. Findings, covering 7a6b75d6 plus the two cases.
   - **A record named like the program's own crate** (`crates/ignite-rustgen/src/lib.rs:6442`). Confirmed, pre-existing, warning.
     - What was measured: the executable writes `use crt::*;` and `use crt::other::seven;` with the program crate's bare name. That is the one crate root not written as `::crt`. The record `pub struct crt` comes in through the glob next to the extern crate `crt`, so rustc fails with E0659 at `src/bin/srv.rs:2`.
     - What reaches it: any system whose main module declares a name equal to the package's last segment. `ignite check` exits 0 and `ignite build --target llvm` builds it.
     - Base compiler: built from 64585854 in scratch, it fails the same way.
     - Why it matters: the story's Outcome says a program may name a record after "a crate root".
     - Likely fix: write `::<crate>::` in the bin's `use` lines and paths.
   - **A resource named like a fn called with method syntax** (`crates/ignite-rustgen/src/lib.rs:6391`, `resource_variable` = `ident`). Confirmed, pre-existing, warning.
     - What was measured: Ignite resolves `x.f()` to the fn `f` even next to a resource `f`, so the checker accepts the program. The generator writes the bare call `f(&x)` in `main`, after `let f = ...TcpListener::bind(...)`, so rustc fails with E0618. This happens for a program fn (`twice`) and for an imported std fn (`to_upper`).
     - What reaches it: `ignite check` exits 0 and LLVM builds it.
     - Base compiler: fails the same way.
     - Doc impact: the new sentence in `docs/slice.md:731-733` ("spelled like a program item only when both have one Ignite name") is literally true, but it implies safety that this case disproves.
     - Likely fix: give resource locals a `__` form, as `endpoint_variable` already has, or write UFCS callees by path.

5. Attacked and could not break:
   - **Old escape list:** `port` was guarded because of `main`'s old `let port` (now `__port`). `application`, `stream` and `error` guarded nothing anything still binds in `main`. The spawn template's `Err(error)` cannot become a pattern, because constants are emitted as fns and protocols as braced structs.
   - **Components:** a component named like a fn that a property calls is refused (E040). A resource and a component with one name are refused (E004).
   - **Names in `main`:** a resource named like the kind (`Srv`) only meets type-namespace paths (`Srv {..}`, `Srv::default_x()`). `Srv__main`, `__a0` and the glue `__ignite_*` names are out of reach, because `ident` escapes `__`.
   - **`::std::option::Option` paths:** no format string joins a type string onto an identifier or a `:`. The variants stay bare and are value-namespace only, so a sibling module `Some` or `None` cannot shadow them. A record, fn or variant named `Some`, `None`, `Ok` or `Err` gets a `_` through `RUST_PRELUDE_NAMES`. `unshadowed` and `variant_path` handle `Option.Some` paths.
   - **`docs/slice.md`:** the lowering table is contiguous again (lines 469-521) and the paragraph now sits after it.
   - **`CODEGEN_REVISION`:** bumped once for the unit (36 to 37).
   - **Both targets:** each of my probes builds with `--target llvm`. Only Rust fails.

6. Path written outside the worktree:
   - `/home/timo/.cache/scratch/e18a99fc-f99c-4987-93c9-e5de2080b61a/unit-rustgen-names/adversary2/`, 159 MB: probes `p1` to `p4` with their build outputs, `red.log`, `suite.log`, `base-build.log`.
   - I deleted the base compiler's build directory there (`base-target`, 698 MB).
   - Probe builds used the worktree's git-ignored `target/` as `CARGO_TARGET_DIR`.

7.
```findings
- file: crates/ignite-rustgen/src/lib.rs
  line: 6442
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: the executable spells the program crate bare (use crt::*, crt::other::seven), so a record named like the package's last segment makes the bin fail E0659 on Rust while LLVM builds it
- file: crates/ignite-rustgen/src/lib.rs
  line: 6391
  category: boundary
  severity: warning
  verdict: CONFIRMED
  origin: pre-existing
  message: a resource named like a fn that a component property calls with method syntax (x.f()) is the local f in main, which shadows the bare call f(&x) (E0618 on Rust; ignite check and LLVM accept it)
```
