# FAQ

## What is Ignite?

An intent-oriented language for specifying, building, deploying and operating systems. One
file describes the components, the network, the contracts and the policies, and the compiler
turns it into native executables. The same declaration is what `ignite plan` models,
`ignite run` supervises and `ignite apply` deploys. Start with [The idea](design/index.md).

## Can I use it in production?

No. Ignite is an early prototype. v0.8.0 runs systems on one machine, in the foreground
(`ignite run`) or under a local host (`ignite apply`); there is no cloud provider, no
portable bundle and no stateful migration. See the [roadmap](roadmap.md).

## How do I install it?

From source: clone the repository and run `cargo install --locked --path crates/ignite`.
There are no binary releases. [Install from source](guide/install.md) has the details.

## Which platforms does it run on?

The documents describe Linux: `ignite run` ties its children to itself with Linux's
`PR_SET_PDEATHSIG`, and the LLVM target's link line uses the Linux system libraries. Other
platforms are not covered by this book.

## Rust target or LLVM target?

Start with the default, LLVM. Both targets compile the same checked program, call the same
IO substrate and must behave the same, down to the text of an error; every feature lands on
both. The LLVM target builds without cargo and needs clang and the runtime archive;
`--target rust` builds with cargo instead. See
[Building for Rust and LLVM](guide/targets.md).

## Is the generated Rust meant to be edited?

No. It is an artifact of `ignite build`, rewritten on every build. Change the Ignite source.

## Where do Telnet and HTTP come from?

They are written in Ignite, in the standard library
([`std/protocol/`](https://github.com/ignite-lang/ignite/tree/main/std/protocol)). The
compiler and the IO substrate know sockets and byte streams, not application protocols. See
[Protocols and roles](design/protocols.md).

## Are `Restart`, `Availability` or `TelnetServer` keywords?

No. They are ordinary record types in the standard library, used as component, contract and
policy kinds. The compiler knows the declaration forms; the vocabulary is library code that
is checked and tested like yours. See the [standard library](reference/std/index.md).

## What happens when a contract has no data?

By default it is not reported as met. With `missing_data = "insufficient_evidence"`, the
default of `Availability` and `ThresholdCompliance` and the only rule of `SLA`, a contract with
no executed probe attempt, or with fewer than `minimum_completeness` of the expected ones, has
the standing `InsufficientEvidence`, not `Compliant`: zero traffic does not prove compliance.
A contract that declares `missing_data = "counts_as_good"` or `"counts_as_bad"` fills the
missing attempts instead. See [Contracts, signals and policies](design/contracts.md).

## Can a crash in one connection take the server down?

A failed `assert`, an integer overflow or an out-of-range index ends the task that hit it
and reports the file, line and column; the server keeps accepting. Overflow is a failure on
both targets, not a wrap-around.

## Can I test a protocol without opening a port?

Yes. `Pipe.pair()` returns two connected in-process streams, and the Telnet and HTTP roles
are generic over any `ByteStream`, so a test can run a whole session in memory, on both
targets. See [Testing with ignite test](guide/testing.md#protocol-tests-without-a-network).

## Why are some code blocks in this book marked "excerpt"?

Every complete Ignite block in this book is checked by `ignite check` in the repository's
test suite, so it compiles with the release it describes. Blocks that show a piece of a
larger file are marked as excerpts. See the [reference conventions](reference/index.md).

## Where is the exact specification?

[`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) is the
contract between the compiler's crates. The design document is
[`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md).
