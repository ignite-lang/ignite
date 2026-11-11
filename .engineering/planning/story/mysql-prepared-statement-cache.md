---
format: aep.planning-md/3
id: story:mysql-prepared-statement-cache
kind: story
status: draft
title: A pooled MySQL connection keeps its prepared statements
relations:
- informed_by: story:std-mysql-client
revision: 1
---
# Story: a pooled MySQL connection keeps its prepared statements

## Outcome

A statement that a program runs again on a pooled connection of `std.protocol.mysql` is prepared once on that connection: one round trip per run instead of three (prepare, execute, close).

## Why

The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS) counted 6 round trips to the database per request where the same service in Rust makes 3: each of its statements is prepared, executed and closed per request (reported 2026-11-11). It is adding a cache of its own.

## Acceptance

- A cache per connection, keyed by the SQL text, bounded (a limit in the pool's options, the least recently used statement closed first), dropped with the connection; a reconnect prepares again.
- Scripted-server tests on both targets: the second run of a statement sends one command; the bound closes the oldest statement; a statement the server no longer knows (error 1243) is prepared again once and the run repeated when it changed nothing.
- `docs/slice.md` section 42 states the behaviour and the limit; a program can switch the cache off.
- `task gate` green.

## Not established

- The server's `max_prepared_stmt_count` across many pools; whether metadata changes (DDL) need the statement prepared again.

## Scope

- `std/protocol/mysql.ignite` (pool, prepared statements), `docs/slice.md` 42. Inferred.
