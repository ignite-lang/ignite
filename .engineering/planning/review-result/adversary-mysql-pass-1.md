---
format: aep.planning-md/3
id: review-result:adversary-mysql-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit mysql (wave 11)
relations:
- reviews: story:std-mysql-client
revision: 1
---
red

Adversary pass 1 on unit mysql (`story:std-mysql-client`), attacked at `700f089e`; the pass's cases are commit `77284900` (`crates/ignite/tests/mysql_adversary.rs`, appended tests in `std/protocol/mysql.ignite` and `std/protocol/mysql/binary.ignite`).

verdict: red; cases executed 55→58 inline per target plus 2 cargo tests holding 8 Ignite cases per target, red 3 inline and 2 cargo; origin: introduced 4, pre-existing 1.

Findings:
1. `std/protocol/mysql.ignite:789` (also 1005, 1122), CONFIRMED, introduced, blocker. A command of two frames (payload of 16 MiB − 1 bytes or more, which the 64 MiB default limit allows) is refused with `OutOfOrder(1, 2)` and its connection closed: `simple`, `results` and `prepared` read the reply at the literal sequence 1, the server answers at 2. Red on both targets, inline and against MySQL 8.0.
2. `std/protocol/mysql/binary.ignite:215` (also 235), CONFIRMED, introduced, warning. A DOUBLE does not print alike from `query` and from `execute`: `notation` switches to an exponent at 1e21 and below 1e-6 where the server switches at 1e15 and prints plain down to 1e-15, and `shortest` breaks a digit tie upward where the server breaks to even (1000000000000000.3 against .2). 10 of 17 values differ against MySQL 8.0.
3. `std/protocol/mysql.ignite:735`, CONFIRMED, pre-existing, blocker. On the LLVM target every connection keeps its descriptor until the process exits: 7 before, 47 after 40 connections that quit, were refused or were refused TLS; 9 to 129 for 40 pools of 3; the Rust target returns to its start. The mechanism is the LLVM owner rule (`docs/llvm.md` section 4: a binding that is returned or stored in a field is owned by no block) and `TcpStream.close()` shutting only the write side; the unit changes no compiler or runtime file and a probe without the unit grows the same way.
4. `std/protocol/mysql.ignite:247`, CONFIRMED, introduced, note. `describe(UnsupportedPlugin(name))` holds the server-chosen plugin name whole: line breaks, escape sequences, any length up to the packet limit.
5. `std/protocol/mysql/binary.ignite`, INFEASIBLE, introduced, note. `date_text` and `time_text` print a microsecond field of 1,000,000 or more as its first six digits and an out-of-range month, day or hour as sent; only a broken server sends these, no panic, both targets alike.
6. `std/protocol/mysql.ignite`, CONFIRMED, introduced, note. 6 of 15 mutants survive the 47 inline tests: `prepare`'s column limit off by one, the server's `CLIENT_SECURE_CONNECTION` flag not required, the bound of 8 authentication packets removed, bytes after the authentication accepted, a failed handshake leaving the stream open, a NUL in the database name accepted.

Attacked and sound on both targets: every strict prefix of handshake, OK, ERR, column definition, text row and binary row; length prefixes 0xfb to 0xff; column counts 0, 4,096, 4,097 and 2^63; a field length past its packet; a NULL bitmap one byte short; the sequence wrap over 600 rows; endless authentication chains (cut at 8); scramble lengths 0, 8 and 255; 13 orderings of switch and more-data on a plain stream without the password in the bytes written; `mysql_clear_password` refused; `connect_tls` against a server without TLS, 40 times; parameters with NUL, quote, backslash, 0xff, empty text, Int min and max; 34 expressions of dates, times, decimals, JSON and geometry alike from `query` and `execute` against MySQL 8.0; 640 FLOAT and 639 of 640 DOUBLE bit patterns.

```findings
[
{"file":"std/protocol/mysql.ignite","line":789,"category":"wrong-result","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"a command of two frames (payload of 16 MiB - 1 bytes or more) is refused with OutOfOrder(1, 2) and its connection closed: simple (789), results (1005) and prepared (1122) read the reply at the literal sequence 1, the server answers at 2; red inline and against MySQL 8.0 on both targets"},
{"file":"std/protocol/mysql/binary.ignite","line":215,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a DOUBLE does not print alike from query and execute: notation (235) uses the exponent bounds 1e21 and 1e-6 where MySQL uses 1e15 and 1e-15, and shortest (215) breaks a digit tie upward where the server breaks to even; 10 of 17 values differ against MySQL 8.0"},
{"file":"std/protocol/mysql.ignite","line":735,"category":"leak","severity":"blocker","verdict":"CONFIRMED","origin":"pre-existing","message":"on the LLVM target every connection keeps its descriptor until the process exits (7 before, 47 after 40 connections; 9 to 129 for 40 pools of 3; the Rust target returns to its start): the LLVM owner rule gives a binding that is returned or stored in a field to no block, and close() shuts only the write side; the unit changes no compiler or runtime file and a probe without the unit grows the same way"},
{"file":"std/protocol/mysql.ignite","line":247,"category":"log-injection","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"describe(UnsupportedPlugin(name)) holds the server-chosen plugin name whole: line breaks, escape sequences, any length up to the packet limit"},
{"file":"std/protocol/mysql/binary.ignite","line":1,"category":"robustness","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"date_text and time_text print a microsecond field of 1,000,000 or more as its first six digits and an out-of-range month, day or hour as sent; only a broken server sends these, no panic, both targets alike"},
{"file":"std/protocol/mysql.ignite","line":1,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"6 of 15 mutants survive the 47 inline tests: prepare's column limit off by one, the server's CLIENT_SECURE_CONNECTION flag not required, the bound of 8 authentication packets removed, bytes after the authentication accepted, a failed handshake leaving the stream open, a NUL in the database name accepted"}
]
```
