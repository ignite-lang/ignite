# Empty Cargo harness inventory

This diagnostic verifies six manifest-only settings on the same current source tree.
It is not performance acceptance and does not execute test bodies. The only production
changes are `test = false` and `bench = false` for five previously empty library harnesses
(`ignite-syntax`, `ignite-types`, `ignite-rustgen`, `ignite-eval`, `ignite-llvm`) and the
previously empty `ignite` binary harness. Normal library/binary targets, all integration
targets, every doctest flag, compiler profiles and lint flags remain unchanged.

The same commands ran before and after:

```text
cargo metadata --no-deps --format-version 1
cargo clippy --workspace --all-targets --message-format=json -- -D warnings
cargo test --workspace --no-run --message-format=json
```

Cargo JSON changes diagnostic output only. Compiler-artifact records establish the actual
selected normal/test units, including fresh units. Every produced test executable was then
queried with `--list` and `--list --ignored`; neither command runs cases. The environment
used the same owned `target-main`, offline Cargo, incremental disabled, dev debug line tables
and release debug disabled on both captures. Exact commands are retained per phase.

Clippy test-profile units and no-run test executables both fell from 20 to 14. Every removed
harness had zero cases and zero ignored cases. All 659 named cases, including the same five
ignored cases, match exactly by package/target. All normal clippy/no-run units and all 20
Cargo metadata target roots remain. The normal CLI exists with the same SHA256 before/after.
`verification.json` records the comparisons; each phase retains metadata, case inventories,
raw list output and source SHA256 snapshots. `manifest-changes.json` includes exact before/after
manifest text; the pre-change content was recovered from HEAD only after its hash was checked
against the recorded before snapshot. Root-authorized documentation additions during this
capture are explicitly listed in the verification record; production Rust/scripts are unchanged.

Maintenance contract: these five libraries keep their cases in `tests/main.rs`, and CLI
cases execute the normal binary through `CARGO_BIN_EXE_ignite`. Before adding source unit tests,
test-only source branches, generated source tests or benchmarks, revisit and re-enable the
corresponding harness flags. Harnesses with real cases remain enabled: driver library,
name checker, IO runtime, LLVM runtime and load binary. Doctests remain enabled everywhere.
Baseline/comparison manifests remain untouched, and the complete cold/warm performance gate
must still pass all existing budgets. No test-count or executable-count reduction is used
as permission to omit an assertion, integration target, source-format check or lint.

`capture.py` is the exact inventory script. `evidence-sha256.json` checksums the retained files.
