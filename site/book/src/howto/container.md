# Build in a container

Each release from v0.13.0 on has a public image, `ghcr.io/ignite-lang/ignite-toolchain:X.Y.Z`.
It holds the pre-built `ignite` and its standard library, clang and lld, Rust 1.98 and the
release's source in `/usr/local/src/ignite`. The substrate's crates are fetched into the image
and `CARGO_NET_OFFLINE=true` is set, so builds on both targets need no network. The release
notes give the image's `@sha256:` digest. Pin that.

## Build a program

Mount the directory with your program at `/work`, the image's working directory. Run as your
own user, so the output belongs to you, and point the cache at a writable directory, where the
LLVM target writes its runtime archive:

```sh
docker run --rm --user "$(id -u):$(id -g)" -e XDG_CACHE_HOME=/tmp -v "$PWD:/work" \
  ghcr.io/ignite-lang/ignite-toolchain:0.16.4 ignite build hello.ignite
./target/ignite/hello/bin/hello
```

```text
built target/ignite/hello/bin/hello
Hello, Ignite!
```

Without `XDG_CACHE_HOME`, a run as a user other than root stops with exit 3: `the runtime
this ignite carries could not be written: Permission denied`.

## The Rust target

The image's `ignite` finds the substrate crate beside itself, in
`/usr/local/share/ignite/substrate`, where the release archive installed it (in images before
v0.16.1, in the release's source tree). So `--target rust` needs no `--substrate`. It builds
with no network:

```sh
docker run --rm --network none --user "$(id -u):$(id -g)" -e XDG_CACHE_HOME=/tmp -v "$PWD:/work" \
  ghcr.io/ignite-lang/ignite-toolchain:0.16.4 ignite build hello.ignite --target rust
```

```text
note: building the generated Rust project in target/ignite/hello; compiler messages refer to generated code
   …
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.99s
built target/ignite/hello/target/debug/hello
```

## Tests on both targets

```sh
docker run --rm --user "$(id -u):$(id -g)" -e XDG_CACHE_HOME=/tmp -v "$PWD:/work" \
  ghcr.io/ignite-lang/ignite-toolchain:0.16.4 ignite test hello.ignite --target all
```

```text
running 1 test
test hello/greet puts the name in the sentence ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.00s
running 1 test
test hello/greet puts the name in the sentence ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.01s
```

The first block is the LLVM target, the second the Rust target.

## Static executables

`--static` works in the image on the LLVM target, whose `ignite` carries the musl runtime:

```sh
docker run --rm --user "$(id -u):$(id -g)" -e XDG_CACHE_HOME=/tmp -v "$PWD:/work" \
  ghcr.io/ignite-lang/ignite-toolchain:0.16.4 ignite build hello.ignite --static
file target/ignite/hello/bin/hello
```

```text
built target/ignite/hello/bin/hello
target/ignite/hello/bin/hello: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, …
```

The image has no Rust musl target, so `--static --target rust` stops with exit 2:

```text
error: --static links against musl and needs the Rust target x86_64-unknown-linux-musl, which is not installed: install it with `rustup target add x86_64-unknown-linux-musl`
```

A static executable runs in an image that has no libc. [Static
executables](../guide/targets.md#static-executables) has the details.
