# Telos slice 2: LLVM target

Status: implementation contract, 2026-11-01. Extends `docs/slice.md` (language, std, systems,
exit statuses, acceptance), which stays the contract for everything both targets share.

Direction (Timo, 2026-11-01): an LLVM target that bypasses Rust code generation. The compiler
emits LLVM IR text; clang compiles and links it. The substrate is the Rust crate `telos-io`,
the same one the Rust target calls, behind the C ABI adapter `telos-rt`, prebuilt as the
static library `libtelos_rt.a`. Building a program
for this target never runs cargo or rustc. The Rust target stays and stays the default.

## 1 Pipeline

```
std/**/*.telos, program.telos --parse--> --check--> --type (telos-types)--> --telos-llvm--> std.ll, program.ll, main_<exe>.ll
  --clang -c--> .o files --clang (link) + libtelos_rt.a--> terminald, tn
```

| stage | crate | output |
|---|---|---|
| type | `crates/telos-types` | a type for every expression, the resolved callee of every call and method |
| generate | `crates/telos-llvm` | LLVM IR text, one module per std, program and executable `main` |
| runtime | `crates/telos-rt` | `libtelos_rt.a` (crate-type `staticlib` + `rlib`): `telos-io` and the C ABI adapter of section 5 |
| driver | `crates/telos` | `telos build --target llvm`, `telos test --target llvm` |

`telos build --target llvm FILE --out DIR [--clang PATH] [--runtime PATH] [--release]`:
writes `DIR/llvm/{std.ll, program.ll, main_<exe>.ll}`, runs
`clang -c -O1 -ffunction-sections -fdata-sections -Wno-override-module` (`-O2` with
`--release`) on each, links each executable with
`clang -o DIR/bin/<exe> main_<exe>.o program.o std.o <runtime> -Wl,--gc-sections [-Wl,--strip-debug] -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`
(`-Wl,--strip-debug` with `--release` only, as cargo's release profile drops debug
information; the libraries are the list `cargo rustc -p telos-rt --print native-static-libs`
prints, fixed in the driver), and prints `built DIR/bin/<exe>`. Exit statuses as for the Rust
target; 2 also when clang fails (its output passed through).

Size. Every function and datum is in its own section (rustc emits the runtime's code that
way by default), so `--gc-sections` drops what no executable reaches. The workspace builds `telos-rt` with `codegen-units = 1` in the release
profile.

Cache. A `.ll` file is rewritten only when its text changed. After a step succeeds the driver
writes a stamp to `DIR/llvm`: `<unit>.o.stamp` holds the clang command and a hash of the
unit's IR text, `<exe>.link.stamp` the link command, the stamps of its three objects and the
runtime archive's canonical path, size and modification time. A step whose output exists and
whose stamp equals the one the current build would write is skipped, so an unchanged rebuild
runs no clang, and an edit to the program recompiles `program.ll` and relinks. The stamp is
deleted before its step runs, so a failed step runs again. A clang upgraded in place under
the same path is not noticed; delete `DIR/llvm`.

`--clang` defaults to `clang` on `PATH`. `--runtime` defaults to the first file of: (1)
`libtelos_rt.a` in the directory of the running executable, which `cargo build -p telos-rt`
writes next to `target/<profile>/telos`; (2) `libtelos_rt.a` in that directory's parent, for
a driver running in a test binary in `target/<profile>/deps`. When neither exists the error
(exit 3) names the first. Tests pass `--runtime` with the newest `deps/libtelos_rt-*.a`,
because a test build does not refresh the top-level copy. `task bench` prints sizes, build
times and startup time of both targets.

## 2 Types (`telos-types`)

The type checker runs after `telos-check` on every std module (in import order) and on the
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
  fallback: a `match` with no arm for the value calls `telos_rt_panic` with `no match arm`
  (panic exit, section 4); for a program that passed the type checker it is unreachable.
- Every std file and `terminal.telos` must type-check; where the Rust target relied on rustc
  inference (e.g. `var verb = 0` later assigned a `Byte`), the Telos source gets an annotation.

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

| Telos | LLVM | size, align |
|---|---|---|
| `Int` | `i64` | 8, 8 |
| `Bool` | `i1` in registers, `i8` in memory | 1, 1 |
| `Byte` | `i8` | 1, 1 |
| `Unit`, `()` | no value (`void` return, omitted parameter) | 0 |
| `Text`, `List<T>` | `{ ptr, i64 len, i64 cap }` (`len`, `cap` in elements; `Text` is UTF-8 bytes) | 24, 8 |
| `TcpListener`, `TcpStream` | `i64` runtime handle | 8, 8 |
| record | `{ fields in declaration order }` with C layout | C rules |
| tuple `(A, B, ..)` | `{ elements in order }` with C layout, an aggregate like a record; copy function `"copy.(A,B)"` | C rules |
| enum, `Option`, `Result`, `Input` | `{ i32 tag, payload }`: tag = variant index in declaration order (`Some` 0, `None` 1; `Ok` 0, `Err` 1; `Data` 0, `Eof` 1); payload at offset 8 (or 4 when every payload's alignment is ≤ 4), laid out as a C struct of the variant's fields; size = max over variants, rounded to 8 | |
| `IoError` | `{ Text message, i32 exit_code }` | 32, 8 |
| `fn(..) -> R` | `{ ptr code, ptr env }`, an aggregate (section 3.1) | 16, 8 |
| `fn(..) -> R requires [..]` (effectful) | as `fn(..) -> R` | 16, 8 |
| boxed field or payload position (`docs/slice.md` section 13.4) | `ptr` to the position's value in its own block | 8, 8 |
| generic record or enum `R<A, B>` | the record or enum layout with the type arguments substituted, one layout per argument list; mangled `R<A,B>` | C rules |
| `Store<T>` | `ptr` to the runtime store of the resource (section 5) | 8, 8 |

Calling convention (all functions the generator emits, and every `telos_rt_*` function that
takes or returns Telos values):

- Scalars (`Int`, `Bool`, `Byte`, handles, `Store` pointers) by value.
- Aggregates (`Text`, lists, records, tuples, enums, `IoError`, fn values) by pointer. A parameter is
  borrowed when the function's body never modifies it (by the rule of section 4's carried
  variables: no assignment to it or a field of it, no `push`, `append` or `clear` through a
  place rooted at it; matched by name). Otherwise the callee owns it and may change it in
  place. A direct call (a named function, method or role) passes a place to a borrowed
  parameter as the place's own pointer, with no copy. The exception is a later argument
  that modifies a variable, because it could change the place before the call. Every
  other aggregate argument is a fresh copy (section 4). A call through a fn pointer
  always passes copies, which is correct for both kinds of parameter. A borrowed
  parameter reads the caller's memory, which stays valid for the whole call: no callee
  releases below its own loops' marks.
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
- A refutable pattern `let` or a `match` no arm accepts calls the internal
  `telos.no_match()`, which panics. A range pattern `lo..=hi` is `lo <= v && v <= hi`,
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
to a block with the value: an arena block made by `@telos.alloc`, or a `malloc` block in a
heap copy. Constructing allocates the block; reading, matching and assigning go through
the pointer. The copy glue gives the copy a new block (deep), the heap-free glue frees
it, and the equality and debug glue follow it. A type has glue whenever it holds a boxed
position, even with no buffer.

## 4 Values and memory

- Value semantics as in the Rust target: in every value position of the ownership rule
  (`docs/slice.md` section 6) a place is deep-copied. The generator emits one copy function
  per aggregate type (`"copy.<mangled type>"`). A place in a position that only reads it
  is not copied:
  - an argument to a borrowed parameter (section 3);
  - the scrutinee of a `match` whose arms do not modify the place's root variable;
  - the list of a `for` whose body does not modify the list's root variable;
  - the receiver of `to_bytes`: the result is the receiver itself, owned or borrowed as the
    receiver is;
  - the argument of `append` when the element type holds no buffer: its elements are
    copied by the `memcpy` that appends them.

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
  which appends the text of `v` to a `List<Byte>` (`@telos.append`); `Text` goes through
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
  its first `+` and appends every further operand to it in place (`@telos.append`, a
  literal straight from its constant), which is linear where a new text per step would be
  quadratic. `@telos.append` and `@telos.append_text` are `noinline`: inlined 100,000
  times they made clang's `-O1` superlinear.
- Allocation: from the current task's arena, inline (`@telos.alloc`, an internal
  `alwaysinline` function of every IR module). It bumps the task's bump state
  (`telos_rt_arena_state`, section 5) when the request fits in the current chunk. Otherwise
  it calls `telos_rt_alloc(size, align)`. Growing a list allocates a new buffer and copies;
  the old one stays in the arena until a release.
- An arena is a stack of 64 KiB chunks with a bump pointer in the last one. The runtime
  gives a request over 16 KiB its own block; the inline path puts any request that fits
  into the current chunk. Every block is allocated zeroed. In a debug build of the runtime
  a release overwrites the freed bytes with 0xDB, so a test that reads released memory
  sees garbage instead of the old value. A mark (`@telos.arena_mark`) is
  a position in that stack: the bump state's `cursor + offset`. `@telos.arena_release(mark)`
  frees everything allocated after the mark. When the mark is inside the last chunk, this
  resets the cursor inline; otherwise it calls `telos_rt_arena_release`. Released chunks
  are kept for reuse, up to 16 per arena; larger blocks and further chunks go back to the
  system allocator (both measured equal on the memory test). Each task also has a scratch
  arena, which allocations use while `telos_rt_arena_scratch(1)` is on. The runtime moves
  the bump state between the two arenas.
- Loop regions. Every `while`, `loop` and `for` takes a mark at its entry (for `for`, after
  the iterated list is evaluated). Its carried variables are the variables in scope at the
  entry that the body (nested loops included) assigns, assigns a field of, or calls `push`,
  `append` or `clear` on through a place rooted at them (field, element, `?`, `await`, the
  last expression of a block), and whose type holds a buffer; matched by name, so a
  shadowing body only adds copies. At each back-edge (end of the body, `continue`, for
  `for` after the counter step):
  - no carried variables: `@telos.arena_release(mark)`;
  - otherwise, when `@telos.arena_due(mark, kept)` (inline: more than max(64 KiB, 4 × kept)
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
  evaluated before the mark. Callee arguments are fresh copies or the caller's places, and
  both lie below every mark the callee takes. A loop inside an expression that holds a partial value (an
  operand, an argument, a field, a receiver) gets no region, because that value could
  point into memory a release frees. The copies go through the scratch arena, not to a
  second mark above the first: a copy above a second mark is freed by the release to the
  first, and copying it down onto the released range overwrites it while it is read.
- A service's accept loop in the generated `main` releases to a mark taken before the loop
  after each `spawn` returns. Functions take no mark of their own: whatever a call
  allocates is released by the caller's loop.
- A task is the process's main thread, or a task `telos_io::spawn` started for `spawn` or
  for a connection of a service's accept loop. `telos_io::spawn` reuses idle threads
  (`docs/slice.md` section 7). A task's arena is freed when it ends (`task_end`), except
  for one chunk that the thread keeps for its next task. `spawn` hands the new task a
  pointer to the arguments in the parent's memory. The task deep-copies them into its own
  arena and calls `telos_rt_spawn_ready()` before the parent continues
  (`telos_rt_spawn` blocks until then).

  A connection task of the accept loop starts without that handshake, through
  `telos_rt_spawn_copy(thunk, env, size)`. The loop stores the stream handle and the
  `application` value into the environment shallowly, the runtime copies those bytes,
  and the loop continues at once. This is sound because `application` is evaluated
  before the loop's mark and is never released or changed, so its buffers outlive every
  connection task. The task passes a borrowed parameter of `serve` straight from its copy
  of the environment, which lives until `serve` returns. It deep-copies an owned parameter
  into its own arena first, so no task changes a buffer it shares. When no thread can
  start, `spawn_copy` returns 0 and the loop releases the stream handle, which closes
  that one connection; the substrate logs the failure once per run of failures and the
  service keeps accepting.
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
  belongs to the calling task and is released by its loop regions as usual.
- A connection task of a service ends with `telos_rt_tcp_release(stream)` after `serve`
  returns, so the handle table does not keep one socket per finished connection.
- Index out of range, `slice` out of range, and allocation failure call
  `telos_rt_panic(message)`: message to stderr, terminal restored, exit status 101.

## 5 Runtime C ABI (`telos-rt`)

All functions `extern "C"`, prefix `telos_rt_`. `T*` means a pointer to the section-3 layout;
`out` pointers are written on success, `err` on failure. Status `i32`: 0 ok, 1 error.

| function | meaning |
|---|---|
| `void init(i32 argc, ptr argv)` | first call in every `main`: store args, install the panic hook and the `atexit` terminal restore, ignore `SIGPIPE` |
| `ptr alloc(i64 size, i64 align)` | arena allocation of the current task (its scratch arena while scratch is on) |
| `ptr arena_state()` | the calling thread's bump state `{ ptr cursor, ptr limit, i64 offset, ptr base }`, for the inline helpers below; `memory(none)` in the IR declaration, since a thread's state never moves |
| `void spawn(ptr fn, ptr env)` | start `fn(env)` as a new task (`telos_io::spawn`) with a new arena; returns after the task called `spawn_ready`, or at once when no thread can start (then `fn` does not run) |
| `i8 spawn_copy(ptr fn, ptr env, i64 size)` | start `fn(copy)` as a new task, where `copy` is a shallow copy of the `size` bytes at `env` that lives until `fn` returns; returns 1 at once, or 0 when no thread can start and `fn` will not run (section 4, the accept loop) |
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
| `void panic(Text* message)` | section 4; writes `panic: <message>` |
| `void log(Text* line)` | stderr |
| `i32 tcp_bind(Text* address, i64 port, i64* out, IoError* err)` | |
| `i32 tcp_accept(i64 listener, i64* out, IoError* err)` | |
| `i64 tcp_local_port(i64 listener)` | |
| `i32 tcp_connect(Text* address, i64 port, i64* out, IoError* err)` | |
| `i32 tcp_read(i64 stream, i64 maximum, Input* out, IoError* err)` | one read, straight into a `maximum`-byte arena block whose unused tail is given back; a concurrent `tcp_write_all` on the same stream from another task does not wait for it |
| `i32 tcp_write_all(i64 stream, List* bytes, IoError* err)` | appends to the stream's send buffer (`docs/slice.md` section 7) |
| `i32 tcp_close(i64 stream, IoError* err)` | shut down the write half |
| `void tcp_release(i64 handle)` | drop a listener or stream handle; later use of it is an error |
| `i32 stdin_read(i64 maximum, Input* out, IoError* err)` | |
| `i32 stdin_is_terminal(i8* out, IoError* err)` | |
| `i32 stdin_set_raw(i8 on, IoError* err)` | same termios rules as `docs/slice.md` section 7 |
| `i32 stdout_write_all(List* bytes, IoError* err)` | write and flush |
| `void io_error_new(IoError* out, Text* message)` | exit code 1 |
| `void text_from_bytes_lossy(Text* out, List* bytes)` | |
| `void text_trim(Text* out, Text* text)` | Unicode whitespace, as Rust `str::trim` |
| `void text_concat(Text* out, Text* a, Text* b)` | |
| `i8 text_eq(Text* a, Text* b)` | |
| `void text_debug(Text* out, Text* text)` | the debug text of `text` (`docs/slice.md` section 13.6): quoted and escaped |
| `i8 text_starts_with(Text* text, Text* prefix)` | |
| `void text_join(Text* out, List* texts, Text* separator)` | |
| `void int_to_text(Text* out, i64 value)` | |
| `i8 int_parse(Text* text, i64* out)` | 1 and `out` when `text` is a decimal `i64` with an optional leading `-` (Rust `str::parse`; a leading `+` is rejected), else 0 |
| `i32 tcp_set_idle_timeout(i64 stream, i64 ms, IoError* err)` | a read that waits longer than `ms` returns `Eof`; 0 or less removes the timeout (`telos_io::TcpStream::set_idle_timeout`) |
| `ptr store_new(ptr to_heap, ptr from_heap, ptr free_heap, i64 size, i64 align)` | a `telos_io::Store` of heap copies of `size`-byte values; the three functions are the element type's glue (section 4); never freed |
| `i64 store_next_id(ptr store)` | |
| `void store_put(ptr store, i64 id, ptr value)` | copies `*value` to the heap |
| `i8 store_get(ptr store, i64 id, ptr out)` | 1 and a copy in the task's arena at `out`, or 0 |
| `i8 store_remove(ptr store, i64 id, ptr out)` | as `get`, and removes the entry |
| `void store_values(ptr store, List* out)` | copies of every value, in ascending id order |
| `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)` | 0 when `id` holds nothing (`f` is not called); otherwise replaces the value by `thunk(f, old)` under the store's lock, 1 and a copy of the new value at `out` |
| `void assert_failed(Text* cond, Text* message, Text* left, Text* right, Text* at)` | `telos_io::assert_failed`: the report of `docs/slice.md` section 12.5 to stderr, restore the terminal, exit 101. `message` is null without a message; `left` and `right` are null unless the condition is `a == b` or `a != b` |
| `void try_failed(Text* error, Text* at)` | `telos_io::try_failed`: `` `?` failed on Err: <error> `` (`error` not null) or `` `?` failed on None ``, then `at`, exit 101 |
| `void test_main(TelosTest* table, i64 count)` | `telos_io::test_main_with` over the `count` entries at `table` and the arguments `init` stored: the runner of `docs/slice.md` section 12.7. Only with the feature `runner` (below) |

`telos-rt` is an adapter: every `tcp_*`, `stdin_*` and `stdout_*` function, `exit`, `log`
and the thread `spawn` starts are calls into `telos-io` (`docs/slice.md` section 7), the
substrate the Rust target calls too; argument parsing, termios save and restore and exit
codes are `telos-io`'s as well. The adapter converts section-3 values to and from `telos-io`
values (a `Text` or `List` argument is copied into a Rust value, a result into the task's
arena) and owns only what the LLVM target needs: the arenas and loop regions (section 4), the
`spawn`/`spawn_ready` handshake, and the handle table. A handle is an index into a
process-wide table of `telos_io::TcpListener` and `telos_io::TcpStream` values. The table
has no global lock on the IO path. It is a list of segments that never move; segment k
holds 64 × 2^k slots, each slot behind its own mutex. A handle is the slot index plus one
in its low 32 bits, with the slot's generation in the high bits. A release bumps the
generation and puts the slot on a free list (one lock, taken only by registering and
releasing). So the table grows with the number of open handles, while a stale handle is
still an error and no handle value is issued twice. Neither crate uses tokio.

Inline helpers. Every IR module defines these as internal `alwaysinline` functions over
`arena_state()`:
- `ptr @telos.alloc(i64 size, i64 align)` bumps the cursor when an aligned request under
  64 KiB ends below `limit`; otherwise it calls `alloc`.
- `i64 @telos.arena_mark()` returns `cursor + offset`.
- `i1 @telos.arena_due(i64 mark, i64 kept)` is `arena_due` without the call.
- `void @telos.arena_release(i64 mark)` does nothing when `mark` is at or above the
  position. When `mark` is in the last chunk (`mark ≥ base + offset`), it sets the cursor;
  otherwise it calls `arena_release`.

The runtime reads the cursor back before it touches an arena and republishes the state
afterwards.

`exit`, `fail`, `panic`, `assert_failed`, `try_failed` and `test_main` do not return; the
generator emits `unreachable` after them.

Tests. `TelosTest` is `{ Text name, ptr run }` (`{ ptr, i64, i64, ptr }`), `run` a
`void ()`. `test_main` exists only in an archive built with
`cargo build -p telos-rt --features runner`, which enables the runner in `telos-io`; the
default `libtelos_rt.a` leaves it out, so a program without tests links the same members
as before tests existed. `telos test --target llvm` checks the archive for the symbol and
otherwise fails with exit 3 naming that command. `assert_failed` and `try_failed` are in
every archive, because `assert` is allowed outside tests; they call no generic function,
and the report functions of `telos-io` are `#[inline]`, which keeps the runtime's other
codegen units as they were. Measured on `terminald` and `tn` (2026-11-01): the Rust target's
debug executables are byte-identical; the LLVM target's grow by about 1 KB in debug (the
two adapters' line tables and constants) and by 16 and 40 bytes with `--release`.
Without `--port`, `args_port` writes port 0 and `present` 0.

## 6 Generated mains

Same behaviour and exit statuses as `docs/slice.md` section 6.1: a `Service` main binds
(`fail` with exit 1 on error), logs `listening on <address>:<port>`, and for each accepted
stream starts a task running `serve(stream, application)` (an error is logged as `session
ended with error: <message>`); a `Client` main connects (exit 3), runs `connect` (exit 4 on
`Err`), exits 0. `--port` errors exit 2.

Each `Store<T>` resource is created by `store_new` right after `init` and bound under the
resource's name, so a component property can name it. A `Service` main evaluates
`application` after the bind and the log line, with the listener resource bound under its
name as well (so `application` may name it), and before the accept loop's mark (section 4).
The role comes from the protocol's instance when the protocol is generic.

Test build (`telos test --target llvm`, `docs/slice.md` sections 12.6 and 12.7). The
selected modules (`Options::test_modules`) keep their test-only items, and each `test`
block becomes `define void @"<module>.test.<decl index>"()`. The only executable is
`telos-test`; its module `main_telos-test.ll` holds `@telos.tests`, a constant
`[N x { ptr, i64, i64, ptr }]` of the full test name (a `Text` constant) and the test
function, in the order of the selected modules and then declaration order, and
`main(argc, argv)` calls `init` and `test_main(@telos.tests, N)`. The runner starts the
same executable with `--run-one <name>` for each test, and `test_main` then runs that
one function under `telos_io::run`. Lowering:
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
name or wrong number of type arguments. `telos-check` reports E047 and E048 before the type
checker runs, together with E016, an effect missing from `requires` (`docs/slice.md`
section 5.1); `telos-types` reports E047 and E048 too for a module it is given unchecked.
`telos check` runs the type checker too (both targets), so a program that type-checks is
valid for both. Fixtures: E040 to E046 under `crates/telos-types/tests/fixtures`; E016, E047
and E048 under `crates/telos-check/tests/fixtures`.

## 8 Acceptance

1. `telos build --target llvm examples/terminal/terminal.telos` prints `built …/terminald` and
   `built …/tn` with `CARGO` and `RUSTC` set to `/bin/false` and no cargo or rustc on `PATH`.
2. The produced binaries contain no code generated by rustc from Telos source: the only Rust
   in them is `libtelos_rt.a` (checked by the absence of any `telos_std`/program crate symbols
   and the presence of `std.protocol.telnet.*` symbols).
3. Every case of `crates/telos/tests/terminal.rs` passes for both targets.
4. Each code E040–E046 has a fixture under `crates/telos-types/tests/fixtures`; E016, E047 and E048 under `crates/telos-check/tests/fixtures`.
5. The IR of a small program is a golden file; changing the generator shows up as a diff.
6. The slice 6 surface syntax (`docs/slice.md` section 10) is the golden program
   `crates/telos-llvm/tests/golden/syntax.telos` (IR goldens `syntax.program.ll`,
   `syntax.main_syntax.ll`); `crates/telos/tests/syntax.rs` builds and runs it on both targets
   and checks one output line per feature.
