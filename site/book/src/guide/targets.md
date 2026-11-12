# Build for the Rust target and static executables

`ignite build` uses the LLVM target unless told otherwise. This page covers the other builds:
the Rust target, release builds and static executables. Both targets compile the same checked
program and must behave the same; why both exist is in
[One substrate, two targets](../design/substrate.md). Every flag is in the
[command-line reference](../reference/cli.md#ignite-build).

## The Rust target

`--target rust` writes a Rust project and runs `cargo build` on it, so cargo must be on the
`PATH`. The generated project depends on the IO substrate crate, `ignite-io`
([where it is looked for](install.md#pre-built-binary)).

- An `ignite` installed from source finds the crate in its own clone. Nothing to pass.
- The pre-built binary from v0.17.0 on finds the crate its archive installed beside it,
  `share/ignite/substrate`. Nothing to pass.
- An older pre-built binary does not carry it. Pass the crate of a clone of the same release
  with `--substrate`, as an absolute path. The clone of
  [Get the examples](install.md#get-the-examples) has it:

```sh
ignite build examples/todo/todo.ignite --target rust --substrate "$PWD/crates/ignite-io"
```

```text
note: building the generated Rust project in target/ignite/todo; compiler messages refer to generated code
    Updating crates.io index
     Locking 6 packages to highest Rust 1.99.1 compatible versions
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.20s
built target/ignite/todo/target/debug/todod
built target/ignite/todo/target/debug/health
```

The executables are in cargo's target directory, `target/ignite/<package>/target/debug/`. The
first build of a project fetches the substrate's crates from crates.io. `ignite emit FILE`
prints the generated Rust instead of building it. Generated Rust is an artifact: never edit it
and never commit it.

`ignite test --target rust` and `--target all` take `--substrate` too, with the same meaning.

## Release builds

`--release` builds with clang `-O2` on the LLVM target, where it also strips the debug
sections, and with the release profile on the Rust target:

```sh
ignite build examples/todo/todo.ignite --release
```

```text
built target/ignite/todo/bin/todod
built target/ignite/todo/bin/health
```

Here `todod` is 926,120 bytes against 5,137,808 for the dev build. Dev builds carry debug
metadata, so a debugger shows Ignite source lines.

## Static executables

`--static` links every executable against musl, on either target. The result has no dynamic
loader and no shared library, so it runs on a host or in a container that has no libc:

```sh
ignite build examples/todo/todo.ignite --static
file target/ignite/todo/bin/todod
```

```text
built target/ignite/todo/bin/todod
built target/ignite/todo/bin/health
target/ignite/todo/bin/todod: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, …
```

The pre-built `ignite` carries the musl runtime, so a static LLVM build needs only clang. The
Rust target builds with `cargo build --target x86_64-unknown-linux-musl` and needs that Rust
target:

```sh
rustup target add x86_64-unknown-linux-musl
ignite build examples/todo/todo.ignite --static --target rust --substrate "$PWD/crates/ignite-io"
```

```text
…
built target/ignite/todo/target/x86_64-unknown-linux-musl/debug/todod
built target/ignite/todo/target/x86_64-unknown-linux-musl/debug/health
```

Without the musl target the build stops with exit 2 and names what to install. It never falls
back to a dynamic build. `--triple` with `--static` must name a musl target. `ignite test
--static` and `ignite conformance --static` build their test executables the same way. Sizes
and build times against a dynamic build are in
[`docs/performance/static/`](https://github.com/ignite-lang/ignite/tree/main/docs/performance/static).

## Another clang or target triple

On the LLVM target, `--clang PATH` names the clang to use and `--triple T` the target triple,
by default clang's own. A runtime archive built for another target is refused before clang
runs, with exit 3:

```sh
ignite build examples/todo/todo.ignite --triple aarch64-unknown-linux-gnu
```

```text
error: /home/user/.cache/ignite/runtime/f9d45032cf23f20d/libignite_rt.a: the runtime archive is not built for the target aarch64-unknown-linux-gnu: its objects are ELF64 little-endian machine 62, the target needs machine 183 (build ignite-rt for aarch64-unknown-linux-gnu and pass it with --runtime)
```

`--runtime FILE` gives an archive of your own, built for that target.

## Build times

The measured build times of both targets, per release, are on the [Quality](../quality.md)
page.
