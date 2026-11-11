# FAQ

## What is Ignite?

An intent-oriented language for specifying, building, deploying and operating systems. See
[The idea](design/index.md).

## Can I use it in production?

No. Ignite is an early prototype, at v0.16.0, and runs systems on one machine only. See
[What works today](roadmap.md#what-works-today).

## How do I install it?

Unpack the release archive on Linux x86_64, or build from source with cargo. See
[Install](guide/install.md).

## Which platforms does it run on?

Linux. `ignite run` ties its children to itself with Linux's `PR_SET_PDEATHSIG`, and the LLVM
target links against the Linux system libraries. Other platforms are not covered by this book.

## Rust target or LLVM target?

Start with the default, LLVM. Both targets behave the same, and every feature lands on both.
See [One substrate, two targets](design/substrate.md) for why there are two, and
[Build for the Rust target](guide/targets.md) for how.

## Is the generated Rust meant to be edited?

No. `ignite build` rewrites it on every build. Change the Ignite source.

## Where do Telnet and HTTP come from?

They are Ignite code in the standard library,
[`std/protocol/`](https://github.com/ignite-lang/ignite/tree/main/std/protocol). See
[Protocols and roles](design/protocols.md).

## Are `Restart`, `Availability` or `TelnetServer` keywords?

No. They are record types in the standard library. See the
[standard library](reference/std/index.md).

## What happens when a contract has no data?

By default its standing is `InsufficientEvidence`, not `Compliant`. See
[Missing data](reference/std/sli.md#missing-data) in `std.sli`.

## Can a crash in one connection take the server down?

No. A failure ends the task that hit it, and the server keeps accepting. See
[Failures](reference/tests.md#failures).

## Can I test a protocol without opening a port?

Yes, over `Pipe.pair()`. See [Pipe and PipeStream](reference/std/net.md#pipe-and-pipestream)
and [Write a protocol and test it over a pipe](howto/protocol.md).

## Why are some code blocks in this book marked "excerpt"?

An excerpt is part of a larger file. Every complete block is checked. See the
[Welcome](index.html#how-this-book-is-organised) page.

## Where is the exact specification?

[`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) is the
contract between the compiler's crates. The design document is
[`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md).
