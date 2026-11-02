# Soundness audit, 2026-11-01

Probe programs behind `vision.md` section 10. Slice 9 (`docs/slice.md` section 13.11)
fixed what they demonstrate, and `task test` runs every probe:

- each probe is copied to `crates/ignite-types/tests/audit/<dir>_<name>.ignite` with the
  diagnostics `ignite check` must report in the sibling `.expected` (empty for a probe that
  checks clean); `copies_match_the_audit_directory` in `crates/ignite-types/tests/audit.rs`
  fails when a copy drifts from the probe here;
- the probes that check clean run through `ignite test` on both targets:
  `audit_probes_pass` (`llvm/`) and `audit_io_probes_pass` (`io/`) in
  `crates/ignite/tests/slice9.rs`.

Run one by hand with:

```bash
target/debug/ignite test docs/audit/2026-11-01/llvm/<probe>.ignite --target llvm \
  --out target/audit --runtime target/ignite-test/rt/debug/libignite_rt.a
target/debug/ignite test docs/audit/2026-11-01/llvm/<probe>.ignite --out target/audit --target-dir target/audit/cargo
target/debug/ignite check docs/audit/2026-11-01/effects/<probe>.ignite
```

## llvm/

| probe | expected after the fix | today |
|---|---|---|
| `to_bytes_push.ignite` | 2 tests pass on both targets | E006 at check (twice), by a coordinator decision: `x.to_bytes()` is no receiver of `push`. The alias form `var b = x.to_bytes(); b.push(..)` replaces it as a build-and-run test: the three `var to_bytes alias` tests of `crates/ignite/tests/fixtures/audit/aliases.ignite` (`var_aliases_leave_the_source_unchanged`), pass on both targets |
| `for_push.ignite`, `for_push2.ignite` | E006 from `ignite check` | E006 (two sites each) |
| `match_push.ignite`, `match_push2.ignite` | E006 from `ignite check` | E006 |
| `let_push.ignite` | E006 from `ignite check` (three sites) | E006 (three sites) |
| `index_if.ignite`, `index_if_only.ignite` | the index runs | pass on both targets |
| `len_match.ignite` | compiles | compiles; passes on both targets |
| `later_arg.ignite`, `sound_ok.ignite` | pass (they pass today; kept as the positive cases) | pass on both targets |
| `range_exhaustive.ignite` (added in slice 9) | matches on `Byte` and `Int` whose range arms cover the domain check without E046 and never reach the default arm a backend adds | pass on both targets: all 256 bytes, every `Int` from -1100 to 1100 and both extremes |

## io/

| probe | expected after the fix | today |
|---|---|---|
| `p1_recursive.ignite` | accepted: row 15 allows recursion through `Option` and enum payloads | accepted since row 15; builds on both targets |
| `p2_unused.ignite` | a checker diagnostic for the unused type parameter (was: check exit 0, build exit 2 with rustc E0392, E0091, E0283) | E057 (three sites) |
| `p3a_spawn_field.ignite`, `p3b_spawn_value.ignite` | E013 | E013 |
| `p3c_spawn_fieldarg.ignite` | accepted: `spawn` of a call whose arguments are fields | accepted; builds on both targets |
| `p7_store.ignite` | E017 | E017 |
| `p8_assign_param.ignite` | E006 | E006 |
| `p4`, `p5`, `p6`, `p9`, `p9b`, `p10` | build and pass (the positive cases) | pass on both targets |

`iotest/` is a standalone Rust harness over `ignite-io` (its `Cargo.toml` names the crate by
absolute path; build it with its own `--target-dir`): subcommands `drop`, `cross`, `panic`,
`reuse` reproduce vision 10.3 R2 and R3 and the two correct behaviours. It is not part of
`task test`.

## effects/

| probes | expected after the fix | today |
|---|---|---|
| `a01` to `a04`, `b08`, `c01` | rejected: a parameter named like an ambient capability | E056 |
| `a05`, `a06`, `b09`, `b11` | E016: `map` carries the roots of its fn argument | E016 |
| `a07`, `b10` | E016: `push` and `append` on a field extend the place root's roots | E016 |
| `c03` | E042, consistent with `slice.md` §11.7 | E042 |
| `a21` | E116 since slice 11 (`docs/slice.md` 15.2, O16): a property expression is pure, so the effectful call inside `application = ..` is refused; slice 10 accepted it as an effect root | E116 |
| every other probe | the correctly rejected attack keeps the diagnostic `vision.md` section 10.2 names | unchanged; the `.expected` files hold them |

`crates/ignite-types/tests/audit/effects_closure_variants.ignite` repeats the effect probes with
the handle captured by a closure. All at once by hand:

```bash
for f in docs/audit/2026-11-01/effects/*.ignite; do echo "== $f"; target/debug/ignite check "$f"; echo "exit=$?"; done
```
