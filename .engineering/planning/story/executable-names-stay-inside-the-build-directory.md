---
format: aep.planning-md/3
id: story:executable-names-stay-inside-the-build-directory
kind: story
status: active
title: A component's executable name cannot write outside the build directory
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:50:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:50:59Z", actor: "human:timo", revision: 3}
---
# Story: a component's executable name cannot write outside the build directory

## Outcome

A component whose `executable` property is not one portable file name (empty, `.`, `..`, a path separator, an absolute path) is refused by `ignite check` with a diagnostic at the property, and neither the Rust project writer nor the LLVM link step creates or replaces a file outside the output directory it was given, whatever name reaches them.

## Why

IG-01, severity high, class security; traced in source only (the maintainer review of 2026-11-23 against main at a1d1407e). No program was built to show the write.

The type checker copies the string literal of a component's `executable` property into `ExecutableInfo.executable` unchanged (`crates/ignite-types/src/typer/system.rs:1778` to `:1786`); the only other checks on the property are its type (`:744`, `:1132`). The Rust project writer joins the name to the project's binary source directory as `<name>.rs` (`crates/ignite-rustgen/src/lib.rs:1578`). The LLVM build joins it to the binary directory as the link output (`crates/ignite/src/llvm_build.rs:661`) and also names the object `main_<name>.o` (`:670`) and the stamp `<name>.link.stamp` (`:681`) after it. A name such as `../../x` or an absolute path makes `Path::join` leave the directory, so building a project somebody else wrote can overwrite a file the building account may write.

## Acceptance

- Fixtures with `executable = "../escape"`, with `executable = "a/b"` and with an absolute path: `ignite check` fails with a diagnostic at the property (red first: today all three pass), and `ignite build` on both targets writes nothing outside its output directory.
- Each writer refuses an `ExecutableInfo.executable` that is not a single file name before it touches the disk: one unit test per writer that constructs such a value directly.
- Controls: `executable = "daemon.v2"` and names with `-` and `_` still build (`story:run-finds-an-executable-whose-name-has-a-dot` needs the dot); a component without `executable` still gets its own name.
- `docs/slice.md` (rule 12 at line 2641) states the allowed form; `task gate` green.

## Scope

- `crates/ignite-types/src/typer/system.rs:1778` to `:1786` (`executable`; the review cites `:1781`, the `find` in that chain), `crates/ignite-rustgen/src/lib.rs:1578` (`write_project`), `crates/ignite/src/llvm_build.rs:661` (the link loop), `docs/slice.md:2641`. Each cited line was read at a1d1407e and says what the review states.
