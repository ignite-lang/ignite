---
format: aep.planning-md/3
id: story:llvm-temporaries-without-owner-rule
kind: story
status: draft
title: Five kinds of store read on LLVM get an owner rule
relations:
- informed_by: story:llvm-release-handle-temporaries
revision: 1
---
# Story: five kinds of store read on LLVM get an owner rule

## Outcome

On the LLVM target, each of five kinds of temporary that reads a handle from a store is released once: no handle is left unreleased.

## Why

Adversary pass 1 on unit handles of wave 11 (`review-result:adversary-handles-pass-1`, finding 4), `crates/ignite/tests/soak/temporaries_unreleased.ignite:78`, pre-existing: red on the unit's base and at its head, green on the Rust target. The five reads without an owner rule:

- the receiver of a builtin method (`senders.values().len()`),
- the rest of a fresh value read for one field (`open(s).id`),
- a thrown-away match arm value,
- a store read inside a record argument of a call,
- a match binding put in a store wrapped in `Some`.

v0.15.0 was not measured.

## Acceptance

- The soak `temporaries_unreleased.ignite` is green on LLVM for all five reads and stays green on Rust.
- One inline test per read in the differential corpus; row 1.1 stays 0.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-llvm/src/own.rs`, `crates/ignite-llvm/src/lower.rs`, `crates/ignite/tests/soak/`. Inferred.
