---
format: aep.planning-md/3
id: review-result:adversary-unsafe-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit unsafe (wave 11)
relations:
- reviews: story:unsafe-sites-back
revision: 1
---
red

Adversary pass 2 (the last) on unit unsafe, `wave/11-unsafe`, fixes `a6059e1a`, `07cbf054`, `bb7cd7ea`; test commits `769c0058`, `b9d48555`.

verdict: red; cases executed 322→329, red 1; origin: introduced 3.

Findings (all notes):
1. `crates/ignite-io/src/fs.rs:594`, INFEASIBLE, introduced. A tree at the depth bound needs 257 spare descriptors where base needed 256 (the duplicate while the deepest level is listed). Red case `unsafe_adversary_fixes::a_tree_at_the_depth_bound_is_removed_with_the_descriptors_the_base_needed`. Nothing found that reaches it other than a process with exactly base's headroom.
2. `crates/ignite-io/src/fs.rs:597`, INFEASIBLE, introduced. `nix` 0.31.3 `Dir::from_fd` never closes the descriptor when `fdopendir` fails, so the duplicate leaks when `fdopendir` runs out of memory. Read in the code, no case.
3. `crates/ignite-io/src/fs.rs:185`, CONFIRMED, introduced. The doc comment counts one `fcntl` for `fdopendir`; strace shows `F_GETFL` and `F_SETFD` per listing, and each level also costs a dup and a close.

Held: fix 1: `any` is set before `.` and `..` are filtered; a listing that reads entries makes no extra `fstat`; EACCES, ENOTDIR and the ENOENT message as base. Fix 2: each level listed once; every duplicate is `F_DUPFD_CLOEXEC` (614 in the trace); failing walks leave no descriptor open (1, 2, 100, 255 spare). Fix 3: handler installed once, first thing in `spawn`; overflow aborts with the same message; a signal outside a guard page reaches the handler installed before; `restore_default` as base; `guard_page()` reads a const thread-local `Cell`, no lock, no allocation; `unsafe` counts rust 11, llvm 81. Suite: ignite-io lib 112 passed, main 216 passed and 1 failed (the red case); `leak::` 5 passed; clippy and fmt exit 0.

Coordinator's decision on finding 1: one descriptor per level plus one while a level is listed is accepted; the ways back to 256 are the raw `getdents64` call (an `unsafe` site, row 3.6) or 256 open listings (8 MiB at the bound). The case is changed to hold the new bound (levels + 1 passes, levels is refused, tree intact, no descriptor left open) and the bound is stated in the doc comment and `docs/slice.md`.

```findings
[
{"file":"crates/ignite-io/src/fs.rs","line":594,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a tree at the depth bound needs 257 spare descriptors where the base needed 256 (red case unsafe_adversary_fixes::a_tree_at_the_depth_bound_is_removed_with_the_descriptors_the_base_needed)"},
{"file":"crates/ignite-io/src/fs.rs","line":597,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"nix 0.31.3 Dir::from_fd never closes the descriptor when fdopendir fails, so the level's duplicate leaks when fdopendir runs out of memory"},
{"file":"crates/ignite-io/src/fs.rs","line":185,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the doc comment counts one fcntl for fdopendir, but strace shows F_GETFL and F_SETFD per listing, and each level also costs a dup and a close"}
]
```
