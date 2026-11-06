---
format: aep.planning-md/3
id: story:static-linking
kind: story
status: active
title: ignite build --static writes fully self-contained executables
summary: Static linking against musl on both targets, so a compiled program runs without any shared library or dynamic loader
scope:
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: cited
  path: crates/ignite/build.rs
- confidence: cited
  path: crates/ignite/src/lib.rs
- confidence: cited
  path: crates/ignite/src/llvm_build.rs
- confidence: cited
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/tests
- confidence: inferred
  path: docs/slice.md
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T01:06:55Z", actor: "human:timo", revision: 9}
- {from: "proposed", to: "active", at: "2026-11-06T01:07:33Z", actor: "human:timo", revision: 10}
---
# Story: static-linking

## Outcome

`ignite build FILE --static` writes fully self-contained Linux executables: no dynamic loader, no
shared library, runnable on a host or in a container that has no libc. Both targets support it; the
LLVM target is the default and the one this story is judged on first.

## Context

- Today an LLVM build links `libignite_rt.a` with clang against glibc and the shared libraries in
  `RUNTIME_LIBS` (`crates/ignite/src/lib.rs:48-56`: `-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`),
  link flags in `crates/ignite/src/llvm_build.rs:260`. `ldd` of the release `ignite-lsp` (2026-11-06)
  lists `libgcc_s.so.1`, `libm.so.6`, `libc.so.6` and `ld-linux-x86-64.so.2`; 1.3 MB.
- `ignite build` already takes `--triple TRIPLE` (`crates/ignite/src/main.rs:7`).
- The driver carries one `libignite_rt.a`, built by `crates/ignite/build.rs` for the host triple
  (PR 33); a static build needs a runtime built for the static libc.
- This host has the Rust target `x86_64-unknown-linux-musl` installed and `/usr/bin/musl-gcc`.
- No name resolution in the substrate: `grep getaddrinfo|to_socket_addrs|lookup_host|dlopen` over
  `crates/ignite-io/src` and `crates/ignite-rt/src` finds nothing (2026-11-06), so glibc's NSS
  limits on static linking do not bite today.

## Decisions (defaults, recorded in the contract section the unit writes)

- D1: `--static` links against musl (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` where
  available), not static glibc. Reason: static glibc warns for and breaks NSS and `dlopen`, musl is
  the libc meant for it, and Rust's musl target is static by default.
- D2: the runtime for musl is a second archive, built by `cargo build -p ignite-rt --target
  x86_64-unknown-linux-musl`, found by `--runtime` or written to the runtime cache like the carried
  one; whether the driver also carries it is the unit's call, with the size cost measured.
- D3: `--target rust --static` builds the program crate with `--target x86_64-unknown-linux-musl`.
- D4: a missing musl toolchain is a stated error (exit 2, naming what to install), never a silent
  dynamic build.

## Acceptance

- `ignite build examples/todo/todo.ignite --static` on the LLVM target: `file` reports each
  executable as statically linked, `ldd` reports "not a dynamic executable", and `todod` serves
  its HTTP tests when run with an empty environment from a directory holding only the binary.
- The same on `--target rust`.
- The end-to-end tests that run a compiled program (todo, terminal) run once more with `--static`
  on both targets and pass.
- `--static` without the musl target installed exits 2 with a message naming it.
- Size and build time of `todod` and `ignite-lsp`, dynamic against static, both targets, recorded
  under `docs/performance/`.
- `docs/slice.md` gains the contract subsection; the book's install or build page names the flag;
  `CODEGEN_REVISION` moves if generated code changes.
- `task gate` green.

## Out of Scope

- Cross-compiling to another architecture beyond what `--triple` already does.
- macOS and Windows static builds.
- Making static the default.
