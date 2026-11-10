---
format: aep.planning-md/3
id: story:typer-record-literal-deferred
kind: story
status: draft
title: A record literal argument is typed after the arguments that bind its parameters
summary: Record literal arguments are deferred to the second typing pass, as closures are.
revision: 1
---
# Story: A record literal argument is typed after the arguments that bind its parameters

## Outcome

A call `run(Box { state = "s" }, 7)` with `run<T, S>(boxed: Box<S, T>, stream: T)` types: the record literal waits for the second pass, as a closure argument does.

## Why

A record literal gets its type parameters only from the arguments before it (`crates/ignite-types/src/typer.rs:617`); the call above gives E042 for `S` of `run` and `T` of `Box`. Found by `review-result:adversary-http-app-pass-1`, F2; it existed before that unit.

## Acceptance

- The ignored case `adversary_http_app::a_record_literal_before_the_argument_that_binds_its_parameter` in `crates/ignite-types/tests/adversary_http_app.rs` runs and passes.
- A record literal whose parameter no argument binds still gives E042.

## Scope

- Own: `crates/ignite-types/src/typer.rs`. Inferred.
