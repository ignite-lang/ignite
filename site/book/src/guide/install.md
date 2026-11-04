# Install from source

Ignite is installed from source. There are no binary releases yet; the releases (v0.1.0 to
v0.3.0) are tags of the repository.

## What you need

| tool | needed for |
|---|---|
| git | getting the source |
| Rust, stable, with edition 2024 (cargo) | installing the compiler, and every build on the Rust target, the default |
| clang | the LLVM target only (`--target llvm`) |
| [go-task](https://taskfile.dev) | optional: the `task` shortcuts of the repository |

## Install

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
ignite 0.3.0
```

`ignite --help` lists the commands: `check`, `emit`, `build`, `test`, `plan`, `host`,
`apply`, `inspect`, `explain`, `run` and `replay`. The
[command-line reference](../reference/cli.md) covers each one. Every page of this book
writes the command as plain `ignite`.

**Keep the clone.** The installed `ignite` reads the standard library (`std/`) and the IO
substrate crate (`crates/ignite-io`) from the clone it was installed from, so it works from
any directory as long as the clone stays where it is. Every command that reads a program
takes `--std DIR` to use another copy of the standard library.

## What a build needs at run time

`ignite check`, `plan` and `explain` need nothing else. Building a program does:

- **Rust target (the default).** `ignite build` writes a Rust project and runs `cargo build`
  on it, so cargo must be on the `PATH`. The first build of a project fetches the crates the
  IO substrate depends on.
- **LLVM target.** `ignite build --target llvm` never runs cargo or rustc for the program:
  clang compiles the generated LLVM IR and links it against the runtime archive
  `libignite_rt.a`, with the Linux system libraries. Build the archive once in the clone and
  pass it with `--runtime` (by default `ignite` looks for it next to its own binary):

  ```sh
  cargo build -p ignite-rt
  ignite build hello.ignite --target llvm --runtime ~/ignite/target/debug/libignite_rt.a
  ```

  Here `~/ignite` stands for your clone. [Building for Rust and LLVM](targets.md) has the
  details.

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
| `task compile` | build the terminal example into `target/ignite/terminal`; prints `built …/terminald` and `built …/tn` |
| `task compile-llvm` | the same with the LLVM target, into `target/ignite-llvm/terminal/bin/` |
| `task server`, `task client` | build and run `terminald` and `tn` (`PORT=n`, `TARGET=llvm`) |
| `task compile-todo`, `task todo-server` | build and run the todo HTTP backend `todod` on port 8080 |
| `task site`, `task site-serve` | build this website into `target/site`, and serve it on `http://127.0.0.1:8765/` |
