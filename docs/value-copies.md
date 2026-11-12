# Value copies on the Rust target: counts by form and four designs

Design note, 2026-11-12, at `1dfc2937` (stories `rust-target-shared-list-storage` and
`rust-target-read-only-bindings-borrow`). No compiler change is merged with it. Every number is a
count (allocations and bytes from valgrind memcheck, instructions from callgrind, allocation sites
from DHAT); nothing here is a timing.

Status: design D is built (unit `rust-shared-payload-lists`, `docs/slice.md` section 6) with
the two-block layout of the prototype. Its measured counts replace the prototype's in sections
2 to 4 where they differ; the rows of the other designs are the prototypes'.

## Recommendation

1. Share the list storage of enum payloads and record fields by reference count (design D below),
   then return lent results from lookups whose callers only read them (design C1).
2. On the synthetic workload, D alone takes a request from 250.6 M to 17.5 M instructions and from
   607,779 to 51,172 allocations (measured, as built); D with C1 to 8.2 M and 5,407 (prototype). By-value parameters (B) make it
   worse (295.1 M), because the filtered lists are live.
3. Order: the counting harness, D, C1; views (C2) and shared text only if the counts left after C1
   ask for them (section 4.3).

## 0 What is being copied

The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS)
measured, after every copy a borrow or a move removes was gone: 74 % of instructions in libc
(memcpy, malloc, free); 66 % in one pattern, a filter over a live list captured by a closure (each
parent row filters the same list again), keeping about 99 % of its rows, whose result is at once
filtered again (about 1 % kept) and dropped; about 10 % in lookups whose result is only read. Of 95
filter call sites 71 have a live source. Of the lookup sites about half only read the result
(sometimes a whole row set), half store one small field.

The forms (numbering of the reporter's reproducer):

| form | source | emitted at `1dfc2937` |
|---|---|---|
| 1 | `for r in l { if keep(r) { out.push(r) } }` over a lent `l` | `out.push(r.clone())` |
| 2 | `for e in l { if e.0 == k { return Some(e.1) } }` | `return Some(e.1.clone())` |
| 3 | a lent parameter or a match payload returned whole | `Val::Seq(l) => l.clone()`, `v.clone()` |
| 4 | an element or a lent parameter handed to a by-value parameter | `f(r.clone())`, `put(out, .., e.1.clone())` |

None is a copy made by mistake: under value semantics the list keeps its element and the result
needs one. The question is what representation or calling mode makes the copy cheap or
unnecessary.

## 1 The workload

[`crates/ignite/tests/fixtures/value_copies.ignite`](../crates/ignite/tests/fixtures/value_copies.ignite)
since unit `value-copies-harness` (section 4.4; written as `docs/value-copies/workload.ignite`
for this note): its two inline tests are units of the differential corpus, and
`crates/ignite/tests/value_copies.rs` appends the executables `s1-r1` and `s1-r2` and counts one
request at s1 on the Rust target with memcheck on every merge.

- Data: three tables of rows, parents, middles (each under one parent) and children (each under
  one middle). A row is a dynamic record, `Rec(List<(Text, Val)>)`, as the reporter's are: an
  `id`, a link (`kind`, `parent` or `middle`), a two-valued `state`, a `name`, text fields `f4` to
  `f7` (fields F = 8), an `amount` and `attrs`, a nested list of depth D = 3. One row copy is 24
  allocations (the field list, 10 names, 6 texts, 7 for `attrs`).
- Sizes: s1 = 33 parents, 128 middles, 222 children (the reporter's largest answer); s10 = ten
  times each. Rows R, fields F and depth D are parameters (`Sizes`).
- The answer (`answer`): `each(db.parents, |p| ..)` builds one record per parent. Its closure
  reads the lent tables `db.middles` and `db.children`, so every parent filters the same live
  list again: `keep(db.middles, "parent", pid)` (keeps 3.1 %), `keep(mine, "state", on)` (80 %),
  and per middle `keep(keep(children, "state", on), "middle", mid)`: the inner filter keeps
  219 of 222 (99 %), its result is filtered at once (keeps 1 to 2) and dropped. Results are
  plucked (`pluck`), counted, iterated (`each`, `lent_reads`), taken first (`mine[0]`), never
  stored as they are.
- Lookups (`field`, `get`, `has`, `num_of`): read only in `keep`'s comparison, `has`,
  `num_of`, `size(get(m, "attrs"))` and `rows_of(get(p, "attrs")).len()` (a whole nested list,
  only measured); stored for `name`, `state`, `id`.
- Forms 3 (`rows_of`, `or_nil`) and 4 (`each`'s fn value, `put` in `renamed`); grouping in a
  `Map<Text, List<Val>>`; and the forms `mysql-cost` already lends (`lent_reads`: a tuple
  pattern in a `for`, `size(rows[i])`, `match rows[i]`, `rows[i] == Nil`).
- `answer_lean`: the same answer from the same filters per row, written with the source forms of
  section 5 only.
- The system `ValueCopies` the note's file ended with builds eight executables: `s1-r1`,
  `s1-r2`, `s10-r1`, `s10-r2` and the same four with `lean-`. `rN` answers N requests over one
  set of tables, so `r2` less `r1` is one request without building the tables. The harness
  appends the first two.

Same output on every consumer: both inline tests pass on the Rust target, the LLVM target (`ignite
test --target rust|llvm`) and the evaluator (run as a unit of the differential runner on a
temporary copy in the fixtures directory, 2026-11-12: `eval` passed, Rust and LLVM status 0 for
both tests; the copy was removed). The executables print the same digests on both targets:
48625856 (s1, one request), 556027385 (s1, two), 396786190 (s10, one), 697157996 (s10, two), for
`answer` and `answer_lean` alike.

How the counts were taken, all release builds (`ignite build --target rust|llvm --release`; the
Rust build with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` for the sites, which changes no
code): allocations and bytes from the `total heap usage` line of `valgrind --tool=memcheck`,
instructions from the totals of `valgrind --tool=callgrind`, both per request as `r2` less `r1`;
allocation sites from `valgrind --tool=dhat`, each block assigned to the innermost frame of the
generated crate. Two s10 rows are `r1` less ten times the s1 table build (marked `*`). The
generated Rust carries `ignite_io::census::Counting` as its global allocator, as every program
does.

## 2 Counts at `1dfc2937`

### 2.1 Per request

| target | size | allocations | bytes | instructions |
|---|---|---|---|---|
| Rust | s1 | 607,779 | 24,044,531 | 250,639,812 |
| Rust | s10 | 58,808,402 | 2,434,980,694 | 24,217,181,005 |
| LLVM | s1 | 0 | 0 | 200,748,838 |
| LLVM | s10 | 0 | 0 | 23,066,659,261 |

The work grows with parents × middles and middles × children, so s10 is about 97 times s1.

Where the Rust target's instructions go (callgrind, inclusive, s1): `free` 37.3 %, `malloc`
35.3 %, `memcpy` 3.3 %: 75.9 % in the allocator and `memcpy`, the reporter's 74 %.

### 2.2 By form, Rust, s1, per request

Rows copied and allocations per site (DHAT, one row = 24 allocations):

| form | site | rows | allocations |
|---|---|---|---|
| 1 | `keep`: inner filter by `state` over the live `children` (99 % kept) | 22,338 | 536,112 |
| 1 | `keep`: outer filter of that result by `middle` (dead source, 1 to 2 kept) | 174 | 4,176 |
| 1 | `keep`: `db.middles` by `parent` (live, 3.1 %) | 128 | 3,072 |
| 1 | `keep`: `mine` by `state` (live, 80 %) | 102 | 2,448 |
| 1 | growth of the four result lists | | 881 |
| 2 | `field`, result only read (compared in `keep`, `has`, `num_of`, `size`, `len`) | | 23,834 |
| 2 | `field`, result stored (`name`, `state`) | | 644 |
| 2 | `Some(want.clone())`: the operand `keep` builds to compare with | | 22,772 |
| 3 | `rows_of` (231), `or_nil` (33) | | 264 |
| 4 | `each`: `f(r.clone())` over parents, active middles, kept children | 309 | 7,416 |
| 4 | `renamed`: `put(out, .., e.1.clone())`, the whole answer record once more | | 2,164 |
| | `let first = mine[0]`, only read (story `rust-target-read-only-bindings-borrow`) | 33 | 792 |
| | `Map` grouping: `get(..).cloned()`, the arm's `g.clone()`, `for (k, v) in groups.clone()` | | 799 |
| | building the answer (record literals, keys, pushes) | | 2,405 |
| | total | | 607,779 |

Form 1 is 89.9 % of the allocations and 96.7 % of the bytes; the inner 99 % filter alone is
88.2 % of the allocations. Form 2 is 7.8 % with its comparison operand. Two copies in the table
are not forms 1 to 4 and look like plain overhead: `Some(want.clone())`, a lent parameter copied
only to be compared, and the `Map` arm `Some(g) => g.clone()` that copies an owned binding a
moment before it dies. Instructions are not split by site: the hot functions are inlined into
each other, so callgrind's per-function costs do not follow the forms. Section 3.5 splits them by
removing one form at a time.

### 2.3 The LLVM target

The LLVM target copies at the same sites (the moves and lending rules of `docs/slice.md` 20.3 are
read by both backends), with its own mechanism (`docs/llvm.md` section 4): the copy glue
`copy.<T>` walks the value and copies every buffer into the task's arena with a bump allocation,
and nothing is freed one by one: a loop's region is released at its back-edge. malloc sees none of
it (memcheck counts the same 24 blocks with 1 and 2 requests), and the arena's chunks are
mappings, not heap blocks, so the byte column is 0. What remains is the walk and the `memcpy`:
200.7 M instructions per request at s1, 80 % of the Rust target's. Copies by form were not
counted on this target: a release executable is linked with `--discard-all`, so callgrind names
no internal function.

## 3 Designs

Each design was counted on the generated Rust of the workload edited by hand to the code the
backend would emit, built in the generated project with its own `Cargo.toml` (release, LTO, one
codegen unit). None was prototyped in `crates/ignite-rustgen`. The edits were made to the Rust
generated for the workload before `answer_lean` and its helpers were added; the functions they
share are emitted alike (the final file's `1dfc2937` build counts the same allocations and bytes,
and 4,226 more instructions per request at s1, which is the figure in 2.1). Section 3.5 has every
count.

### 3.1 A: reference-counted list storage

A `List<T>` other than `Bytes` is `::ignite_io::List<T>`: a pointer to one block holding the count,
the length, the capacity and the elements; the empty list is no block. Clone is a count
increment; drop a decrement, the last one drops the elements and frees the block. Reading
derefs to `&[T]`, so `checked::at`, `slice_ref`, `iter()`, `len()` and the equality and debug glue
keep their slice signatures. Every in-place change goes through `make_mut`: when the count is 1 it
changes in place, otherwise it first copies the elements once (each element clone, which is itself
an increment when the element holds a list). `push`, `append`, `clear` and `l[i] = v`
(`checked::at_mut(l.make_mut(), ..)`) are the in-place changes; the type has no `DerefMut`, so a
change that bypasses `make_mut` does not compile. A `for` by value takes the elements when the
count is 1 and clones them otherwise (`into_iter`).

- `spawn` and channels: a value that crosses either must be `Send`. The count is atomic (`Arc`),
  which is what the prototype counts. The alternative, a plain count and a deep copy at the
  boundary (generated per type, as the copy glue is on LLVM), would keep the atomic operations off
  the paths that never share but make every `send` and `spawn` copy as today. Instruction counts
  do not show what a locked increment costs against a plain one; that needs a timing run on the
  dedicated host.
- `checked::at` and in-place changes: unchanged signatures (slices); `at_mut` takes the result of
  `make_mut`. A uniqueness test per `push` (one load and a compare) is the only cost on a path that
  never shares.
- FFI of `ignite-io`: every substrate function that takes or returns a `Vec<T>` of a list type
  converts: `text::pieces` and `checked::slice` (return `Vec<String>`), `Store<T>` values, channel
  payloads, process arguments and `Invocation` lists. Beyond the type substitution, the
  prototype on `todod` and `terminald` needed hand edits where a substrate call returns a
  `Vec<String>` (`text::pieces`, `checked::slice`), where the generated `main` names a fn type
  over a list, and where a list literal of bytes is a `Bytes`; `Bytes` (`Vec<u8>`) stays a
  `Vec`.
- The prototype (`Option<Arc<Vec<T>>>`) has two blocks per non-empty list (the `Arc` box and the
  buffer) and two indirections per read. The design has one of each; the difference is counted
  where it matters (section 3.5, 4.1).
- `Text` and `Map` separately: not prototyped. A shared `Text` (count, length and bytes in one
  block, short texts inline) would turn the text copies left after A into increments: on the
  workload 46,404 of A's 51,370 allocations per request are text copies. `Map` is a
  `BTreeMap`; sharing it needs a persistent or copy-on-write map, not counted here.

### 3.2 B: parameter and iteration modes

A parameter of type `List<T>` whose elements leave the function (pushed into the result,
returned, passed to a by-value parameter) is taken by value and iterated by value (`for r in rows`,
`out.push(r)` moves). A caller passes a list that is dead after the call by moving it (the moves
analysis, including the late moves of unit `call-moves`) and copies one that is live. `keep` and
`each` take their lists by value; `field` keeps its lent row (its row is a `Val`, not a list, and
taking it by value would copy a whole row per lookup to save one field).

What it does to the reporter's pattern: the inner filter's source is the live capture `children`,
so the caller copies all 222 rows to keep 219: 22,644 rows copied instead of 22,338 (543,456
allocations instead of 536,112). The outer filter's source is the inner result, a temporary, so
it moves: 174 rows saved. The case that gets worse is the filter keeping few rows of a list used
again: `keep(db.middles.clone(), "parent", pid)` copies 128 rows per parent to keep 4, 4,224 rows
per request instead of 128 (101,410 allocations at that site against 3,105). `each(active, ..)`
and `each(kids, ..)` move (276 rows saved); `each(db.parents, ..)` still copies.

### 3.3 C: lent results

C1, a lent single result: a fn whose every return value is a place rooted at a parameter it does
not consume (a field, an element, a match payload, a binding of a lent `for`), or a constant with
no buffer (`Nil`, `None`, a `static`), returns a reference (`fn field<'a>(v: &'a Val, k: &String)
-> Option<&'a Val>`, `fn get<'a>(..) -> &'a Val`, `fn rows_of<'a>(v: &'a Val) -> &'a [Val]`). A
caller that only reads it (an operand of a comparison, a `match` whose arms only read, an argument
to a lent parameter, the receiver of a read method, a `let` only read) takes the reference; a
caller that stores it copies at the call (`get(c, "name").clone()`). A comparison with a lent
parameter compares in place: `match field(r, k) { Some(x) => eq(x, want), None => false }`
instead of building `Some(want.clone())`.

- Through `Option` and `Result`: `Option<&T>`, `Result<&T, E>` with `E` owned; through tuples
  `(&A, B)` when the lent part is a place. Each mix is another signature, so the rule is
  per fn, not per return path: one owned path makes the fn return owned values.
- Through a fn value: no gain. A fn value's parameters and result are owned (`docs/slice.md`
  13.3); a lent-result fn used as a value is wrapped to return a copy, `|a| f(&a).clone()`, and a
  call through a fn value returns an owned value as today.
- The borrow must hold until the last read: the analysis must show that nothing changes the
  source while the result is live, as the element-read rules of 20.3.7 do; otherwise the caller
  copies. rustc checks it, and a program the checker accepts must still compile (vision X23).

C2, views: a fn that returns a `List<T>` built only of elements of a lent list, all of whose
callers only read the result, returns `Vec<&'a T>`. Every fn that takes such a list must accept
both kinds of element (`fn keep<'a, R: Borrow<Val>>(rows: &'a [R], ..) -> Vec<&'a Val>` in the
prototype), so a view is a second representation of `List<T>` that fn signatures carry. The
prototype hit E0716 at `keep(keep(children, ..), ..)`: the inner view is a temporary that the
outer view borrows through; the backend has to bind it in the enclosing block, as late moves bind
their arguments.

Counted: C = C1 and C2 together, and C1 alone.

### 3.4 D: shared lists in payloads and fields only

As A, but only for a list that is the payload of an enum variant or a field of a record
(`Seq(List<Val>)`, `Rec(List<(Text, Val)>)`, `type R { items: List<T> }`). Local lists,
parameters and results stay `Vec`. Constructing a payload or field wraps the `Vec`
(`Val::Rec(List::from_vec(fields))`); reading one derefs to a slice; a payload taken out into a
`Vec` position copies its elements (`l.to_vec()`, each element an increment when it holds a list);
changing a field in place goes through `make_mut`. A row of the workload is a `Rec`, so copying a
row is one increment, wherever the row goes. Paths that build lists in local variables and never
put them into a value pay nothing, unlike A, which tests uniqueness on every `push`.

As built: a payload taken out into a `Vec` position is `List::into_vec`, which takes the `Vec`
without a copy when nothing else shares it and copies it otherwise; `Bytes` (`List<Byte>`)
stays `Vec<u8>`. The layout is the prototype's, two blocks, with no `unsafe`
(`docs/slice.md` section 6).

Counted alone and with C1 and C (D+C1, D+C).

### 3.5 Counts

Per request, Rust target (`*`: `r1` less ten times the s1 table build). The D rows are the
built unit's release executables at `ef69b544` plus the unit, against `ef69b544`'s own 607,784
allocations, 24,044,784 bytes and 250,639,812 instructions at s1, and 58,808,402,
2,434,980,694 and 24,217,202,415 at s10 (memcheck moves a run by up to 5 allocations); the
prototype counted 51,167 allocations and 18,104,987 instructions at s1, 1,608,110,309
instructions at s10:

| design | size | allocations | bytes | instructions | instructions against `1dfc2937` |
|---|---|---|---|---|---|
| `1dfc2937` | s1 | 607,779 | 24,044,531 | 250,639,812 | 100 % |
| A | s1 | 51,370 | 1,593,878 | 18,898,670 | 7.5 % |
| B | s1 | 703,420 | 28,672,785 | 295,062,959 | 117.7 % |
| C (C1 and C2) | s1 | 15,101 | 1,082,521 | 9,527,185 | 3.8 % |
| C1 | s1 | 560,909 | 23,902,005 | 244,408,221 | 97.5 % |
| D (as built) | s1 | 51,172 | 1,598,045 | 17,498,837 | 7.0 % |
| D+C1 | s1 | 5,407 | 1,503,386 | 8,199,497 | 3.3 % |
| D+C | s1 | 5,407 | 663,642 | 7,632,276 | 3.0 % |
| source forms of section 5 | s1 | 4,159 | 624,496 | 6,525,413 | 2.6 % |
| `1dfc2937` | s10 | 58,808,402 | 2,434,980,694 | 24,217,181,005 | 100 % |
| A | s10 | 4,611,204 | 212,964,478 | 1,700,565,484 | 7.0 % |
| B | s10 | 69,379,038 `*` | 2,940,721,719 `*` | 28,728,364,862 `*` | 118.6 % |
| C (C1 and C2) | s10 | 155,122 | 73,851,545 | 526,225,237 | 2.2 % |
| D (as built) | s10 | 4,609,225 | 213,023,658 | 1,545,742,663 | 6.4 % |
| D+C1 | s10 | 57,913 | 203,871,046 | 639,679,011 | 2.6 % |
| D+C | s10 | 57,913 | 69,615,494 | 555,289,117 | 2.3 % |
| source forms of section 5 | s10 | 45,241 | 69,176,210 | 491,937,415 | 2.0 % |

The LLVM target runs the section-5 forms in 11,553,041 instructions per request at s1 and
1,166,189,656 at s10, with no allocation (its `1dfc2937` counts are in 2.1).

By form, allocations per request at s1 (rows copied in brackets; "inc" is a count increment, no
allocation):

| form | `1dfc2937` | A, D | B | C | C1 | D+C1, D+C |
|---|---|---|---|---|---|---|
| 1, 99 % filter of the live capture | 536,112 (22,338) | inc | 543,456 (22,644) | 0 (a view) | 536,112 | inc / view |
| 1, its 1 % filter (dead source) | 4,176 (174) | inc | 0 (moved) | 0 (a view) | 4,176 | inc / view |
| 1, 3.1 % filter of a live table | 3,072 (128) | inc | 101,376 (4,224) | 0 (a view) | 3,072 | inc / view |
| 1, 80 % filter of a live result | 2,448 (102) | inc | 3,072 (128) | 0 (a view) | 2,448 | inc / view |
| 2, read only | 23,834 | 22,889 (texts) | 23,834 | 0 | 0 | 0 |
| 2, comparison operand | 22,772 | 22,772 | 22,772 | 0 | 0 | 0 |
| 2, stored | 644 | 644 | 644 | 644 | 644 | 644 |
| 3 | 264 | 33 | 264 | 33 | 33 | 33 |
| 4, `each` | 7,416 (309) | inc | 792 (33) | 7,416 | 7,416 | inc |
| 4, `renamed` | 2,164 | 66 | 2,164 | 2,164 | 2,164 | 66 |

Instructions by form, from removing one at a time: C1 removes form 2 and the comparison
operand, 6.2 M of 250.6 M (2.5 %); D removes forms 1, 3 and 4, 232.5 M (92.8 %); after D, C1
removes 9.9 M of the 18.1 M the D prototype left. A copy and drop of a row costs about 10,000 instructions at
`1dfc2937` (24 allocations and frees). The reporter's pattern with a count instead: D+C1 and D+C
differ only in what the filters return, shared rows or references to them, so their difference is
what a count increment per kept row and its decrement at the drop of the list cost: 567,221
instructions for 22,742 kept rows at s1 (25 per row), 84,389,894 for 2,254,810 at s10 (37 per
row, the wider elements' list growth included). The drop of a list of shared rows is one
decrement per row and one free of the list's buffer; no row is freed, since the source still
holds each.

D+C uses more instructions than C at s10 (555 M against 526 M) though it allocates a third: the
prototype reads a shared list through two indirections (`Option<Arc<Vec<T>>>`) and the workload
reads rows' fields 2.3 million times per request. The single-block layout has one, as `Vec` does;
that it closes this gap is not established.

## 4 The designs compared

### 4.1 What a path that never shares pays

A was prototyped on the two frozen scenarios of rows 3.3 and 3.7 (the same transformation of the
generated program and `ignite-std`, plus the FFI edits of 3.1), counted under callgrind per
request round: 50 rounds to warm up, the counters zeroed, 200 rounds on one connection.

| program | round | `1dfc2937` | A (prototype) | change |
|---|---|---|---|---|
| `todod` (Rust, release) | `GET /todos/1` | 21,874, 21,453, 21,450 | 19,587, 19,590, 19,590 | -8.7 % to -10.5 % |
| `terminald` (Rust, release) | `echo hello` (the `rr` workload) | 3,480, 3,480 | 3,612, 3,612 | +3.8 % |

D as built, the same method, against `ef69b544` (the unit's base):

| program | round | `ef69b544` | D (as built) | change |
|---|---|---|---|---|
| `todod` (Rust, release) | `GET /todos/1` | 21,875, 21,464 | 19,219, 19,219 | -10.5 % to -12.1 % |
| `terminald` (Rust, release) | `echo hello` | 3,480, 3,480 | 3,602, 3,602 | +3.5 % |

`terminald`'s rise per round under D: `Arc<Vec<String>>::drop_slow` +75, `free` +33, `malloc`
+30, the reply's construction +14, the telnet server's loop -30: the `Arc` block of
`Reply.lines`, one `malloc` and one `free` more per reply. The release binaries grow by 1,240
bytes (`todod`, 1,063,744) and 944 bytes (`terminald`, 744,360).

`todod` shares where it copied: `Vec<(String, String)>::clone`, a request's header list, is
3.35 % of its round at `1dfc2937` and an increment under A. `terminald` builds one `List<Text>` per
round that it never copies: the prototype's second block (`boxed`, 59 instructions per round, and
its `free`) is most of the rise; the single-block layout would leave the uniqueness test and
the decrement, estimated at under 0.5 % (not measured). On the workload's table build, a path that
only builds lists of rows and drops them, A and D cost 1,530 and 1,532 more allocations (one second
block per non-empty list) and 17.5 and 16.5 % more instructions than `1dfc2937` (the table build
of 383 rows and its drop, s1 `r1` less one request). D as built: 10,016 to 11,551 allocations
(1,535 more) and 6,414,688 to 7,443,117 instructions (+16.0 %) against `ef69b544`.

D was not prototyped on `todod` or `terminald`; the built unit's counts are the second table
above. B, C1 and C2 change no
representation and cost nothing on a path that does not take their forms.

### 4.2 Each design

| | A | B | C1 | C2 | D |
|---|---|---|---|---|---|
| removes, of forms 1 to 4 (s1) | rows of 1, 3, 4 (to increments); not the texts of 2 | 174 rows of form 1, 276 of form 4; adds 4,428 rows of form 1 | form 2 read only and the comparison operand | form 1 when the result is only read | as A |
| instructions alone (s1) | 7.5 % | 117.7 % | 97.5 % | with C1: 3.8 % | 7.0 % (as built) |
| never-sharing paths | uniqueness test per change, decrement per drop; `terminald` +3.8 % in the two-block prototype | none | none | none | wrap per payload or field built: `terminald` +3.5 %, the table build +16.0 % (as built) |
| `crates/ignite-rustgen/src/lib.rs` | `rust_type_in`, `rust_ty`, `annotation` (the type), `expr` (`vec!`), `method` and `static_call` (the builtin table: `push`, `append`, `clear`, `at`, `slice`, `map`, `List.empty`, `with_capacity`), `checked_at_call`, `indexed_mutation`, `lent_iterable_text`, `binding_value`, `eq_body`, `debug_body`, `record_from_substrate`, `spawn`, `store_with` | the consumed set and `for` lowering: `lent_iterable_text`, `borrowed_for`, `ref_args`, `call` | `fn_call`, `call`, `ref_args`, `compared`, `match_expr`, `match_by_ref`, `binding_value`, `lent_let`, the fn signature in `program_module`, `fn_value_of` (the owning wrapper), `structural_eq` | as C1, plus the instances of list fns over `&T` elements, lifetimes, and binding inner views (`block`) | type emission in `program_module` (enum payloads and record fields), `typed_variant`, `variant_args`, `record_literal`, `typed_record_literal`, `record_update`, `element_field`, `pattern`, `lend_bindings`, `extract`, `eq_body`, `debug_body` |
| other crates | `ignite-io`: a new `List` type, `checked.rs` (`slice` returns it), `text.rs`, `store.rs`, `channel.rs`, `child.rs`, `args`; `docs/slice.md` section 6 | `ignite-types` `moves.rs` (`consumed` gains "elements leave") | `ignite-types`: a lent-result set per fn and the read-only use of call results (beside `called_only`, `lent_bytes`) | as C1 | `ignite-io`: the `List` type only; `docs/slice.md` section 6 |
| LLVM target | unchanged: a counted block cannot live in an arena whose region release frees it under a live reference; it keeps its deep copies | follows the consumed set (an owned parameter) | a pointer to the element, as `Lower::read_root` already gives for a matched or iterated element; fits the arenas (the source lies below every mark the callee takes) | a list of pointers is a new layout and new glue; or it keeps copying | unchanged, as A |
| differential guarantees | values unchanged; a missed `make_mut` would change a value on Rust only, and the type refuses it at compile time | unchanged | unchanged; a lent result that outlives a change of its source would not compile (rustc), so the rule must copy there | as C1 | as A |
| risks | atomic counts on every list (cost not established by counts); the FFI edits; texts stay copied | worse on the reporter's own pattern | rustc must accept every program the checker accepts (X23): the analysis must be at least as strict as the borrow checker on result lifetimes | a second list representation through every fn signature | conversions at wrap and unwrap; records without list fields gain nothing |

### 4.3 Decision

D, then C1. D removes the reporter's 66 % and its own counts' 92.8 %: the rows are payload lists,
and a copy of one becomes an increment, live source or not, whichever form carries it (1, 3 and
4). It does it with a representation change in one place, the variant and field types and their
construction, and pays nothing where lists are built in locals. A removes the same here and
more in `todod`, but tests uniqueness on every list change in every program and reaches every
FFI signature of `ignite-io`. C1 then removes what D leaves, the copies of a looked-up value that
is only compared or measured (the reporter's 10 %, 55 % of D's remaining instructions here),
and it is the one design the LLVM target can share. B is refused: its own count is worse on the
pattern it was meant for. C2 and shared `Text` wait for the counts after C1: on the workload
they would take D+C1's 8.2 M to 7.6 M (C2) and remove the 644 stored text copies (shared text),
less than the source forms of section 5 already reach.

### 4.4 Units

1. `value-copies-harness`: the workload into `crates/ignite/tests/fixtures/value_copies.ignite`
   (its inline tests join the differential corpus, metric 1.2 blessed), and
   `crates/ignite/tests/value_copies.rs`, which appends the executables and counts one request
   at s1 with valgrind as `borrowed_reads::` does. Acceptance: allocations per request on the Rust
   target at most 607,779; the LLVM executables print the Rust digests; `task gate` green.
2. `rust-shared-payload-lists` (design D, single block, atomic count). Acceptance: the harness at
   most 52,000 allocations per request at s1; a count test per kind (enum payload list, record
   field list): copying a value holding a list of M elements allocates nothing, the first change
   after a share copies once, a change of an unshared list copies nothing; `todod` `GET` and
   `terminald` `echo` instructions per round not above `1dfc2937`'s, counted by the same test on
   both builds; differential corpus and std inline tests green; `CODEGEN_REVISION` bumped.
   Built with two blocks (the single block needs an `unsafe` site, which the ratchet refuses);
   the harness counts 51,172 per request (bar 52,000) and `crates/ignite/tests/shared_lists.rs`
   is the count test per kind. `todod` falls 10.5 to 12.1 %; `terminald` rises 3.5 % (4.1), and
   that acceptance point is waived for the branch, whose merge waits on a measured run of the
   service and the operator's decision.
3. `lent-lookup-results` (design C1, both targets). Acceptance: the harness at most 6,000
   allocations per request at s1 with unit 2; a guard program per refusal (a lent result held
   across a change of its source, one stored, one through a fn value, one in a `Result` with an
   owned path), each compiled and giving the evaluator's value on both targets; on the LLVM
   target the read-only lookups of the harness call no `copy.Val`.
4. Decide shared `Text` or views by the harness's counts after unit 3.

## 5 What a program can do today

Each form below copies nothing in the Rust that `1dfc2937` emits (checked with `ignite emit`; the
only `.clone()` left in them copies an `Int`). All are in the workload, and its second inline test
asserts that each gives the value of the copying form it replaces.

| instead of | write | emitted |
|---|---|---|
| `field(r, k) != None` | a presence test that returns `Bool`: `has_in_place(v, k)`, `return true` inside the `for` | `fn has_in_place(v: &Val, k: &String) -> bool`, no copy |
| `field(r, k) == Some(want)` | the comparison where the value lies: `field_is(v, k, want)` with `return e.1 == want` | `return __ignite_eq_1(&e.1, want);` |
| `num_of(r, k)` through `field` | `match e.1 { Num(n) => n, _ => 0 }` inside the loop (`num_in_place`) | a `match` on `&e.1`, the `Int` copied out |
| `size(get(r, "attrs"))` | `for (k, x) in fields { if k == "attrs" { return size(x) } }` (`attrs_size`) | `return size(x);` with `x` a reference |
| `keep(rows, k, want)` whose result is only read | positions: `keep_at(rows, k, want) -> List<Int>`, then `narrow(rows, at, k, want)`, and every row read as `rows[i]` | `field_is(::ignite_io::checked::at(rows, i, ..), k, want)`; the result is a `Vec<i64>` |
| `each(rows, f)` that only reads its row | a `for` over the lent list calling `f(r)` directly | `child_node(r)` with `r` a reference |
| `renamed(Rec(fields), "p.")` | the record built with its final names | no second copy of the answer |

`answer_lean` is the workload's answer in these forms, with the same filters per row: 4,159
allocations and 6.5 M instructions per request at s1 (2.6 % of `1dfc2937`), fewer than any
design. It still copies the `Map` keys (`groups.get(&state.clone())`, `for (state, n) in
(groups.clone())`) and every stored lookup. What it costs is the program: positions instead of
rows, and every lookup written twice, once to read and once to store.

## Not established

- The single-block layout (count, length, capacity and elements in one block): it needs an
  allocation of its own layout, one `unsafe` site in `ignite-io`, which the ratchet
  (`quality::unsafe_sites_are_no_more_than_at_v0_15_0`) refuses. A and D were counted with
  `Option<Arc<Vec<T>>>`, two blocks and two indirections; the single-block numbers in this note
  are estimates, and whether it takes `terminald` back to 3,480 is not counted.
- A plain count against the atomic one: D as built counts atomically, since values cross
  `spawn` and channels; a plain count would need the generator to prove a value never crosses a
  task, and its cost against the atomic one cannot be shown by counts.
- Shared `Text` and `Map` anywhere.
- A design the unit did not try: wrap only the payload and field types that the program copies
  somewhere, so that a type never copied (`terminald`'s `Reply.lines`, built per reply, read once
  and dropped) stays a `Vec` and pays nothing. The information is in one run: `ignite build` and
  `ignite emit` type the program, its siblings and every std module first, then lower the
  program (`generate_package`) and then std (`generate_std_for`, which lowers every std item
  again through `prepare_std_typed` and prunes by the names the program's text reaches); a test
  build does the same with `generate_tests`. It is not at hand where it would be needed: the
  generator decides a copy (a place in a value position that `TypedProgram::moves` does not
  mark, emitted with `.clone()`) while it lowers a fn body, after the type declarations are
  decided, and std is lowered without any input from the program (`PreparedStd` is meant to be
  rendered for several programs). `Reply` is declared in `std.console`, built in the program and
  read in `std.protocol.telnet`, so its layout would depend on copy sites in both crates, found
  by a pass over every body before any type is emitted, and a copy of a type parameter in a
  generic fn would count for every type it is instantiated with. The `ignite-std` crate's
  version already hashes its `lib.rs`, so a per-program std layout builds as a separate
  artifact, as a per-program pruning already does.
- Copies by form on the LLVM target (symbols discarded in release); its counts under C1.
- Whether the reporter's typed records (if any have text fields and no list field) gain from D:
  such a record's copy is its texts, which D does not share.
