---
format: aep.planning-md/3
id: story:rust-target-text-literals-and-field-copies
kind: story
status: active
title: A text literal and a text copied into a field allocate nothing on the Rust target
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a text literal and a text copied into a field allocate nothing on the Rust target

## Outcome

On the Rust target a text literal in a value position (a key of a pair, a payload, a record field, a list element) allocates nothing, and a `Text` copied into a record field, a tuple or a payload from a value the function only lends costs a count and no allocation.

## Why

Reported on 2026-11-12 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.2) from an allocation profile of one request. Ranked by allocations per request, the two forms of this story stand above every form an accepted story covers: text literals built in value position come first in the small answer and third in the largest; the copy of a column's name into each cell of a row comes first in the largest answer. `story:rust-target-text-reads-borrow` covers the reads that copy and the parameters that could be moved into; it leaves these two out ("Sharing the storage of `Text`", Not in this story).

A synthetic program of 65 lines, `ignite emit` at `1febe156` (v0.16.2 plus documents):

1. A literal in a value position. `Rec([("id", Num(id)), ("title", Txt(title)), ("state", Txt("open"))])` is emitted as `Val::Rec(::ignite_io::List::from_vec(vec![(String::from("id"), Val::Num(id.clone())), (String::from("title"), Val::Txt(title)), (String::from("state"), Val::Txt(String::from("open")))]))`: one allocation per literal per row object, four here, for texts whose bytes are in the executable.
2. A literal passed to a read-only parameter is already built once: `find(fields, { static TEXT: ::std::sync::LazyLock<String> = ..; &*TEXT })`. A literal compared is already free: `(*state) == "open"`. Neither is in this story.
3. A text copied into a field. `out.push(Cell { name = c.name, kind = c.kind, value = values[i] })` over a lent `cols: List<Col>` is emitted as `out.push(Cell { name: c.name.clone(), .. })`: one allocation and a copy of the name per cell, where every cell of a column holds the same name.

`story:rust-target-shared-list-storage` names `Text` in its Outcome and delivered lists (`::ignite_io::List<T>`, PR #143). Its release run is the caution for this story: the shared lists made p99 of `todo/patch-16` on the Rust target 4.4 % worse and the stripped binaries larger (`story:shared-list-costs-back`). A `Text` is in more places than a list.

## Acceptance

- A count test with the synthetic program's functions (`row_object`, `cells`), in the manner of `shared_lists::` and `constants::`, red first: the allocations per call at two sizes on the Rust target. `row_object`: none for the three keys and the literal payload (today four). `cells`: none for the name per cell (today one).
- The design is written down before any generator change, with the alternatives counted on the synthetic program and on `examples/todo` and `examples/terminal`: (a) a text type of the substrate that holds either a `&'static str` or counted storage, used where `::ignite_io::List<T>` is used (fields, payloads, tuple elements, list elements), `String` elsewhere; (b) the same type everywhere; (c) literals only (a field type that can borrow a `'static`), copies left as they are. For each: allocations per request of both examples (`reply_cost::`), instructions per request (row 3.10's cells), stripped size of both servers (row 3.9's method), and the conversions it adds at the boundary between a local `String` and a field. The owner's yes on the design is needed before it reaches `main`, as for the shared lists.
- A text that is changed after a copy (`var t = r.name; t = t + "x"`, `r.name.push(..)` forms the language has, a field assigned, a text taken out of a field into a local and appended to) gives the evaluator's value on all three; a literal is never changed in place.
- `Text` as a `Map` key, in `==`, in ordering, in interpolation, in `to_bytes`, `len`, `slice`, `split`, `concat` and every other builtin gives the same bytes as before; `std` builds; hashing a shared and an unshared text of equal bytes is equal.
- No new `unsafe` site in generated code; the substrate's count of them (`quality::`) is unchanged or the new site is named with its proof.
- Row 3.10 (`task path-cost`): no cell of `todo` and `terminal` above its count at the last release. The dedicated-host run shows no row worse than at the last release, 3.7 `todo/patch-16` and 3.9 included; a row that is worse sends the design back, it is not tracked.
- The LLVM target's count for the same functions is written down; a fix there is a separate story if it copies.
- `CODEGEN_REVISION` is bumped; `task gate` green.

## Scope

- `crates/ignite-io/src` (the text type, beside `list.rs`), `crates/ignite-rustgen/src/lib.rs` (literals in value position, field and payload types, the copies into them, the reads out of them), `crates/ignite-rustgen/tests/golden/`, `crates/ignite/tests/` (count test), `docs/slice.md` section 6, `docs/value-copies.md`. Inferred.

## Not in this story

- Reads that copy and parameters that could be moved into (`story:rust-target-text-reads-borrow`).
- `Bytes` and `Map` storage (`story:rust-target-shared-list-storage` names them; no profile ranks them yet).
- An allocator for the Rust target other than the system's.
