# Soundness audit, 2026-11-01

Probe programs behind `docs/vision.md` section 10. Every probe passes `telos check`. None is
wired into `task test`; each is a regression test waiting for the fix it demonstrates (vision
wave A0). Run one with:

```bash
target/debug/telos test docs/audit/2026-11-01/llvm/<probe>.telos --target llvm \
  --out target/audit --runtime target/telos-test/rt/debug/libtelos_rt.a
target/debug/telos test docs/audit/2026-11-01/llvm/<probe>.telos --out target/audit --target-dir target/audit/cargo
target/debug/telos check docs/audit/2026-11-01/effects/<probe>.telos
```

## llvm/

| probe | expected after the fix | today |
|---|---|---|
| `to_bytes_push.telos` | 2 tests pass on both targets | LLVM: 2 fail (caller's text mutated, then corrupted); Rust: pass |
| `for_push.telos`, `for_push2.telos` | E006 from `telos check` | LLVM: corrupted elements or a runtime panic; Rust: rustc E0596 |
| `match_push.telos`, `match_push2.telos` | E006 from `telos check` | LLVM: corrupted payload; Rust: rustc E0596 |
| `let_push.telos` | E006 from `telos check` (three sites) | `telos check` exit 0; Rust build fails with rustc E0596 |
| `index_if.telos`, `index_if_only.telos` | the index runs | LLVM: binding never written; Rust: pass |
| `len_match.telos` | compiles | LLVM: compiler panic in `lower.rs` |
| `later_arg.telos`, `sound_ok.telos` | pass (they pass today; kept as the positive cases) | pass on both targets |

## io/

`p2_unused.telos`: `telos check` exit 0 today; `telos build` exits 2 with rustc E0392, E0091
and E0283. Expected after the fix: a checker diagnostic for the unused type parameter. `p1`,
`p3a` to `p3c`, `p7`, `p8`: correctly rejected (E055, E013, E017, E006). `p4`, `p5`, `p6`,
`p9`, `p9b`, `p10`: build and pass on the Rust target; kept as the positive cases.
`iotest/` is a standalone Rust harness over `telos-io` (its `Cargo.toml` names the crate by
absolute path; build it with its own `--target-dir`): subcommands `drop`, `cross`, `panic`,
`reuse` reproduce vision 10.3 R2 and R3 and the two correct behaviours.

## effects/

`a01` to `a07`, `b08`, `b10`, `b11`, `c01`: accepted today, each should be rejected with E016
(or a new diagnostic for a parameter named like an ambient capability). `c03`: E042 today,
consistent with `slice.md` §11.7. `a21`: accepted today; the vision leaves component property
expressions as effect roots. Every other probe is a correctly rejected attack; its expected
diagnostic is in `docs/vision.md` section 10.2. All at once:

```bash
for f in docs/audit/2026-11-01/effects/*.telos; do echo "== $f"; target/debug/telos check "$f"; echo "exit=$?"; done
```
