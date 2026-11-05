# Ignite slice 2: LLVM target

Status: implementation contract, 2026-11-01. Extends `docs/slice.md` (language, std, systems,
exit statuses, acceptance), which stays the contract for everything both targets share.

Direction (Timo, 2026-11-01): an LLVM target that bypasses Rust code generation. The compiler
emits LLVM IR text; clang compiles and links it. The substrate is the Rust crate `ignite-io`,
the same one the Rust target calls, behind the C ABI adapter `ignite-rt`, prebuilt as the
static library `libignite_rt.a`. Building a program
for this target never runs cargo or rustc. Since 2026-11-05 it is the default `--target`
(D2 B, `story:llvm-default-target`); the Rust target stays as the differential check.

## 1 Pipeline

```
std/**/*.ignite, program.ignite --parse--> --check--> --type (ignite-types)--> --ignite-llvm--> std.ll, program.ll, main_<exe>.ll
  --clang -c--> .o files --clang (link) + libignite_rt.a--> terminald, tn
```

| stage | crate | output |
|---|---|---|
| type | `crates/ignite-types` | a type for every expression, the resolved callee of every call and method |
| generate | `crates/ignite-llvm` | LLVM IR text, one module per std, program and executable `main` |
| runtime | `crates/ignite-rt` | `libignite_rt.a` (crate-type `staticlib` + `rlib`): `ignite-io` and the C ABI adapter of section 5 |
| driver | `crates/ignite` | `ignite build --target llvm`, `ignite test --target llvm` |

`ignite build --target llvm FILE --out DIR [--clang PATH] [--runtime PATH] [--release] [--triple TRIPLE]`:
writes `DIR/llvm/{std.ll, program.ll, main_<exe>.ll}`, runs
`clang -c -O0 -g -ffunction-sections -fdata-sections -Werror=override-module --target=<triple>`
(`-O2` with `--release`) on each, links each executable with
`clang -o DIR/bin/<exe> --target=<triple> main_<exe>.o program.o std.o <runtime> -Wl,--gc-sections [-Wl,--strip-debug] -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`
(`-Wl,--strip-debug` with `--release` only, as cargo's release profile drops debug
information; the libraries are the list `cargo rustc -p ignite-rt --print native-static-libs`
prints, fixed in the driver), and prints `built DIR/bin/<exe>`. Exit statuses as for the Rust
target; 2 also when clang fails (its output passed through).

Size. Every function and datum is in its own section (rustc emits the runtime's code that
way by default), so `--gc-sections` drops what no executable reaches. The workspace builds `ignite-rt` with `codegen-units = 1` in the release
profile.

Reachable std only (slice 11, `docs/slice.md` 15.1 item 2). `std.ll` holds only the std
fns, constants, field defaults and glue reachable from the program: the program module and
the mains are lowered first, every symbol they name is a root, and the std module keeps a
function when a root, a global or a kept function names it (a walk over the `@symbol`
references of the lowered IR, `ir::Module::set_roots`). Glue for types no kept function uses
goes with them; debug metadata of dropped functions stays unreferenced. A test build's
roots come from the test program module and `main_ignite-test.ll`, so std tests are kept
when their module is selected. For `todod` this cuts `std.ll` from 222 to 107 functions
(18,120 to 12,361 lines), `clang -c -O2` of it from 0.80 s to 0.48 to 0.55 s, and the
release build from 1.10 to 1.11 s to 0.80 to 0.81 s (four interleaved pairs against commit
`60aef38`, load 1.6, 2026-11-02). The program goldens hold no `std.ll`, so none changed.

Target. Every IR module starts with `target triple = "<triple>"` and
`target datalayout = "<layout>"`, and every clang call gets `--target=<triple>`, so the
IR, the objects and the link agree; `-Werror=override-module` makes a module whose triple
differs from clang's target an error instead of a silent override. The triple is
`--triple`, else clang's default target; both it and the data layout come from the module
clang emits for an empty C file (`clang [--target=T] -S -emit-llvm -x c /dev/null -o -`),
so the layout is always the one this clang uses for that triple. The answer is cached in
`DIR/llvm/target.stamp` under that command and the clang binary's path, size and
modification time, so an unchanged rebuild still runs no clang. Before clang compiles
anything, the runtime archive's first ELF member is checked against the triple's
architecture (ELF class, byte order, machine); a mismatch is exit 3 with
`the runtime archive is not built for the target <triple>: its objects are ELF64
little-endian machine 62, the target needs machine 183 (…)`. An architecture or an
archive format the check does not know passes. `ignite test --target llvm` takes
`--triple` too.

Optimisation. Without `--release` clang compiles at `-O0`. Measured on `task test-ignite`
(LLVM target only, std and examples, 2026-11-01, fresh output directory each run):

| level | fresh build and run | cached rebuild and run | compile (difference) | tests (runner) |
|---|---|---|---|---|
| `-O0` | 0.71 s, 0.89 s | 0.29 s, 0.32 s | 0.4 to 0.6 s | 0.02 s |
| `-O1` | 2.97 s, 3.12 s | 0.30 s, 0.29 s | 2.7 to 2.8 s | 0.02 s |

(78 tests in one executable, two runs each; "cached" reruns with every object and link
stamp current, so it is lowering plus the run.) `-O0` is the default: the suite does not
run more slowly at all, and clang compiles about five times faster. The
helpers `@ignite.append` and `@ignite.append_text` stay `noinline`: inlined 100,000 times
they made clang's `-O1` superlinear (section 4).

Cache. A `.ll` file is rewritten only when its text changed. After a step succeeds the driver
writes a stamp to `DIR/llvm`: `<unit>.o.stamp` holds the clang command and a hash of the
unit's IR text, `<exe>.link.stamp` the link command, the stamps of its three objects and the
runtime archive's canonical path, size and modification time. A step whose output exists and
whose stamp equals the one the current build would write is skipped, so an unchanged rebuild
runs no clang, and an edit to the program recompiles `program.ll` and relinks. The stamp is
deleted before its step runs, so a failed step runs again. A clang upgraded in place under
the same path is not noticed; delete `DIR/llvm`.

`--clang` defaults to `clang` on `PATH`. `--runtime` defaults to the first file of: (1)
`libignite_rt.a` in the directory of the running executable, which `cargo build -p ignite-rt`
writes next to `target/<profile>/ignite`; (2) `libignite_rt.a` in that directory's parent, for
a driver running in a test binary in `target/<profile>/deps`; (3) the archive the driver
carries: `crates/ignite/build.rs` builds `ignite-rt --features runner` for the driver's target
and profile, the crate embeds it, and the first build that needs it writes it to
`$XDG_CACHE_HOME/ignite/runtime/<hash>/libignite_rt.a` (`$HOME/.cache` when `XDG_CACHE_HOME`
is unset or relative; a temporary file and a rename), where later builds find it. When that
write fails the error (exit 3) names the first candidate, the cache path and the fix:
`--runtime` with an archive from `cargo build -p ignite-rt --features runner`, or
`--target rust`. Tests pass `--runtime` with the newest `deps/libignite_rt-*.a`,
because a test build does not refresh the top-level copy. `task bench` prints sizes, build
times and startup time of both targets.

## 2 Types (`ignite-types`)

The type checker runs after `ignite-check` on every std module (in import order) and on the
program. It is bidirectional: an expression is checked against an expected type when the
context gives one, else its type is synthesised.

```rust
pub enum Ty {
    Int, Bool, Byte, Unit, Text,
    List(Box<Ty>),               // Bytes is List(Byte)
    Option(Box<Ty>), Result(Box<Ty>, Box<Ty>),
    Record(QualName), Enum(QualName),
    Fn(Vec<Ty>, Box<Ty>),
    TcpListener, TcpStream, IoError, Input,
    Never,                       // return, break, continue, Process.exit
}
pub struct QualName { pub module: String, pub name: String }   // "std.console", "Reply"
```

Rules:

- `Bytes` is `List<Byte>` everywhere; `Byte` literals: an integer literal checked against
  `Byte` is a `Byte` (0 to 255, else E044); otherwise `Int`.
- Binary operators: both operands have the same type. When one operand is a literal, the
  other is typed first and gives the literal its expected type. Arithmetic and ordering on
  `Int` and `Byte`; `==`/`!=` on `Int`, `Byte`, `Bool`, `Text` (E045 otherwise); `&&`, `||`,
  `!` on `Bool`.
- `let x = e` takes the type of `e`; `let x: T = e` checks `e` against `T`. A `var` keeps
  the type of its initializer; assigning another type is E040.
- `[]`, `List.empty()`, `None` and `Err`/`Ok` with an unknown other half need an expected
  type (E042 otherwise; write an annotation).
- `match` and `if`/`else` in value position: all arms have one type (`Never` arms excepted).
- `e?` needs `e: Result<T, E>` in a function returning `Result<_, E>`; its type is `T`.
- `await e` has the type of `e`; `spawn e` is `Unit`.
- Builtin methods are resolved by receiver type (table in `docs/slice.md` section 6 plus the
  substrate methods of section 7); an unknown method or field for the receiver type is E041.
- Call argument count mismatch is E043 (now checked for every callee with a known signature).
- `Bytes.of(list)` takes `Bytes`: list literal elements are `Byte` literals, so E044 checks their range.
- Every `match`, in statement or value position, is exhaustive (E046, message
  ``match does not cover `Command.Quit` ``). An enum, `Option`, `Result` or `Input` needs every
  variant; payload patterns are checked recursively, so `Some(Command.Help)` covers only that
  value and `Pair.Both(Some(n), _)` leaves `Pair.Both(None, _)` uncovered. `Bool`, `Int`,
  `Byte`, `Text` and every other type need a catch-all arm: `_` or a binding (the slice has
  no `Bool` patterns, so a `Bool` match covers `true` and `false` only that way). An arm whose
  payload count is wrong (E043) counts as covering its variant. The generator keeps the
  fallback: a `match` with no arm for the value calls `ignite_rt_fail_match` with the
  `match`'s source position (a section-4 failure, report `match failed at
  <file>:<line>:<col>: no arm matched`); for a program that passed the type checker it is
  unreachable.
- Every std file and `terminal.ignite` must type-check; where the Rust target relied on rustc
  inference (e.g. `var verb = 0` later assigned a `Byte`), the Ignite source gets an annotation.

API (frozen skeleton, add do not rename):

```rust
pub type TypedModules = BTreeMap<String, ModuleTypes>;          // module path -> exported signatures
pub fn type_module(program: &Program, symbols: &Symbols, modules: &TypedModules)
    -> Result<TypedProgram, Vec<Diagnostic>>;
impl TypedProgram {
    pub fn type_of(&self, expr: &Expr) -> &Ty;                  // keyed by the expression's address
    pub fn callee(&self, expr: &Expr) -> Option<&Callee>;       // for Call and Method expressions
    pub fn binding_type(&self, span: Span) -> Option<&Ty>;      // let/var/for/pattern bindings by name span
    pub fn exports(&self) -> ModuleTypes;
}
pub enum Callee { Fn(QualName), Role { protocol: QualName, role: String }, Variant { enum_: QualName, index: usize },
                  Builtin(Builtin), Local, Const(QualName) }
```

`Builtin` is one enum variant per row of the builtin table, typed (e.g. `TextLen`,
`ListLen(Ty)`, `ListPush(Ty)`, `TcpStreamRead`, `StdinSetRaw`, `IoErrorNew`, `ProcessExit`).

## 3 Representation

| Ignite | LLVM | size, align |
|---|---|---|
| `Int` | `i64` | 8, 8 |
| `Bool` | `i1` in registers, `i8` in memory | 1, 1 |
| `Byte` | `i8` | 1, 1 |
| `Unit`, `()` | no value (`void` return, omitted parameter) | 0 |
| `Text`, `List<T>` | `{ ptr, i64 len, i64 cap }` (`len`, `cap` in elements; `Text` is UTF-8 bytes) | 24, 8 |
| `TcpListener`, `TcpStream` | `i64` runtime handle | 8, 8 |
| record | `{ fields in declaration order }` with C layout | C rules |
| tuple `(A, B, ..)` | `{ elements in order }` with C layout, an aggregate like a record; copy function `"copy.(A,B)"` | C rules |
| enum, `Option`, `Result`, `Input`, `Exposure` | `{ i32 tag, payload }`: tag = variant index in declaration order (`Some` 0, `None` 1; `Ok` 0, `Err` 1; `Data` 0, `Eof` 1; `Loopback` 0, `Private` 1, `Public` 2); payload at offset 8 (or 4 when every payload's alignment is ≤ 4), laid out as a C struct of the variant's fields; size = max over variants, rounded to 8 | |
| `IoError` | `{ Text message, i32 exit_code }` | 32, 8 |
| `fn(..) -> R` | `{ ptr code, ptr env }`, an aggregate (section 3.1) | 16, 8 |
| `fn(..) -> R requires [..]` (effectful) | as `fn(..) -> R` | 16, 8 |
| boxed field or payload position (`docs/slice.md` section 13.4) | `ptr` to the position's value in its own block | 8, 8 |
| generic record or enum `R<A, B>` | the record or enum layout with the type arguments substituted, one layout per argument list; mangled `R<A,B>` | C rules |
| `Store<T>` | `ptr` to the runtime store of the resource (section 5) | 8, 8 |
| `Duration`, `ByteSize`, `Ratio` (quantities, `docs/slice.md` section 14.1) | `i64` in the base unit `ignite_io::Quantity` defines; arithmetic is `Int`'s, with the same overflow checks (section 4); debug text through `quantity_debug` (section 5) | 8, 8 |
| `Map<K, V>` (`docs/slice.md` section 14.3) | `{ ptr, i64 len, i64 cap }` of `(K, V)` pairs sorted by key (`Int` and `Byte` numeric, `Text` bytewise through `text_cmp`, `Bool` false before true, enums by tag), looked up by binary search; copy, drop, equality and debug glue (`{k: v, …}`) as for a list of the pair; `for` visits keys in that order. A B-tree is a later milestone | 24, 8 |

Calling convention (all functions the generator emits, and every `ignite_rt_*` function that
takes or returns Ignite values):

- Scalars (`Int`, `Bool`, `Byte`, handles, `Store` pointers) by value.
- Aggregates (`Text`, lists, records, tuples, enums, `IoError`, fn values) by pointer. A parameter is
  owned exactly when its function consumes it (`TypedProgram::consumed`, `docs/slice.md`
  20.3.3: the body changes it in place on some path, or moves it or a place rooted at it on
  every path to an exit), and borrowed otherwise. The consumed set contains every parameter
  the body modifies by the rule of section 4's carried variables (an assignment to it or a
  field of it, a `push`, `append` or `clear` through a place rooted at it; matched by name),
  which owned a parameter until slice 16. The callee may change an owned parameter in place
  and move it. A direct call (a named function, method or role) passes a place to a borrowed
  parameter as the place's own pointer, with no copy. The exception is a later argument
  that modifies a variable, because it could change the place before the call. An argument
  that moves (`TypedProgram::moves`, `docs/slice.md` 20.3.5) passes the place's own pointer
  to the owned parameter, with no copy: the place is dead after the call, so ownership of
  its memory passes to the callee. Every other aggregate argument is a fresh copy (section
  4). A call through a fn pointer passes copies, or the moved place, which is correct for
  both kinds of parameter. A borrowed parameter reads the caller's memory, which stays
  valid for the whole call: no callee releases below its own loops' marks.
- An aggregate result is written to a pointer passed as the first argument (plain `ptr`, no LLVM `sret` attribute).
- Symbol names: `"<module>.<name>"` (`"std.protocol.telnet.decode"`), roles
  `"<module>.<Protocol>.<role>"`, constants `"<module>.<NAME>"` and field defaults
  `"<module>.<Record>.<field>"` as functions without arguments; a record literal that omits a
  defaulted field calls its default function. Executables' entry points are C `main`.
- Generic code is lowered per instance, never as such: a generic fn, a role of a generic
  protocol and a field default of a generic record emit nothing themselves. Each use names an
  instance from `TypedProgram::instance` (`docs/slice.md` section 11): `"<module>.<name><A,B>"`,
  `"<module>.<Protocol>.<role><A>"`, `"<module>.<Record><A>.<field>"`, with the type arguments
  mangled as in copy functions. Every IR module that uses an instance defines it
  `linkonce_odr` in a comdat of its own name (`$"sym" = comdat any`), lowered from the
  declaring module's AST with the type parameters substituted; the linker keeps one copy
  per executable (`docs/slice.md` section 13.1, row 16). Measured on `todod` (2026-11-01,
  debug): its four instances are each used by one module, so nothing is deduplicated, and
  the file grows from 5,915,280 to 5,919,584 bytes (`.text` 520,899 to 521,043) for the
  comdat groups and the symbols an `internal` definition did not export.
- An associated fn `Owner.name` (`docs/slice.md` section 13.8) is the symbol
  `"<module>.Owner.name"`; a path call `Owner.name(..)` passes only its arguments.
- A refutable pattern `let` or a `match` no arm accepts calls `ignite_rt_fail_match` with
  the pattern's or the `match`'s source position (a section-4 failure). A range pattern
  `lo..=hi` is `lo <= v && v <= hi`,
  unsigned on `Byte`, signed on `Int`, with the bounds of `TypedProgram::range_bounds`.

### 3.1 Fn values

A fn value is `{ ptr code, ptr env }`. `code` has the signature
`code(ptr env, [ptr out], params..)`: the environment first, then the usual convention. A
call through a fn value loads both words before it evaluates the arguments.

- A named fn (or associated fn) used as a value is `{ "fnval.<symbol>", null }`, where
  the internal thunk `"fnval.<symbol>"` drops `env` and calls the fn.
- A closure is the internal function `"closure.<module>.<n>"` and an environment block
  allocated in the arena where the closure is created. The block's first word is the
  address of its glue `ptr "<closure>.env"(i32 op, ptr env)`; the captures follow in
  `ClosureInfo.captures` order with C layout, each copied at creation (by copy, as
  `docs/slice.md` section 13.3 says). A closure without captures has `env` null. The body
  is lowered as a fn whose captures are borrowed bindings into the block.
- The glue of a fn value calls the env glue: op 0 deep-copies the block into the arena,
  1 to the heap (a `Store` value holding a fn), 2 frees a heap copy. A null `env` is
  copied as null. So copying a fn value copies every capture, and a closure carried
  across a loop release or put in a store keeps its captures alive.

### 3.2 Boxed positions

A field or payload position that `boxed_field` or `boxed_payload` reports holds a pointer
to a block with the value: an arena block made by `@ignite.alloc`, or a `malloc` block in a
heap copy. Constructing allocates the block; reading, matching and assigning go through
the pointer. The copy glue gives the copy a new block (deep), the heap-free glue frees
it, and the equality and debug glue follow it. A type has glue whenever it holds a boxed
position, even with no buffer.

## 4 Values and memory

- Value semantics as in the Rust target: in every value position of the ownership rule
  (`docs/slice.md` section 6) a place is deep-copied. The generator emits one copy function
  per aggregate type (`"copy.<mangled type>"`). A place in a move position that is dead
  after it (`TypedProgram::moves`, `docs/slice.md` 20.3) is not copied but moved: an
  argument passes the place's pointer to the owned parameter (section 3), and a `let` or
  `var` initializer, an assignment, a field value, a list element, a payload, a returned
  value or a block's result takes it with a shallow `memcpy` of its representation. The
  result of a call is always written to a fresh temporary, so `s = step(s, i)` moves `s`
  into the call and then the temporary into `s`; the result slot never aliases the moved
  place. A place in a position that only reads it is not copied:
  - an argument to a borrowed parameter (section 3);
  - the scrutinee of a `match` whose arms do not modify the place's root variable;
  - the list of a `for` whose body does not modify the list's root variable;
  - the receiver of `to_bytes`: the result is the receiver itself, owned or borrowed as the
    receiver is;
  - the argument of `append` when the element type holds no buffer: its elements are
    copied by the `memcpy` that appends them;
  - a `slice` that `to_text` or `Int.parse(bytes.to_text())` reads, and the `slice`
    receiver of a `slice` whose bounds modify no variable: a view, a list header with
    the address of the first element and the length, pointing into the receiver.

  `push`, `append` and `clear` on a receiver that is not a place rooted at a variable
  (`region::root` is `None`, such as `f().push(x)`) work on an owned copy. A `var` bound
  to a place is a copy, so pushing to it never changes the place it was read from
  (vision 10.1, M1 to M3; the checker refuses the forms through a `let`, a pattern or a
  `to_bytes()` receiver with E006).
- Equality (`docs/slice.md` section 13.5): `Int`, `Byte`, `Bool` and `Unit` compare by
  value and `Text` through `text_eq`; every structural type through the glue
  `i1 "eq.<T>"(ptr a, ptr b)`, which compares lists by length and elements, records and
  tuples field by field, enums by tag and then payload. `!=` is the negation. Both
  operands are evaluated once, left first; a left place is copied first when the right
  operand modifies a variable.
- Debug text (`docs/slice.md` section 13.6): `x.debug()` and the operands of a failed
  `assert a == b` without `to_text` call the glue `void "debug.<T>"(ptr list, ptr v)`,
  which appends the text of `v` to a `List<Byte>` (`@ignite.append`); `Text` goes through
  `text_debug`, `Int` and `Byte` through `int_to_text`.
- A record update `R { f = e, ..base }` evaluates the fields in source order, then
  `base`; the result is `base` with the named fields replaced, and the kept fields of a
  borrowed `base` deep-copied. `t.0` is the element at its tuple offset, a place like a
  field. `break` and `continue` in expression position branch as the statements do and
  have no value.
- Lowering walks the left spine of a binary chain (`a + b + c ..`) with a loop, as the
  loop-region analysis does, so a 100,000-term chain needs no deep native stack. After
  each step the stack slots its right operand used are dead and later temporaries reuse
  them, so a chain has a constant number of slots. A `Text` chain makes one owned text at
  its first `+` and appends every further operand to it in place (`@ignite.append`, a
  literal straight from its constant), which is linear where a new text per step would be
  quadratic. `@ignite.append` and `@ignite.append_text` are `noinline`: inlined 100,000
  times they made clang's `-O1` superlinear.
- Allocation: from the current task's arena, inline (`@ignite.alloc`, an internal
  `alwaysinline` function of every IR module). It bumps the task's bump state
  (`ignite_rt_arena_state`, section 5) when the request fits in the current chunk. Otherwise
  it calls `ignite_rt_alloc(size, align)`. Growing a list allocates a new buffer and copies;
  the old one stays in the arena until a release.
- An arena is a stack of 64 KiB chunks with a bump pointer in the last one. The runtime
  gives a request over 16 KiB its own block; the inline path puts any request that fits
  into the current chunk. Every block is allocated zeroed. In a debug build of the runtime
  a release overwrites the freed bytes with 0xDB, so a test that reads released memory
  sees garbage instead of the old value. A mark (`@ignite.arena_mark`) is
  a position in that stack: the bump state's `cursor + offset`. `@ignite.arena_release(mark)`
  frees everything allocated after the mark. When the mark is inside the last chunk, this
  resets the cursor inline; otherwise it calls `ignite_rt_arena_release`. Released chunks
  are kept for reuse, up to 16 per arena; larger blocks and further chunks go back to the
  system allocator (both measured equal on the memory test). Chunks are anonymous
  mappings of their own (`mmap`, given back with `munmap`), not `malloc` blocks, so a freed
  chunk leaves the RSS at once instead of waiting for the next heap trim, and a live chunk
  pins no freed heap memory below it; large blocks come from `malloc`. Each task also has a scratch
  arena, which allocations use while `ignite_rt_arena_scratch(1)` is on. The runtime moves
  the bump state between the two arenas.
- Loop regions. Every `while`, `loop` and `for` takes a mark at its entry (for `for`, after
  the iterated list is evaluated). Its carried variables are the variables in scope at the
  entry that the body (nested loops included) assigns, assigns a field of, or calls `push`,
  `append` or `clear` on through a place rooted at them (field, element, `?`, `await`, the
  last expression of a block), and whose type holds a buffer; matched by name, so a
  shadowing body only adds copies. At each back-edge (end of the body, `continue`, for
  `for` after the counter step):
  - no carried variables: `@ignite.arena_release(mark)`;
  - otherwise, when `@ignite.arena_due(mark, kept)` (inline: more than max(64 KiB, 4 × kept)
    bytes above the mark): scratch on, deep-copy each carried variable into its scratch
    slot, scratch off, release to the mark, deep-copy each back into its variable, clear
    the scratch arena, set `kept` to the bytes now above the mark. Releasing only when due
    keeps the copying linear: a release at every back-edge would copy a list that a loop
    grows on every iteration, making that loop quadratic. The factor is 4, not 2: the copy
    of a list has capacity = length, so its next growth alone allocates twice what was
    kept, and with a factor of 2 that one doubling would make the next release due, copying
    the whole list at every growth step past 64 KiB.

  `break`, `return` and `?` leave a loop without a release; the enclosing loop's release
  covers what it allocated. Releasing is sound because values never share buffers (value
  semantics) and nothing outside the carried variables that survives a back-edge points
  above the mark. Block-local bindings die with the iteration. Pattern bindings point into a
  scrutinee that was copied before the loop or that lives below the mark. The `for` list is
  evaluated before the mark. Callee arguments are fresh copies, the caller's places
  (borrowed) or the caller's moved places (owned), and all three lie below every mark the
  callee takes. A moved place is dead until it is reassigned, so a callee that changes it
  in place, or a release that copies a carried variable the loop moved and then reassigns,
  never shows a changed value to a later read. A moved variable that the loop reassigns is
  a carried variable like any other. A loop inside an expression that holds a partial value (an
  operand, an argument, a field, a receiver) gets no region, because that value could
  point into memory a release frees. The copies go through the scratch arena, not to a
  second mark above the first: a copy above a second mark is freed by the release to the
  first, and copying it down onto the released range overwrites it while it is read.
- An accept loop (`TelnetServer.main` and the like, `docs/slice.md` section 14.5) is a
  Ignite `loop`, so each round releases to the loop's mark after its `spawn` returns.
  Functions take no mark of their own: whatever a call allocates is released by the
  caller's loop.
- A task is the process's main thread, or a task `ignite_io::try_spawn` started for a
  `spawn`. It reuses idle threads (`docs/slice.md` section 7). A task's arena is freed when
  it ends (`task_end`), except for one chunk that the thread keeps for its next task.
  A task whose arenas held more than 1 MiB of large blocks at `task_end` calls `ignite_io::released_memory`,
  as a `Store` does when it drops a value, and the substrate returns the free heap pages
  with glibc's `malloc_trim(0)` when a later task or an idle thread ends (`docs/slice.md`
  section 7). Without it glibc keeps the freed pages in the arena of every thread that
  allocated them: the ember example's churn (10 rounds of 10 MB SET then FLUSHDB) held
  about 100 MiB RSS on 10 MB of keys (2026-11-04). The chunks a task frees go back to the
  kernel without a trim: ember's `FLUSHDB` (`db.values()` copies the keyspace into the
  arena) left 10 MB of freed chunks resident until the next trim while chunks were
  `malloc` blocks (2026-11-04).
  `spawn` hands the new task a pointer to the arguments in the parent's memory. The task
  deep-copies them into its own arena and calls `ignite_rt_spawn_ready()` before the parent
  continues (`ignite_rt_spawn` blocks until then and returns 1).

  A `TcpStream` or `PipeStream` argument moves to the task when it is a plain name that a
  `let` earlier in the same block bound to a fresh value (a call, a method call other than
  `copy()`, which shares its receiver's handle, `?` or `match`) and no later
  statement of the block mentions (`region::moved_to_task`): the accept loop's `let stream
  = self.listener.accept()?` followed by `spawn serve_with(stream, ..)`. The thunk registers
  a moved stream with `tcp_own`, so a failure closes it, and releases it with `tcp_release`
  when the callee returns, which closes the connection once the session is over and keeps
  the handle table from holding one socket per finished connection. When no thread can
  start, `ignite_rt_spawn` returns 0 without running the thunk and the parent releases the
  moved streams at once, which closes that one connection; the substrate logs the failure
  once per run of failures and the service keeps accepting. A stream that does not move
  stays the parent's: `Telnet.connect` spawns `forward_input(stream)` on its parameter and
  keeps reading the stream, and nothing releases it before the process exits (the Rust
  target's `Arc` clones close it when the last one drops). Until slice 10 the accept loop
  was generated and used a shallow `spawn_copy` without the handshake; the Ignite loop pays
  the handshake per connection.
- String literals are global constants copied into the arena where they are used as values.
- A `Store<T>` keeps its values outside every arena, so they outlive the task and the loop
  region that put them. Five glue functions per element type do the moves:
  `"copy_to_heap.<T>"` copies a value into a block from libc `malloc`, its buffers through
  `"heap_copy.<T>"`; `"copy_from_heap.<T>"` copies one back into the current arena (through
  `"copy.<T>"`, or a `memcpy` when `T` holds no buffer); `"free_heap.<T>"` frees one, its
  buffers through `"heap_free.<T>"`. `put` passes the value's pointer and the runtime
  copies it to the heap; `get`, `remove` and `values` copy back. `update(id, f)` passes a
  pointer to the fn value `f` and the thunk `ptr "store_update.<T>"(ptr f, ptr old)`, which
  copies `old` into the arena, calls `f` (code and env, section 3.1) on it and returns the result copied to the heap. The runtime calls the thunk
  under the store's lock, so `update` is atomic; the arena memory the thunk allocates
  belongs to the calling task and is released by its loop regions as usual. `get`,
  `values` and `update`'s result copy from the stored heap value into the arena under the
  lock (`ignite_io::Store`'s `read`, `read_values`, `replace_with`), so each value is
  copied once; the thunk reads `old` in place (it does not copy it to the heap first). The
  thunk calls `_setjmp` and registers its jump buffer with `ignite_rt_guard_callback`
  before it calls `f`, and `ignite_rt_guard_pop` after: a failure inside `f` keeps its
  report and jumps back to the thunk, which returns null, instead of jumping past the
  runtime frame that holds the store's lock. `store_update` then keeps the old value,
  releases the lock and ends the task with the kept report as below. `upsert(id, f)` passes
  the thunk `ptr "store_upsert.<T>"(ptr f, ptr old)` the same way; `old` is null when `id`
  holds nothing, and the thunk builds `None` or `Some` of an arena copy of `old` for `f`.
  `store_upsert` stores the result under `id` and copies it to the call's result, the
  callback guard as for `update`.
- Failure containment (`docs/slice.md` section 14.8, `vision.md` T1). A failed
  `assert`, a `?` on `Err` or `None` in a test, an integer overflow or division by zero,
  an index or `slice` out of range and a `match` with no arm each end the task that ran
  them, never the process of a service. The generator emits no landing pads: a check
  branches to a cold block of its own at the site (`cond_br bad, good; bad: literals,
  call adapter; unreachable`), which passes the operation, the site's `<file>`, `<line>`
  and `<col>` (`TypedProgram::failure_site`) and the operands to one of
  `ignite_rt_fail_arith`, `fail_neg`, `fail_index`, `fail_slice`, `fail_match`,
  `assert_failed` or `try_failed` (section 5). The adapter builds the report with
  `ignite-io`'s report fns, so both targets print byte-identical text:
  `<operation> failed at <file>:<line>:<col>: <detail>`, the details being `<left> <op>
  <right>` in base units (`+`, `-`, `*`, `/`, `%`), `-(<value>)` (unary `-`), `index <i>
  out of range for length <n>`, `slice <from>..<to> out of range for length <n>`, `no arm
  matched`, `Err(<shown>)` or `None` (`?`); `assert` keeps its `message:`, `left:` and
  `right:` continuation lines. The adapter then releases the handles the task owns
  (`tcp_own`, section 5) and ends the task:
  - the main task and a test have no task wrapper: it logs `error: <report>` and exits
    101 (`ignite_io::FAILURE_EXIT_CODE`), as `ignite_io::run` does on the Rust target;
  - a spawned task runs inside a task-entry wrapper the generator emits around its thunk:
    `_setjmp` on a 256-byte `jmp_buf` alloca (`declare i32 @_setjmp(ptr) returns_twice`,
    an attribute Rust cannot express, so it is never on a Rust frame),
    `ignite_rt_guard_push(jb)`, the body, `guard_pop`, `tcp_release` of the streams the
    task was spawned with, `task_end`. The adapter logs `task ended with error: <report>`,
    flushes, pops the guard and `longjmp`s to the wrapper, whose recovery block calls
    `task_end` and frees the task's arena; the frames it skips carry no destructors. The
    task registers the streams it was spawned with through `tcp_own` so the adapter
    closes them. A spawned task that opens handles of its own and fails leaks them,
    and a failure inside a `Store.update` thunk would hold the store's lock: both are a
    later milestone, no current program reaches them;
  - a Rust panic inside the runtime unwinds through Rust frames only: `ignite_io::spawn`
    catches it at the task's thread, and on the main thread the hook `init` installs
    reports `error: panic at <file>:<line>:<col>: <message>` and exits 101.

  An allocation failure is not a Ignite failure: the runtime aborts the process.
- Integer arithmetic fails instead of wrapping, in every profile (`vision.md` T6).
  `+`, `-` and `*` call `llvm.sadd/ssub/smul.with.overflow.i64` on `Int` and the
  quantities and `llvm.uadd/usub/umul.with.overflow.i8` on `Byte` and branch on the
  overflow bit; unary `-` on `Int` checks for `-9223372036854775808` (a negated literal
  needs no check); `/` and `%` check for a zero divisor and, on `Int`, for
  `-9223372036854775808 / -1` before `sdiv`/`srem`/`udiv`/`urem`, so no operation reaches
  LLVM's undefined behaviour. Each check branches to a cold block of its own carrying the
  site's position (above), so a chain of checked operations costs a compare and a branch
  each. `crates/ignite/tests/fixtures/tests/overflow.ignite` has one failing test per
  operator and type; `ignite_test::overflow_fails_the_same_everywhere` runs it on both
  targets in dev and in release and compares the reports.

## 5 Runtime C ABI (`ignite-rt`)

All functions `extern "C"`, prefix `ignite_rt_`. `T*` means a pointer to the section-3 layout;
`out` pointers are written on success, `err` on failure. Status `i32`: 0 ok, 1 error.

| function | meaning |
|---|---|
| `void init(i32 argc, ptr argv)` | first call in every `main`: store args, install the panic hook and the `atexit` terminal restore, ignore `SIGPIPE` |
| `ptr alloc(i64 size, i64 align)` | arena allocation of the current task (its scratch arena while scratch is on) |
| `ptr arena_state()` | the calling thread's bump state `{ ptr cursor, ptr limit, i64 offset, ptr base }`, for the inline helpers below; `memory(none)` in the IR declaration, since a thread's state never moves |
| `i8 spawn(ptr fn, ptr env)` | start `fn(env)` as a new task (`ignite_io::try_spawn`) with a new arena; returns 1 after the task called `spawn_ready`, or 0 at once when no thread can start (then `fn` does not run and the caller releases the stream handles in `env`, section 4) |
| `void spawn_ready()` | see above |
| `void task_end()` | free the current task's arena and scratch arena, keeping one chunk for the thread's next task; the last call of every task |
| `i64 arena_mark()` | the current position of the task's arena (section 4) |
| `void arena_release(i64 mark)` | free everything the task's arena handed out after `mark`; a mark at or above the current position frees nothing; the scratch arena is not touched |
| `i8 arena_due(i64 mark, i64 kept)` | 1 when more than max(64 KiB, 4 × `kept`) bytes lie above `mark` |
| `void arena_scratch(i8 on)` | 1: later allocations of this task go to its scratch arena; 0: to its arena again |
| `void arena_scratch_clear()` | free everything in the scratch arena |
| `i32 args_port(i64* out_port, i32* present, IoError* err)` | `--port N`; malformed is an error with exit code 2 |
| `void exit(i32 code)` | restore the terminal, exit |
| `void fail(IoError* error)` | write `error: <message>` to stderr, restore the terminal, exit with its exit code |
| `void panic(Text* message)` | writes `panic: <message>`, exit 101; the generator emits it no longer (a section-4 failure goes through the `fail_*` adapters below), the runtime keeps it for its own aborts |
| `void log(Text* line)` | stderr |
| `i32 tcp_bind(Text* address, i64 port, i64* out, IoError* err)` | |
| `i32 tcp_accept(i64 listener, i64* out, IoError* err)` | |
| `i64 tcp_local_port(i64 listener)` | |
| `i32 tcp_connect(Text* address, i64 port, i64* out, IoError* err)` | |
| `i32 tcp_read(i64 stream, i64 maximum, Input* out, IoError* err)` | one read, straight into a `maximum`-byte arena block whose unused tail is given back; a concurrent `tcp_write_all` on the same stream from another task does not wait for it |
| `i32 tcp_write_all(i64 stream, List* bytes, IoError* err)` | appends to the stream's send buffer (`docs/slice.md` section 7) |
| `i32 tcp_close(i64 stream, IoError* err)` | shut down the write half |
| `void tcp_release(i64 handle)` | drop a listener, stream or pipe-end handle, and forget it as owned by the task (`tcp_own`); later use of it is an error |
| `i32 stdin_read(i64 maximum, Input* out, IoError* err)` | |
| `i32 stdin_is_terminal(i8* out, IoError* err)` | |
| `i32 stdin_set_raw(i8 on, IoError* err)` | same termios rules as `docs/slice.md` section 7 |
| `i32 stdout_write_all(List* bytes, IoError* err)` | write and flush |
| `void io_error_new(IoError* out, Text* message)` | exit code 1 |
| `void text_from_bytes_lossy(Text* out, List* bytes)` | invalid UTF-8 sequences become U+FFFD; ASCII is copied without the decoder |
| `void text_trim(Text* out, Text* text)` | Unicode whitespace, as Rust `str::trim` |
| `void text_concat(Text* out, Text* a, Text* b)` | |
| `i8 text_eq(Text* a, Text* b)` | |
| `void text_debug(Text* out, Text* text)` | the debug text of `text` (`docs/slice.md` section 13.6): quoted and escaped |
| `i8 text_starts_with(Text* text, Text* prefix)` | |
| `void text_join(Text* out, List* texts, Text* separator)` | |
| `void int_to_text(Text* out, i64 value)` | |
| `i8 int_parse(Text* text, i64* out)` | 1 and `out` when `text` is a decimal `i64` with an optional leading `-` (`ignite_io::parse_int`: what Rust `str::parse` accepts less a leading `+`, read from the bytes without a UTF-8 pass), else 0; `text` may be the `List` of a `Bytes`, as `Int.parse(b.to_text())` passes `b` |
| `i32 tcp_set_idle_timeout(i64 stream, i64 ms, IoError* err)` | a read that waits longer than `ms` returns `Eof`; 0 or less removes the timeout (`ignite_io::TcpStream::set_idle_timeout`) |
| `ptr store_new(ptr to_heap, ptr from_heap, ptr free_heap, i64 size, i64 align)` | a `ignite_io::Store` of heap copies of `size`-byte values; the three functions are the element type's glue (section 4); never freed |
| `i64 store_next_id(ptr store)` | |
| `void store_put(ptr store, i64 id, ptr value)` | copies `*value` to the heap |
| `i8 store_get(ptr store, i64 id, ptr out)` | 1 and a copy in the task's arena at `out`, or 0 |
| `i8 store_remove(ptr store, i64 id, ptr out)` | as `get`, and removes the entry |
| `void store_values(ptr store, List* out)` | copies of every value, in ascending id order |
| `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)` | 0 when `id` holds nothing (`f` is not called); otherwise replaces the value by `thunk(f, old)` under the store's lock, 1 and a copy of the new value at `out`; a null from the thunk (a failure of `f` its callback guard stopped) keeps the old value, releases the lock and ends the task with the failure's report |
| `void store_upsert(ptr store, i64 id, ptr f, ptr thunk, ptr out)` | under the store's lock stores `thunk(f, old)` under `id`, `old` null when `id` holds nothing, and copies the stored value to `out`; a null from the thunk leaves the store unchanged, releases the lock and ends the task with the failure's report; declared only in a module that calls it |
| `void assert_failed(Text* cond, Text* message, Text* left, Text* right, Text* at)` | ends the task with `ignite_io::assert_report` (section 4: `error: <report>` and exit 101 on the main task or a test; `task ended with error: <report>` and a `longjmp` to the task wrapper in a spawned task). `message` is null without a message; `left` and `right` are null unless the condition is `a == b` or `a != b` |
| `void try_failed(Text* error, Text* at)` | ends the task with `ignite_io::try_report`: `? failed at <at>: Err(<error>)` (`error` not null) or `? failed at <at>: None` (section 4) |
| `void fail_arith(Text* op, Text* file, i64 line, i64 col, i64 left, i64 right)` | ends the task: `<op> failed at <file>:<line>:<col>: <left> <op> <right>` (section 4); `op` is `+`, `-`, `*`, `/` or `%`; `Byte` operands are widened to `i64` |
| `void fail_neg(Text* file, i64 line, i64 col, i64 value)` | ends the task: `- failed at <file>:<line>:<col>: -(<value>)` |
| `void fail_index(Text* file, i64 line, i64 col, i64 index, i64 len)` | ends the task: `index failed at <file>:<line>:<col>: index <index> out of range for length <len>` |
| `void fail_slice(Text* file, i64 line, i64 col, i64 from, i64 to, i64 len)` | ends the task: `slice failed at <file>:<line>:<col>: slice <from>..<to> out of range for length <len>` |
| `void fail_match(Text* file, i64 line, i64 col)` | ends the task: `match failed at <file>:<line>:<col>: no arm matched` |
| `void guard_push(ptr jb)` | register the `jmp_buf` a task-entry wrapper just passed to `_setjmp` (section 4); a thread-local stack, innermost last |
| `void guard_callback(ptr jb)` | register the `jmp_buf` a runtime callback (`store_update.<T>`, `store_upsert.<T>`) just passed to `_setjmp`: a failure whose innermost guard it is keeps its report and jumps back to it (section 4) |
| `void guard_pop()` | drop the innermost `jmp_buf` after the task body or callback returned without failing |
| `void tcp_own(i64 stream)` | mark `stream` (a TCP stream or a pipe end) as owned by the current task: a failure of the task releases it (section 4) |
| `i32 text_cmp(Text* a, Text* b)` | -1, 0 or 1: bytewise order of the UTF-8, the key order of `Map<Text, V>` (section 3) |
| `void quantity_debug(Text* out, i32 kind, i64 value)` | the debug text of a quantity, `kind` 0 `Duration`, 1 `ByteSize`, 2 `Ratio` (`ignite_io::quantity_debug`) |
| `i64 clock_monotonic()` | `Clock.monotonic()` (`docs/slice.md` 16.13): nanoseconds since an arbitrary start of the process, never decreasing (`ignite_io::clock::monotonic`); declared only in a module that calls it, so other modules' IR is unchanged |
| `i32 endpoint_of(Text* reference, Endpoint* out, IoError* err)` | `Endpoint.of(ref)` (`docs/slice.md` 16.9, 16.13): the `--ref <path>=<address>:<port>` argument `init` stored whose path is `reference`, as `std.net.Endpoint` (`{ Text address, i64 port }`, the address in the task's arena); no binding is the error `no binding for <path>`, a malformed `--ref` an error with exit code 2 (`ignite_io::args::endpoint_from_args`); declared only in a module that calls it |
| `void pipe_pair(i64* first, i64* second)` | `Pipe.pair()` (`docs/slice.md` 18.4, 18.9): two new pipe-end handles; the generator stores them as the `(PipeStream, PipeStream)` tuple. Like every `pipe_*` entry, declared only in a module that calls it, so other modules' IR is unchanged |
| `i32 pipe_read(i64 pipe, i64 maximum, Input* out, IoError* err)` | as `tcp_read`, on a pipe end; a TCP handle is the error "`<h>` is not a pipe" |
| `i32 pipe_write_all(i64 pipe, List* bytes, IoError* err)` | appends to the end's send buffer (`docs/slice.md` 18.4) |
| `i32 pipe_close(i64 pipe, IoError* err)` | delivers the buffer, then closes the end's write half |
| `i32 pipe_set_idle_timeout(i64 pipe, i64 ms, IoError* err)` | as `tcp_set_idle_timeout`, on a pipe end |
| `i64 pipe_received_batches(i64 pipe)` | the deliveries into the end's queue since `pipe_pair`; 0 for a handle that is not a pipe |
| `void test_main(IgniteTest* table, i64 count)` | `ignite_io::test_main_with` over the `count` entries at `table` and the arguments `init` stored: the runner of `docs/slice.md` section 12.7. Only with the feature `runner` (below) |

`ignite-rt` is an adapter: every `tcp_*`, `stdin_*` and `stdout_*` function, `exit`, `log`
and the thread `spawn` starts are calls into `ignite-io` (`docs/slice.md` section 7), the
substrate the Rust target calls too; argument parsing, termios save and restore and exit
codes are `ignite-io`'s as well. The adapter converts section-3 values to and from `ignite-io`
values (a `Text` or `List` argument is copied into a Rust value, a result into the task's
arena) and owns only what the LLVM target needs: the arenas and loop regions (section 4), the
`spawn`/`spawn_ready` handshake, and the handle table. A handle is an index into a
process-wide table of `ignite_io::TcpListener`, `ignite_io::TcpStream` and `ignite_io::PipeStream` values. The table
has no global lock on the IO path. It is a list of segments that never move; segment k
holds 64 × 2^k slots, each slot behind its own mutex. A handle is the slot index plus one
in its low 32 bits, with the slot's generation in the high bits. A release bumps the
generation and puts the slot on a free list (one lock, taken only by registering and
releasing). So the table grows with the number of open handles, while a stale handle is
still an error and no handle value is issued twice. Neither crate uses tokio.

Inline helpers. Every IR module defines these as internal `alwaysinline` functions over
`arena_state()`:
- `ptr @ignite.alloc(i64 size, i64 align)` bumps the cursor when an aligned request under
  64 KiB ends below `limit`; otherwise it calls `alloc`.
- `i64 @ignite.arena_mark()` returns `cursor + offset`.
- `i1 @ignite.arena_due(i64 mark, i64 kept)` is `arena_due` without the call.
- `void @ignite.arena_release(i64 mark)` does nothing when `mark` is at or above the
  position. When `mark` is in the last chunk (`mark ≥ base + offset`), it sets the cursor;
  otherwise it calls `arena_release`.

The runtime reads the cursor back before it touches an arena and republishes the state
afterwards.

`exit`, `fail`, `panic`, `assert_failed`, `try_failed`, the `fail_*` adapters and `test_main`
do not return; the generator emits `unreachable` after them.

Tests. `IgniteTest` is `{ Text name, ptr run }` (`{ ptr, i64, i64, ptr }`), `run` a
`void ()`. `test_main` exists only in an archive built with
`cargo build -p ignite-rt --features runner`, which enables the runner in `ignite-io`; the
default `libignite_rt.a` leaves it out, so a program without tests links the same members
as before tests existed. `ignite test --target llvm` checks the archive for the symbol and
otherwise fails with exit 3 naming that command. `assert_failed` and `try_failed` are in
every archive, because `assert` is allowed outside tests; they call no generic function,
and the report functions of `ignite-io` are `#[inline]`, which keeps the runtime's other
codegen units as they were. Measured on `terminald` and `tn` (2026-11-01): the Rust target's
debug executables are byte-identical; the LLVM target's grow by about 1 KB in debug (the
two adapters' line tables and constants) and by 16 and 40 bytes with `--release`.
Without `--port`, `args_port` writes port 0 and `present` 0.

## 6 Generated mains

Same behaviour and exit statuses as `docs/slice.md` sections 6.1 and 14.5: `main_<executable>.ll`
of an executable component calls `init`, creates each `Store<T>` resource the component uses
with `store_new` (bound under the resource's name), reads `--port` (`fail` with exit 2 when
malformed), binds each `TcpListener` resource the component uses with `tcp_bind` (the first
with the `--port` override; `fail` with exit 1 on error) and logs `listening on
<address>:<port>`, builds the `Endpoint` value of each listener the component names as a
target (`--port` applies to it too), builds the embedded components' values in dependency
order and then the component's own record (the kind's defaults for omitted fields,
`executable` defaulting to the component name, a `Signal` output set to its reference text),
and calls `K.main(ptr value)` through `fn_instance_with` (the kind's type arguments when `K`
is generic). `Ok` exits 0; `Err` passes the `IoError` to `ignite_rt_fail`, which prints
`error: <message>` and exits with the error's exit code. The accept loop, the per-connection
`spawn` and the client's exit codes 3 and 4 are Ignite in the kind's `main`
(`std.protocol.telnet.TelnetServer`, `TelnetClient`, `std.protocol.http.HttpServer`); a task
that fails logs `task ended with error: <report>` and the server goes on accepting
(section 14.8).

Test build (`ignite test --target llvm`, `docs/slice.md` sections 12.6 and 12.7). The
selected modules (`Options::test_modules`) keep their test-only items, and each `test`
block becomes `define void @"<module>.test.<decl index>"()`. The only executable is
`ignite-test`; its module `main_ignite-test.ll` holds `@ignite.tests`, a constant
`[N x { ptr, i64, i64, ptr }]` of the full test name (a `Text` constant) and the test
function, in the order of the selected modules and then declaration order, and
`main(argc, argv)` calls `init` and `test_main(@ignite.tests, N)`. The runner starts the
same executable with `--run-one <name>` for each test, and `test_main` then runs that
one function under `ignite_io::run`. Lowering:
- `assert cond`: evaluate `cond`; branch to a block that calls `assert_failed` with the
  source text of `cond`, the message (when given) and the position of `assert`.
- `assert a == b`, `assert a != b`: evaluate `a`, then `b`, once each. A scalar is
  compared with `icmp`; an aggregate through the glue `i1 "eq.<mangled type>"(ptr a,
  ptr b)`, structural and deep (lists element by element, enums by tag then payload),
  emitted once per type like `copy` (section 4). On failure both sides are shown by the
  `Show` the typer chose (`to_text`, an `IoError`'s message, or `<T>`) and passed as
  `left` and `right`.
- `?` in a test body (`TypedProgram::test_try`): the `Err` or `None` branch calls
  `try_failed` with the shown error (null for `None`) and the position of the `?`,
  instead of returning.
A position is `<path>:<line>:<column>` from the module's `SourceFile`
(`Options::sources`), `<module>:?:?` when the driver passed none.

## 7 Diagnostics

E040 type mismatch, E041 unknown method or field for the receiver type, E042 type cannot be
inferred (add an annotation), E043 wrong number of arguments, E044 integer literal out of
range for `Byte`, E045 operator not defined for the operand type, E046 `match` not exhaustive
(section 2), E047 record literal with a missing, unknown or duplicate field, E048 unknown type
name or wrong number of type arguments. `ignite-check` reports E047 and E048 before the type
checker runs, together with E016, an effect missing from `requires` (`docs/slice.md`
section 5.1); `ignite-types` reports E047 and E048 too for a module it is given unchecked.
`ignite check` runs the type checker too (both targets), so a program that type-checks is
valid for both. Fixtures: E040 to E046 under `crates/ignite-types/tests/fixtures`; E016, E047
and E048 under `crates/ignite-check/tests/fixtures`.

## 8 Acceptance

1. `ignite build --target llvm examples/terminal/terminal.ignite` prints `built …/terminald` and
   `built …/tn` with `CARGO` and `RUSTC` set to `/bin/false` and no cargo or rustc on `PATH`.
2. The produced binaries contain no code generated by rustc from Ignite source: the only Rust
   in them is `libignite_rt.a` (checked by the absence of any `ignite_std`/program crate symbols
   and the presence of `std.protocol.telnet.*` symbols).
3. Every case of `crates/ignite/tests/terminal.rs` passes for both targets.
4. Each code E040–E046 has a fixture under `crates/ignite-types/tests/fixtures`; E016, E047 and E048 under `crates/ignite-check/tests/fixtures`.
5. The IR of a small program is a golden file; changing the generator shows up as a diff.
6. The slice 6 surface syntax (`docs/slice.md` section 10) is the golden program
   `crates/ignite-llvm/tests/golden/syntax.ignite` (IR goldens `syntax.program.ll`,
   `syntax.main_syntax.ll`); `crates/ignite/tests/syntax.rs` builds and runs it on both targets
   and checks one output line per feature.
7. Every emitted module starts with its target triple and data layout
   (`llvm_build::ir_is_pinned_to_the_target`, `golden::modules_name_target_and_source`);
   a runtime archive for another architecture fails before clang compiles
   (`llvm_build::runtime_for_another_target_is_refused`).
8. An overflowing `Int` or `Byte` operation fails a test with the same report on both
   targets, in dev and in release (`ignite_test::overflow_fails_the_same_everywhere`).
9. `gdb` stops at a breakpoint on a line of `examples/todo/todo.ignite` in the LLVM `todod`
   and shows Ignite fn names and lines in the backtrace (section 9).

## 9 Debug metadata

Every IR module whose functions come from Ignite source carries DWARF 5 debug metadata
that maps code to the Ignite source, not to the IR: one `DICompileUnit` (`language:
DW_LANG_C`, no name table, `!llvm.dbg.cu`), one `DIFile` per Ignite module with code in it
(the path the driver read it from, relative paths under the driver's current directory),
one `DISubprogram` per function lowered from Ignite code (fns, roles, constants, field
defaults, tests, closures, instances of generic code, and `main` at the component's
`application`), and `!dbg !N` on every instruction of those functions, `!N` a
`!DILocation(line, column, scope)` made once per module for each distinct position (the
operators of one chain share one), so an instruction grows by a short suffix. The position
is that of the innermost expression or statement being lowered, columns in characters as
in diagnostics, clamped to 65,535. Glue, helpers, thunks and spawn
thunks have no subprogram and no locations. The module flags are `Dwarf Version 5` and
`Debug Info Version 3`. `-g` is passed to clang; `--release` strips the debug sections at
link time (`-Wl,--strip-debug`), as before.

So `gdb` breaks on `todo.ignite:28`, a backtrace reads
`examples.todo.create () at …/examples/todo/todo.ignite:28`, and `perf report --sort
sym,srcline` attributes samples to Ignite lines (`std.json.quote json.ignite:42`). Size of
the dev `todod` (2026-11-01, the same IR and runtime archive, `-Wl,--gc-sections`):
6,193,128 bytes with debug metadata stripped (`opt -strip-debug`) at `-O1` without `-g`,
the previous dev build; 6,208,520 bytes at `-O0` without `-g`; 6,237,064 bytes at `-O0 -g`
(+44 KB, 0.7 %). Most of the file is the runtime archive's own debug information.
