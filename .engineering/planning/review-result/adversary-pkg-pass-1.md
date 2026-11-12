---
format: aep.planning-md/3
id: review-result:adversary-pkg-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit pkg (release v0.16.1)
relations:
- reviews: story:release-archive-carries-the-substrate
revision: 1
---
red

Adversary pass on unit `pkg` at `9264ba87` (2026-11-12): NEEDS-CHANGE; 10 findings introduced, 3 older. Tests on branch `wave/13-pkg-adv` (`aa68393a`), file `crates/ignite/tests/dist_adversary.rs`.

- Introduced, fixed in `8b4b6a78`: the packing check needed crates of other platforms (`windows-sys`, `wasi`) that a host build never fetches, so `task gate` and `task dist` failed on a host-only cargo home. The pack now trims the lock with `cargo update --workspace --offline` and checks it with `cargo tree --locked --offline` over the host's edges; the adversary's case is in `dist.rs`.
- Introduced, fixed in `8b4b6a78`: six doc sentences named v0.17.0 as the first release with the substrate; the release is v0.16.1.
- Introduced, fixed in `8b4b6a78`: nothing checked the `.sha256` that `task dist` writes (`sha256sum -c` is now a step); the archive packed the substrate's 19 test files (left out); the Dockerfile's `emit | grep -q` made the driver panic on a closed pipe in the build log (the output goes to a file); no test covered the `Cargo.toml` condition of the installed substrate (one case added); `ignite conformance` and `ignite run` rejected `--substrate` (both take it).
- The story's outcome, older than the unit: the released driver held the build user's home directory 762 times (cargo registry 321, toolchain 441), as the published v0.16.0 driver does. `task dist` now remaps the cargo home and the sysroot; 16 remain, the source names of ring's C files in the bitcode runtime, which clang does not remap. The archive check skips those by name and fails on any other.
- Kept: the byte search for build paths runs in `task dist` only; a gate build keeps the clone fallback by design. The Dockerfile builds images of v0.16.1 and later only.
- Filed as `story:substrate-lookup-says-what-it-found`: a relative `--substrate` is resolved against two directories (older); a released driver without its substrate shows only cargo's error (older); a substrate of another release beside the driver is used without a notice.

Attacked and not broken: 38 cases in seven groups (the archive outside the repository under `bwrap` with the tree hidden, the order of the three places, the seeded lock against the repository's, build paths in the driver and the packed files, the archive's content, a driver in a build tree, the Dockerfile against the two image tasks), and every added doc sentence checked by a command. `task dist`, `task image` and `task image-check` were read line by line against the new layout: nothing fails on a machine whose cargo home holds the host's crates.

The coordinator reran the unit's cases after the correction on `8b4b6a78`: 90 passed, 0 failed, 4 ignored in `dist:: image:: quality:: site:: apply_way_out::`; `task --list` 26 tasks.

```findings
[
{"file":"crates/ignite/tests/dist.rs","line":156,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The packing check (cargo metadata/fetch --offline of the packed substrate) needs other platforms' crates (windows-sys, wasi) that a workspace build never fetches, so task gate and task dist fail on a cargo cache without them."},
{"file":"AGENTS.md","line":536,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"AGENTS.md, install.md:9/45, targets.md:16, container.md:33 and cli.md:67 say the substrate ships from v0.17.0, while the release cut from this tree is v0.16.1."},
{"file":"site/book/src/reference/cli.md","line":64,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"A relative --substrate is not resolved against the generated project: the std crate one directory down resolves it again and seed_lock resolves it against the cwd, and ignite test --substrate now reaches this too; filed as story substrate-lookup-says-what-it-found."},
{"file":"crates/ignite/src/lib.rs","line":353,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"A released driver without share/ignite/substrate fails with cargo's missing-manifest error only, naming neither --substrate nor the order of the places looked at; filed as story substrate-lookup-says-what-it-found."},
{"file":"Taskfile.yml","line":138,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"No test or task step checks the .sha256 that task dist writes; its first check is the image's sha256sum -c after upload."},
{"file":"crates/ignite/tests/dist.rs","line":62,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The archive packs the 19 test files of crates/ignite-io/tests (72,134 of the 218,724 added bytes) and the lock carries their dev-dependencies, which the image fetches."},
{"file":"dist/toolchain/Dockerfile","line":36,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"ignite emit | grep -qxF makes the driver panic with 'failed printing to stdout: Broken pipe' in the image build log; the step's status is unaffected."},
{"file":"crates/ignite/src/lib.rs","line":327,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"A stale substrate beside the driver is used before the source tree with no version check or notice, and a mismatch shows only as Rust compile errors; filed as story substrate-lookup-says-what-it-found."},
{"file":"dist/toolchain/Dockerfile","line":25,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The Dockerfile can no longer build an image for v0.16.0 or older because cp of share/ignite/substrate fails on those archives."},
{"file":"crates/ignite/src/lib.rs","line":327,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"No test covers the Cargo.toml condition of installed_substrate_path; changing it to is_dir() would leave the suite green (reasoned, not executed)."},
{"file":"crates/ignite/tests/dist.rs","line":1,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"The task dist driver holds the build user's home directory 762 times (cargo registry 321, rustup toolchain 441), the same counts as the published v0.16.0 image's driver; the story's outcome asks for no path of the build machine."},
{"file":"AGENTS.md","line":130,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"ignite conformance and ignite run reject --substrate, while AGENTS.md says conformance takes the build flags of ignite test."},
{"file":"crates/ignite/tests/dist.rs","line":1,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The byte search for the build path runs only with IGNITE_DIST_ARCHIVE inside task dist, never in task gate; kept, since a gate build keeps the clone fallback by design."}
]
```
