# Install

Each [release](https://github.com/ignite-lang/ignite/releases) from v0.13.0 on carries a
pre-built `ignite` for Linux x86_64. On any other system, install from source.

## Pre-built binary

The archive `ignite-X.Y.Z-x86_64-linux.tar.gz` holds `bin/ignite`, linked statically against
glibc, and `share/ignite/std`, the standard library it reads. From v0.16.1 on it also holds
`share/ignite/substrate`, the IO substrate crate that the Rust target builds against. Unpack
it into a prefix whose `bin` is on your `PATH`, here `~/.local`:

```sh
curl -LO https://github.com/ignite-lang/ignite/releases/download/v0.16.4/ignite-0.16.4-x86_64-linux.tar.gz
tar -xzf ignite-0.16.4-x86_64-linux.tar.gz
cp ignite-0.16.4-x86_64-linux/bin/ignite ~/.local/bin/
mkdir -p ~/.local/share/ignite
cp -R ignite-0.16.4-x86_64-linux/share/ignite/. ~/.local/share/ignite/
```

Each archive has a checksum beside it on the release page, `<archive>.sha256`. Check the
download with `sha256sum -c ignite-0.16.4-x86_64-linux.tar.gz.sha256`.

Check it:

```sh
ignite --version
```

```text
ignite 0.16.4
```

A driver at `PREFIX/bin/ignite` reads `PREFIX/share/ignite/std` when it exists. It also
carries the runtime archive of the LLVM target, the default. So `check`, `build` and `test`
need only clang on the `PATH`. The Rust target, `--target rust`, also needs cargo. It builds
against the IO substrate crate `ignite-io`, the first of:

1. `--substrate PATH` of `ignite build`, `test`, `emit`, `run` or `conformance`;
2. `PREFIX/share/ignite/substrate/crates/ignite-io` for a driver at `PREFIX/bin/ignite`, when
   it exists;
3. the `crates/ignite-io` of the source tree the driver was built from. The pre-built driver
   records none.

So from v0.16.1 on the archive builds and tests on the Rust target without a clone. With an
older archive, pass a clone's crate: [Build for the Rust target](targets.md#the-rust-target).

## Get the examples

The guide's pages build the examples of the repository. Fetch the release's source, without
history:

```sh
git clone --depth 1 --branch v0.16.4 https://github.com/ignite-lang/ignite
cd ignite
ignite check examples/terminal/terminal.ignite
```

`ignite check` prints nothing and exits 0 when the program and the standard library are well
formed. Every page of this book writes the command as plain `ignite` and runs it from this
directory.

## Container image

Each release from v0.13.0 on also has a public image,
`ghcr.io/ignite-lang/ignite-toolchain:X.Y.Z`, with everything `ignite build` needs on both
targets. [Build in a container](../howto/container.md) shows how to use it.

## Install from source

The releases are also tags of the repository, v0.1.0 to v0.16.4. You need:

| tool | needed for |
|---|---|
| git | getting the source |
| Rust, stable, with edition 2024 (cargo) | installing the compiler and the runtime archive, and every build on the Rust target |
| clang | every build on the LLVM target, the default |

```sh
git clone https://github.com/ignite-lang/ignite
cd ignite
cargo install --locked --path crates/ignite
```

This builds the `ignite` command into cargo's binary directory, `~/.cargo/bin` unless you set
`--root`. rustup puts that directory on your `PATH`. Check it:

```sh
ignite --version
```

```text
ignite 0.16.4+g91fa0a25740a
```

A build from a clone prints the commit after `+g`. It appends `.dirty` when a tracked source of
the compiler's crates was modified. Only a clean build of a release tag prints the bare
version.

**Keep the clone.** An `ignite` installed this way reads the standard library (`std/`) and the
IO substrate crate (`crates/ignite-io`) from the clone it was built from. It works from any
directory as long as the clone stays where it is. `--std DIR` selects another copy of the
standard library.

## What a build needs

| command | needs |
|---|---|
| `ignite check`, `plan`, `explain` | nothing else |
| `ignite build`, `test`, `run` on the LLVM target | clang. The runtime archive comes from the `ignite` binary, written once to `$XDG_CACHE_HOME/ignite/runtime/`, else `~/.cache/ignite/runtime/`. No cargo, no network |
| the same with `--target rust` | cargo on the `PATH` and the IO substrate crate, above. The first build of a project fetches the crates the IO substrate depends on |

`ignite --help` lists the commands. The [command-line reference](../reference/cli.md) covers
each one, with every flag and exit status.

Next: [your first program](first-program.md).
