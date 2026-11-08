# Install

From v0.13.0 on, each [release](https://github.com/ignite-lang/ignite/releases) carries a
pre-built `ignite` for Linux x86_64; on any other system, install from source.

## Pre-built binary

The archive `ignite-X.Y.Z-x86_64-linux.tar.gz` holds `bin/ignite` (glibc, linked statically)
and `share/ignite/std`, the standard library it reads. Unpack it into a prefix whose `bin` is on
your `PATH`, here `~/.local`:

```sh
curl -LO https://github.com/ignite-lang/ignite/releases/download/v0.15.0/ignite-0.15.0-x86_64-linux.tar.gz
tar -xzf ignite-0.15.0-x86_64-linux.tar.gz
cp ignite-0.15.0-x86_64-linux/bin/ignite ~/.local/bin/
mkdir -p ~/.local/share/ignite
cp -R ignite-0.15.0-x86_64-linux/share/ignite/std ~/.local/share/ignite/
```

A driver at `PREFIX/bin/ignite` reads `PREFIX/share/ignite/std` when it exists. It carries the
runtime archive of the LLVM target, so `check`, `build` and `test` on the default target need
only clang. The Rust target (`--target rust`) also needs the IO substrate crate of a clone,
`--substrate CLONE/crates/ignite-io`.

## Container image

Each release from v0.13.0 on also has a public image,
`ghcr.io/ignite-lang/ignite-toolchain:X.Y.Z`, with everything `ignite build` needs on both
targets: the pre-built `ignite` and its standard library, clang and lld, Rust 1.98, and the
release's source in `/usr/local/src/ignite`, so `--target rust` needs no `--substrate`. The
substrate's crates are fetched into the image and `CARGO_NET_OFFLINE=true` is set, so builds
need no network. The release notes give the image's `@sha256:` digest; pin that.

```sh
docker run --rm -v "$PWD:/work" ghcr.io/ignite-lang/ignite-toolchain:0.15.0 \
  ignite build hello.ignite --target rust
```

## Install from source

The releases are also tags of the repository (v0.1.0 to v0.15.0).

### What you need

| tool | needed for |
|---|---|
| git | getting the source |
| Rust, stable, with edition 2024 (cargo) | installing the compiler and the runtime archive, and every build on the Rust target (`--target rust`) |
| clang | every build on the LLVM target, the default |
| [go-task](https://taskfile.dev) | optional: the `task` shortcuts of the repository |

### Install

```sh
git clone https://github.com/ignite-lang/ignite
cd ignite
cargo install --locked --path crates/ignite
```

This builds the `ignite` command and puts it in cargo's binary directory (`~/.cargo/bin`
unless you set `--root`), which rustup puts on your `PATH`. Check it:

```sh
ignite --version
```

```text
ignite 0.15.0+g91fa0a25740a
```

A build from a clone names the commit it was built from after `+g`, with `.dirty` when a
tracked source of the compiler's crates (not `std/` or docs) was modified; only a clean build
of a release tag, such as the release archive, prints the bare version (`ignite 0.15.0`).

`ignite --help` lists the commands: `check`, `emit`, `build`, `test`, `plan`, `host`,
`apply`, `inspect`, `recover`, `explain`, `run`, `replay`, `rfc` and `conformance`. The
[command-line reference](../reference/cli.md) covers each one but `recover`, `rfc` and
`conformance`. Every page of this book writes the command as plain `ignite`.

**Keep the clone.** The installed `ignite` reads the standard library (`std/`) and the IO
substrate crate (`crates/ignite-io`) from the clone it was installed from, so it works from
any directory as long as the clone stays where it is. Every command that reads a program
takes `--std DIR` to use another copy of the standard library.

## What a build needs at run time

`ignite check`, `plan` and `explain` need nothing else. Building a program does:

- **LLVM target (the default).** `ignite build` never runs cargo or rustc for the program:
  clang compiles the generated LLVM IR and links it against the runtime archive
  `libignite_rt.a`, with the Linux system libraries. `cargo install` builds that archive, with
  the test runner that `ignite test` links, into the `ignite` binary, and the first LLVM
  build writes it to `$XDG_CACHE_HOME/ignite/runtime/` (`~/.cache/ignite/runtime/` when
  `XDG_CACHE_HOME` is unset), so there is nothing to pass:

  ```sh
  ignite build hello.ignite
  ```

  An archive given with `--runtime`, or one next to the `ignite` binary or in that
  directory's parent, comes first. [Building for Rust and LLVM](targets.md) has the details.
- **Rust target.** `ignite build --target rust` writes a Rust project and runs `cargo build`
  on it, so cargo must be on the `PATH`. The first build of a project fetches the crates the
  IO substrate depends on.

## Check that it works

From the clone:

```sh
ignite check examples/terminal/terminal.ignite
```

`ignite check` prints nothing and exits 0 when the program and the standard library are
well formed. Next, [write your first program](first-program.md).

## The task shortcuts

Inside the clone, `Taskfile.yml` wraps the common steps. These build the compiler from the
clone themselves, so they do not need the installed `ignite`:

| task | what it does |
|---|---|
| `task build` | build every crate of the workspace |
| `task test` | every Rust test, then the inline Ignite tests |
| `task test-ignite` | the inline tests of `std/` and `examples/` on both targets (`FILTER=text` selects tests) |
| `task check` | `cargo fmt --check` and clippy with warnings denied |
| `task compile` | build the terminal example with `--target rust` into `target/ignite/terminal`; prints `built …/terminald` and `built …/tn` |
| `task compile-llvm` | the same with the LLVM target, into `target/ignite-llvm/terminal/bin/` |
| `task server`, `task client` | build and run `terminald` and `tn` (`PORT=n`, `TARGET=llvm`) |
| `task compile-todo`, `task todo-server` | build (`--target rust`) and run the todo HTTP backend `todod` on port 8080 (`TARGET=llvm`) |
| `task site`, `task site-serve` | build this website into `target/site`, and serve it on `http://127.0.0.1:8765/` |
