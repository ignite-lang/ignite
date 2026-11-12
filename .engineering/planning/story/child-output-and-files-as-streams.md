---
format: aep.planning-md/3
id: story:child-output-and-files-as-streams
kind: story
status: draft
title: A child's output and a large file are read as a stream
revision: 1
---
# Story: a child's output and a large file are read as a stream

## Outcome

A program reads the standard output of a child process and the content of a file piece by piece through a `ByteStream`, so neither is bound by the limit of one whole read.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which pages its listings to stay under the limits. A whole-file read stops at a fixed limit (`crates/ignite-io/src/fs.rs` `READ_LIMIT`). A child's collected output is bounded by `Invocation.output_limit`, at most `MAXIMUM_OUTPUT_LIMIT` (`crates/ignite-io/src/child.rs`, `std/process.ignite`); a started child offers `id`, `wait` and `kill` and no stream of its output.

## Acceptance

- A started child exposes its standard output (and standard error) as a `ByteStream`; a test reads more than the whole-read limit from a child in pieces with memory bounded by the piece size (allocations counted at two output sizes).
- A file opens as a `ByteStream` for reading; a test reads a file larger than `READ_LIMIT` the same way.
- Closing the stream early ends the read without leaving the child running: the child is killed or waited for, and the design says which.
- A child that writes more than the pipe holds while nobody reads blocks and does not lose bytes (a test with a slow reader).
- The limits of the whole-read forms are unchanged and stated in `site/book` beside the stream forms; the errors that report a limit name the stream form.
- Both targets and the evaluator, or the evaluator's refusal with its reason; `task gate` green.

## Scope

- `std/process.ignite`, `std/fs.ignite`, `crates/ignite-io/src/child.rs`, `fs.rs`, `docs/slice.md`. Inferred.

## Not in this story

- Raising or making configurable the limit of a whole read.
- Writing to a child's standard input as a stream, unless the design step finds it is the same work.
