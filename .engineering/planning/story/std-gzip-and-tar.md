---
format: aep.planning-md/3
id: story:std-gzip-and-tar
kind: story
status: draft
title: gzip and tar in std
revision: 1
---
# Story: gzip and tar in std

## Outcome

A program reads and writes gzip streams (RFC 1952 over DEFLATE, RFC 1951) and tar archives (POSIX ustar with the pax extended headers for long names), written in Ignite, within stated limits.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: at v0.16.2 `std` has no compression and no archive format; the only way to either is to start `gzip` or `tar` through `std.process`.

## Acceptance

- `Gzip.decode(bytes, limits) -> Result<Bytes, GzipError>` inflates stored, fixed and dynamic blocks, checks CRC-32 and ISIZE, reads several members, and stops with an error once the output passes `limits.maximum_output` (a test with a 10 MB run of zeros compressed to under 20 kB pins it).
- `Gzip.encode(bytes, level)` produces a stream that `decode` and the system `gzip -d` both read; the design step states which levels exist and the size of each against `gzip -6` on three stated inputs.
- A reader form over `ByteStream` decodes without holding the whole input; the memory bound is stated and counted.
- `Tar.read` lists and extracts entries (regular file, directory, symbolic link as data, never followed); an entry name that is absolute or contains `..` is reported as such and never written by a helper that extracts to a `Directory`.
- `Tar.write` produces an archive that `tar -t` lists; long names use pax headers.
- The RFC 1951 and RFC 1952 requirements claimed are listed through `std.rfc`; `ignite conformance` reports them.
- `docs/layering.md` lists the modules; `task gate` green.

## Scope

- `std/compress.ignite` or `std/gzip.ignite` and `std/tar.ignite` (new), `std/rfc/` (RFC 1951, RFC 1952), `docs/layering.md`, `docs/slice.md`. Inferred.

## Not in this story

- zlib framing, zstd, brotli, zip.
- `Content-Encoding: gzip` in `std.protocol.http` (a later story on top of this one).
