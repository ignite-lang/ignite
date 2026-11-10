---
format: aep.planning-md/3
id: story:tls-engine-own-crates
kind: story
status: draft
title: A program that opens no TLS session links no part of the TLS engine, debug info included
revision: 1
---
# Story: a program that opens no TLS session links no part of the TLS engine, debug info included

## Outcome

The runtime's TLS code lives in crates of its own, reached only through the `ignite_rt_tls_*`
entry points, so a dev binary on the LLVM target of a program that opens no TLS session carries
no rustls, ring or webpki compile unit, and the dev sizes of row 3.2 return to what they were
before TLS client sessions.

## Why

Measured at the v0.16.0 release cut: `3.2 llvm/terminald/dev` is 15,560,616 bytes against the
bar 9,144,568 and `3.2 llvm/todod/dev` 16,272,232 against 9,849,080. The linker pulls in a whole
runtime object when it needs one symbol from it, and `--gc-sections` removes unused code but
keeps that object's debug info. In the dev runtime archive the TLS code shares objects with code
every program calls: from the linker map, `ignite_rt_int_to_text` reaches `ignite_io::parse_int`,
whose object holds a shared `LocalKey::with` instance and a reference to a rustls table, so 16
rustls, 16 ring and 9 webpki objects are linked. The dev `terminald` holds 193 rustls, 163 ring
and 51 webpki compile units. The same objects linked against the runtime before TLS give
9,287,136 bytes. Tried and short of the bar: `debug = false` for the nine engine crates
(10,732,272), that plus ring's C and assembly debug info removed (9,760,088), more codegen units
(10,966,792): rustc places generic helpers instantiated with TLS closures in shared objects.

## Acceptance

- A test builds a dev LLVM binary of a program that opens no TLS session and fails when it holds
  a rustls, ring or webpki compile unit.
- `task quality` reports `3.2 llvm/terminald/dev` and `3.2 llvm/todod/dev` within 5 % of the
  sizes before TLS client sessions (9,144,568 and 9,849,080), and the decision that moved the two
  bars for v0.16.0, if one was recorded, is withdrawn.
- A program that opens a TLS session builds and passes the inline tests of `std/tls.ignite` on
  both targets.
- `docs/layering.md` and its test name the new crates.

## Scope

- Own: `crates/ignite-io/src/tls.rs` and the TLS entry points of `crates/ignite-rt` (moved to
  crates of their own), the Rust target's code generator where it names `::ignite_io::Tls…` and
  the `tls` feature, `docs/layering.md`, the layering test, `docs/slice.md` 38. Inferred.
