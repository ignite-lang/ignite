# Ignite on WebAssembly: components, a runtime in Ignite, hot reload

**Status:** future design, not a slice contract. Nothing here is implemented.
**Date:** 1 November 2026
**Scope:** compiling Ignite components to WebAssembly, running them on a WebAssembly runtime
written in Ignite, and replacing single components in a running system.

Direction (Timo, 2026-11-01): whole systems, services or components compile to WebAssembly and
run on a runtime. The runtime is written in Ignite. The goal is to compile components on the fly
and hot-reload them: a watcher, a running application, and a reload of the components that
changed.

This document decides. Each section states the chosen design, the alternatives it rejects and
why. References to Ignite cite `design.md` as "design §n", `docs/slice.md` as
"slice §n", `docs/llvm.md` as "llvm §n", or a file and line. External facts carry the IDs of
section 13. Where a proposal conflicts with a stated Ignite decision, it is marked **Conflict**,
and section 12 collects these conflicts. Actor state, `code_change`-style migration of actor
state, and supervision are in `docs/actors.md` (written in parallel). This document covers the
component swap mechanism only.

## 1 Goals and non-goals

### 1.1 The dev loop

```bash
$ ignite run --watch examples/todo/todo.ignite
listening on 127.0.0.1:8080
watching examples/todo/todo.ignite and std/: components server (v1), server.application (v1)
# edit `handle` in todo.ignite, save
reloaded server.application v2 (3f9c0a…) in 180 ms; store todos kept (3 values)
# edit `type Todo` without a migration, save
examples/todo/todo.ignite:15:1: error[E080]: `Todo` changed its stored shape; `todos` holds 3 values of the old shape; declare `migrate` on resource `todos`
server.application v2 keeps running
```

The loop has five steps:

1. **Change.** The watcher in the `ignite` driver sees a write to the program file or to a file
   under `std/`.
2. **Check.** The frontend checks the whole program again (parse, check, type; slice §1). Any
   diagnostic stops the step. The running version stays and the driver prints the diagnostic.
3. **Rebuild.** The driver hashes each component's reachable definitions (section 6) and
   compiles only the components whose hash changed, plus the dependents whose interface
   changed.
4. **Ship.** It sends the new component bytes over a loopback control connection to the running
   runtime process, `ignite-wasmrt`.
5. **Swap.** The runtime decodes and validates the component, checks its interface and state
   schema, links it, and swaps it at the next swap point (section 5.4). It answers `reloaded`
   or `refused: <reason>`.

`ignite run` is the design's foreground development command (design §10.1).
`--watch` extends it. The command uses an implicit development assembly that places
every component in one local `ignite-wasmrt` process. Assemblies are design §5 and the
`assembly local` declaration of design §11.3; they are not in a slice yet.

### 1.2 Latency budget

The budget runs from the file save to the first request served by the new version. Three of
the steps depend on the compilation path of section 2.

| step | LLVM path (phases 1–4) | direct emitter (phase 5) |
|---|---|---|
| debounce | 30 ms | 30 ms |
| frontend over program and std | 50 ms | 50 ms |
| codegen of one component | 150 ms (`clang -O0`, wasm-ld, component wrap) | 30 ms |
| transfer, decode, validate | 20 ms | 20 ms |
| link, pre-instantiate | 5 ms | 5 ms |
| wait for the swap point | one in-flight request | same |
| **total** | **≤ 300 ms** | **≤ 150 ms** |

**Conflict** with design §1.5: "Set numeric performance budgets only
after the first baseline exists." These numbers are therefore targets. The phase 1 measurement
replaces them before any phase gate uses them.

### 1.3 Goals

- **G1. Same semantics.** Ignite semantics on wasm are the same as on the Rust and LLVM targets
  (design §8.3). Hot reload is an operation on a running system, like a
  rollout; it is not a language feature.
- **G2. Authority by imports.** A component's authority is its import list. The list is derived
  from the effects the checker computed (slice §5).
- **G3. A runtime in Ignite.** The runtime is Ignite, compiled natively by the LLVM target. The
  only Rust it runs is the substrate (`AGENTS.md`, Layout, first rule).
- **G4. Reloads keep data.** A reload keeps resource data and open sessions.

### 1.4 Non-goals

- Browser execution.
- Running arbitrary third-party components or WASI programs. This is a later interop phase
  (section 11, Q8).
- Replacing native targets for releases. Releases stay native (design §11.9).
- Moving live TCP connections between processes (design §11.4).
- Reloading the runtime itself. Changing it means a restart.
- A JIT before phase 6.
- WasmGC.

## 2 Compilation path

### 2.1 Choice and sequence

| question | chosen | rejected | why |
|---|---|---|---|
| first wasm path | reuse `ignite-llvm` with a wasm32 data layout; `clang --target=wasm32-unknown-unknown`, `wasm-ld`, then `wasm-tools component new` | direct emitter first | It reuses the lowering, regions and glue of llvm §3–4 that the tests already exercise, so wasm is running in weeks rather than months. |
| guest support code | arena slow path and spawn glue emitted as IR by `ignite-llvm` (`llvm.wasm.memory.grow`) | `ignite-rt` compiled for wasm32 | No hand-written Rust inside the guest; `ignite-rt` depends on `ignite-io` and `std::net`. |
| clang triple | `wasm32-unknown-unknown` with Ignite's own imports | `wasm32-wasip1`/`wasip2` [W11, W13, W14] | Ignite code needs no libc. A WASI triple would bring a second IO path that bypasses `ignite-io` (**Conflict** with `AGENTS.md`, Layout, second rule: "the substrate is implemented once"). |
| second path | a direct wasm emitter written in Ignite (phase 5) | an emitter written in Rust | The emitter is the first backend piece toward self-hosting (design §12.4). It compiles inside the runtime process with no clang, which is what makes on-the-fly compiles cheap. |
| input of the Ignite emitter | a serialized Ignite Code IR that the Rust frontend writes (`ignite emit --code-ir`) | re-parsing in Ignite | The frontend is Rust until milestone M6 (design §12.2). Code IR is the layer the design already names (design §8.5). |

Sequence: LLVM path (phase 1) → Ignite interpreter (phase 2) → reload (phases 3, 4) → direct
emitter (phase 5) → baseline compiler (phase 6). The LLVM path stays as the reference: the
emitter's output must behave like it on the same tests.

### 2.2 WebAssembly feature set

The guest feature set is LLVM's fixed `lime1` CPU: MVP plus mutable-globals, bulk-memory-opt,
multivalue, sign-ext, nontrapping-fptoint, extended-const and call-indirect-overlong [W12]. The
Ignite interpreter implements this set and nothing more. The set is frozen, so the runtime has a
target that does not move. Newer features go into later CPUs such as `lime2`, not into `lime1`
[W12].

| Wasm 3.0 / proposal feature [W1, W2, W5] | decision | why |
|---|---|---|
| GC (3.0) | rejected | Ignite values are arena-allocated with value semantics (llvm §4), not GC references. Wasmtime's baseline compiler Winch lacks GC [W16]. |
| exception handling (3.0) | rejected | Ignite has `Result` and no exceptions. A trap ends the task (section 8). |
| tail calls (3.0) | not needed | No Ignite construct requires them. |
| memory64 (3.0) | rejected | Each task has its own instance (section 7). 4 GiB per task is enough. |
| multiple memories (3.0) | rejected | The scratch arena is a second chunk list in the same memory (section 2.3). |
| threads (Phase 4, not in 3.0) | rejected | Ignite tasks share nothing, so one instance per task needs no shared memory. Threads are Tier 2 in Wasmtime and unsupported on Pulley [W16]. `wasi-threads` is legacy [W28]; shared-everything-threads is Phase 1 [W5, W29]. |
| stack switching (Phase 3) | rejected for now | Not standardized, and in progress on x86_64 Cranelift only [W5, W6, W16]. Suspension happens in the host (section 7). |

### 2.3 Mapping existing pieces onto wasm

| piece (native) | on wasm |
|---|---|
| task arena, 64 KiB chunks, bump state (llvm §4, §5) | The instance's linear memory is the task's arena. Chunks are carved from memory, and the slow path calls `memory.grow`. `@ignite.alloc`, `arena_mark`, `arena_release` and `arena_due` stay inline helpers in IR. |
| scratch arena (llvm §4) | A second chunk list in the same memory. |
| loop regions (llvm §4, `crates/ignite-llvm/src/region.rs`) | Unchanged. They are IR, so they compile to wasm as they are. |
| `Text`, `List<T>` `{ptr, i64 len, i64 cap}` (llvm §3) | `{i32 ptr, i64 len, i64 cap}`: 24 bytes, alignment 8. `len` stays `Int`. |
| handle table, generation in the high bits (llvm §5; module doc and `encode` in `crates/ignite-rt/src/net.rs`) | It moves to the runtime as one table per instance (the canonical ABI's per-instance handle table [W9]). The guest sees an `i32` resource index; a stale or forged index traps. |
| `spawn` handshake (llvm §4, `ignite_rt_spawn`) | Import `ignite:task/spawn(entry, env-ptr, env-len)`. Per-type `pack.<T>`/`unpack.<T>` glue writes the arguments to a flat buffer. The runtime copies the buffer into a new instance of the same module version, whose start export unpacks it. |
| generics, monomorphized per instance (slice §11.1) | Unchanged. WIT has no user generics, so each instance that crosses a boundary gets a WIT name such as `pair-s64-string`. |
| `Store<T>` (slice §11.3; `Store` in `crates/ignite-io/src/store.rs`) | It lives in the runtime as a resource (section 5.2). |
| generated `main` (slice §6.1, llvm §6) | No main per executable. The runtime reads the system manifest, binds listeners and runs the accept loops in Ignite (section 4.1). |

## 3 Unit of compilation and isolation

### 3.1 The cut

There is one wasm component per Ignite component, in the Component Model sense: shared-nothing,
importing and exporting only typed functions and resources [W7]. In the slice-7 surface, the
`application` of a `Service` is a value inside `server` (component `server` of `system Terminal` in `examples/terminal/terminal.ignite`).
In design §11.3 it is a component of its own: `component console: CommandConsole`, with
`application = console` (components `console` and `server` of design §11.3). The wasm target follows the design. A
`Service` therefore yields two components:

- `server`: the protocol role (`Telnet.serve`, `Http.serve`) and everything reachable from it.
- `server.application`: the functions reachable from the fn-typed fields of the `application`
  value (`respond = execute`, `handle`).

Every fn-typed field of the application value is a cut point. It becomes an export of the
application component and an import of the protocol component. The remaining fields
(`prompt`, `maximum_line`, `state = todos`) are passed as data to `serve`. A fn value that
crosses a cut must be a named function known at build time. A computed one is a new error,
E082 ``function value `f` crosses a component boundary; name a module-level fn``. A `Client`
yields one component.

| rejected | why |
|---|---|
| one component per system component, with the application linked in | Only drain swaps would then be possible. The application change, the common dev edit, would wait for every session to close. |
| one component per fn or module | Every call would cross the canonical ABI. The program is one file anyway (slice §4.1 "Imports and the standard library"). |
| Ignite-private boundary ABI using llvm §3 layouts | It is the same copy work as the canonical ABI, but unspecified and without interop. Wasmtime implements the canonical ABI for phase 1 [W20]. |

### 3.2 Imports come from effects

`ignite-check` already computes each function's declared or inferred effect set (slice §5, §5.1).
The roots of a component (its protocol role, or its exported application functions) determine
its imports. Each effect maps to the import interface that performs it:

| effect | import |
|---|---|
| `r.read`, `r.write`, `r.accept` on a stream or listener | `ignite:io/tcp.{read, write-all, close, accept, set-idle-timeout}`, the subset needed |
| `net.listen`, `net.connect` | `ignite:io/net.{bind, connect}` |
| `stdin.read`, `stdin.mode`, `stdout.write` | `ignite:io/stdio.{…}` |
| `process.exit` | `ignite:io/process.exit` |
| `s.read`, `s.write` on a `Store<T>` | `ignite:store/<T>.{get, values}`, `{next-id, put, remove, update}` |
| `spawn` | `ignite:task/spawn` |
| none (the free function `log`, slice §6) | `ignite:io/log`, always granted |

Effects rooted at a parameter (`stream.read`) grant the import, but only the handles the
instance holds can be used. Handles reach an instance only as arguments of its exports or as
results of imports it was granted. The checker records the effect set of each component in the
build manifest. The runtime linker refuses a component whose import list exceeds that set:
E083 is a runtime refusal and a compiler bug, since the generator derived both from one source.

For the examples: terminal's `server.application` imports only `log`, because `execute` is pure.
The Telnet `server` imports `tcp.{read, write-all, close}`, matching `requires [stream.read,
stream.write]` (role `serve` of `protocol Telnet` in `std/protocol/telnet.ignite`). todo's `server.application` imports
`store/todo` only, so it cannot open a socket.

### 3.3 Exports

| component | exports |
|---|---|
| `server` (protocol) | `serve: func(stream: own<tcp-stream>, application: <data fields>) -> result<_, io-error>`, plus `ignite-spawn-entry` |
| `server.application` | one function per cut point (`respond: func(line: string) -> reply`, `handle: func(state: borrow<store-todo>, request: request) -> response`), `application-data: func() -> <data fields>`, and the declared `migrate` functions (section 5.3) |
| `client` | `connect: func(stream: own<tcp-stream>) -> result<_, io-error>` |

### 3.4 Types in WIT and the canonical ABI

| Ignite (slice §6, llvm §3) | WIT [W8] | canonical ABI notes [W9] |
|---|---|---|
| `Int` | `s64` | |
| `Byte` | `u8` | |
| `Bool` | `bool` | |
| `Text` | `string` (UTF-8) | `(ptr, len)` in linear memory; the callee's `realloc` allocates in its arena |
| `Bytes` = `List<Byte>` | `list<u8>` | |
| `List<T>` | `list<T>` | |
| `Option<T>`, `Result<T, E>` | `option<T>`, `result<T, E>` | |
| tuple `(A, B)` | `tuple<A, B>` | |
| `Unit` | no result, or `tuple<>` in a payload | |
| record `type R { … }` | `record r { … }`, fields in kebab-case | |
| enum with payloads / without | `variant` / `enum` | |
| generic record or enum | one monomorphized `record`/`variant` per instance | |
| `TcpStream`, `TcpListener` | `resource tcp-stream`, `resource tcp-listener`; `own` for results of `accept`/`connect` and for `serve`'s argument, `borrow` for other parameters | an `i32` index into the instance's handle table [W9] |
| `Store<T>` | `resource store-<T>` per `T` | WIT resources are not generic |
| `IoError` | `record io-error { message: string, exit-code: s32 }` | |
| `Input` | `variant input { data(list<u8>), eof }` | |
| `fn(..)` value | not a WIT type; a cut point becomes an import binding (section 3.1) | |
| recursive type (`enum Json`, std `json.ignite`) | `list<u8>` holding a Ignite encoding | WIT's documented types include no recursive definitions [W8]. Non-interop; section 11, Q4. |

The guest's lift and lower glue is generated per type, next to the existing `copy.<T>` glue
(llvm §4). It converts between llvm §3 layouts and canonical ABI layouts. Strings and lists are
copied into the callee's arena through the canonical `realloc` option, and nothing is freed
after the call: the caller's loop region releases it (llvm §4).

### 3.5 The `ignite:*` world

The interfaces `ignite:io`, `ignite:store` and `ignite:task` describe `ignite-io` (slice §7) one
operation for one operation, as synchronous functions. Phase 1 defines them, and every later
phase implements the same WIT. This document rejects the Component Model async ABI of WASI
0.3 (`async func`, `stream<T>`, `future<T>` [W10]) for the internal boundary: guest tasks
suspend in the host (section 7), and async lowering in the guest would add a second suspension
mechanism. It is adopted only at an interop boundary (section 11, Q8).

## 4 The runtime written in Ignite

### 4.1 Layers

| layer | module | role |
|---|---|---|
| decoder | `std.wasm.decode` | Reads the binary format into module records: types, imports, functions, tables, memory, globals, exports, data, code. For components it reads a subset: one core module, function and resource imports and exports, the canonical options `memory`, `realloc` and `string-encoding=utf8`. Anything else is refused with a message. |
| validator | `std.wasm.validate` | Follows the spec's validation algorithm [W4]. It validates eagerly, before linking. The spec allows lazy per-function validation [W3]; eager validation is chosen so that a bad build is refused before the swap rather than trapping mid-request. |
| translator and interpreter | `std.wasm.exec` | Translates validated code into compact internal code with resolved branch targets and stack heights, then dispatches it in a loop. WAMR likewise ships a fast interpreter beside its classic one [W22], and wasmi is an interpreter with fuel metering [W23]. Fuel is counted at branches and calls. |
| canonical ABI | `std.wasm.canon` | Lifts and lowers WIT values between guest memories, and between guest memory and host Ignite values, driven by type descriptors from the component's type section. |
| linker | `std.wasm.link` | Resolves imports against host functions and against other instances' exports (protocol → application). It checks import lists against the manifest (section 3.2) and produces a pre-linked version, the analogue of Wasmtime's `instantiate_pre` [W20]. |
| instance manager | `std.wasm.instances` | Holds versions per component, instantiates one instance per task (section 7), and runs the swap state machine and rollback (section 5). |
| scheduler | none of its own | A guest task is a runtime Ignite task (section 7). |
| host bridge | `std.wasm.host` | One Ignite function per `ignite:*` import. It lifts the arguments, calls the Ignite builtin (`stream.read(n)`, `todos.get(id)`) and lowers the result. |
| system host | `runtime/wasmrt/wasmrt.ignite` | Reads the system manifest, binds listeners, runs the accept loops of slice §6.1 in Ignite, and serves the control connection. |

Tiering follows the existing runtimes: Wasmtime has the Pulley interpreter, the Winch baseline
compiler and Cranelift [W16, W18, W19]; Wasmer has Singlepass, Cranelift and LLVM [W21]; WAMR
has interpreters, AOT and a Fast-JIT-to-LLVM tier-up [W22]. The Ignite runtime starts with an
interpreter. Its compiled tier (phase 6) goes through LLVM rather than single-pass machine code
as Winch does [W19], because Ignite already owns an LLVM lowering.

The libraries live in `std/wasm/` because a program is one file plus the standard library
(`docs/slice.md` §4.1 "Imports and the standard library"). A `system` cannot live in a std module (E020, `docs/slice.md` §8 "Diagnostics"),
so the runtime's own system is a program under a new `runtime/` directory. `AGENTS.md`'s layout
table gains it when it lands.

```ignite
package runtime.wasmrt

import std.wasm.host.{Control, Runtime}

system WasmRuntime {
  resource control: TcpListener { address = "127.0.0.1", port = 0 }
  resource versions: Store<Bytes>
  component host: Service {
    listener = control
    protocol = Control
    application = Runtime { versions }
    executable = "ignite-wasmrt"
  }
}
```

The control connection runs a line protocol: `load <component> <hash> <length>` followed by the
bytes, then `status` and `rollback <component>`. It is the only way code enters the runtime,
which therefore needs no file system access. The guest system's own listeners (port 8080) are
bound by `Runtime` through `TcpListener.bind` (effect `net.listen`) from the manifest.

The interpreter core uses the gap features of section 4.3 (marked G1, G2, G4):

```ignite
const OP_BR: Int = 0x0C
const OP_I32_ADD: Int = 0x6A
const OP_I32_LOAD: Int = 0x28

enum Trap { Unreachable, OutOfBounds, OutOfFuel, CallDepth, Host(Text) }

fn run(code: Buffer, stack: Buffer, memory: Buffer, start: Int, base: Int, fuel: Int)
  -> Result<Int, Trap> requires [code.read, stack.*, memory.*] {
  var pc = start
  var sp = base
  var left = fuel
  loop {
    let op = code.load_u8(pc).to_int()                       // G2
    pc = pc + 1
    match op {                                               // G4: lowered to a switch
      OP_I32_ADD => {
        sp = sp - 8
        stack.store_i32(sp - 8, stack.load_i32(sp - 8).wrapping_add(stack.load_i32(sp)))  // G1
      }
      OP_I32_LOAD => {
        let address = stack.load_i32(sp - 8).to_unsigned() + code.load_i32(pc).to_int()
        if address + 4 > memory.len() { return Err(OutOfBounds) }
        stack.store_i32(sp - 8, memory.load_i32(address))
        pc = pc + 4
      }
      OP_BR => {
        pc = code.load_i32(pc).to_int()
        left = left - 1
        if left == 0 { return Err(OutOfFuel) }
      }
      _ => return Err(Unreachable)
    }
  }
}
```

### 4.2 What stays in the substrate

Everything that touches the operating system stays in `ignite-io`, unchanged in kind: sockets,
standard input and output, the terminal mode, process exit, tasks and (from slice 5) the tokio
event loop (`AGENTS.md`, Layout, second rule). The runtime reaches it through ordinary Ignite builtins, compiled
by the LLVM target into calls of `ignite-rt`, which calls `ignite-io` (llvm §5). The chain is
guest import → Ignite host bridge → builtin → `ignite-rt` → `ignite-io`, so there is still one
substrate. The phase 6 baseline compiler adds one substrate operation, loading a shared object
(section 4.3, G7). Nothing named wasm enters `crates/` except `ignite-llvm`'s wasm32 layout mode,
the driver's `--target wasm`, and the phase 1 scaffold host.

### 4.3 Language gaps

| gap | needed for | design | slice |
|---|---|---|---|
| G1 machine scalars | i32 and f32/f64 arithmetic | Builtin types `I32`, `F32`, `F64` (`Int` is i64). Wrapping and bit methods named after the wasm operators: `wrapping_add`, `and`, `or`, `xor`, `shl`, `shr_s`, `shr_u`, `rotl`, `clz`, `ctz`, `popcnt`, `lt_u`, `div_s`, `div_u`. Conversions `to_unsigned`, `reinterpret`, `trunc_sat`. IEEE semantics per design §6.3. Methods first; operators `& \| ^ << >>` later. | 9 |
| G2 `Buffer` | linear memory, the value stack, internal code | A builtin handle type like `Store`: copies share storage, and the storage lives outside the arenas (malloc'd, freed when the last handle drops). Bounds-checked little-endian `load_*`/`store_*` for every width, plus `len`, `grow`, `copy`, `fill`. The LLVM target lowers the accesses inline (no `ignite-rt` call per access); the Rust target uses a `ignite-io` type. Effects `b.read`, `b.write`. | 9 |
| G3 indexed assignment | translator tables, local slots | `xs[i] = e` on a `var` list, in place. `place` is `ident ('.' ident)*` today (`docs/slice.md` §3 "Grammar", rule `place`). | 10 |
| G4 dense `match` on `Int` | dispatch loop | Codegen only: a `match` on constant patterns of `Int` or `Byte` lowers to LLVM `switch` (jump table) without copying the scrutinee. | 10 |
| G5 multi-module programs | moving `std/wasm` out of std | Lift E031 for non-std imports (`docs/slice.md` §4.1 "Imports and the standard library"). | 11 |
| G6 task yield | fuel-based preemption on tokio | `Task.yield_now()`, effect `task.yield`. Threads (slice 4) do not need it; cooperative tokio tasks (slice 5) do. Tests need none either: slice 8 landed (`docs/slice.md` §12 "Slice 8: tests"), and `ignite test` runs each test as a child process with a timeout (§12.7 "Runner ABI"). | 5 |
| G7 load native code | phase 6 baseline compiler | Substrate `Native.load(bytes)` and `call`: `dlopen` plus a symbol table, in `ignite-io`. Trusted (section 8). | phase 6 |

**Conflict** with llvm §4 "Values and memory": "Releasing is sound because values never
share buffers (value semantics)". `Buffer` shares storage. Two things keep the region rule
sound: the storage is outside every arena, and the handle is copied like `TcpStream` (8 bytes,
no buffer of its own). Value semantics stays the rule for every other type. `Store<T>` already
has this shape (`impl Clone for Store` in `crates/ignite-io/src/store.rs` shares the map).

**Conflict** with design §12.6: "Avoid expanding the language solely to
eliminate a stable host dependency." Writing a runtime in Ignite does expand the language. G1–G4
are not solely for the runtime: the self-hosted compiler's byte encoders and code emitters
(design §12.4) need the same features. Timo's request (2026-11-01) makes the runtime a goal of
its own.

### 4.4 Bootstrap

`ignite build --target llvm runtime/wasmrt/wasmrt.ignite` produces `ignite-wasmrt`. The driver
finds it the way it finds `libignite_rt.a`, next to the running executable (llvm §1). The
compiler that builds the runtime is the Rust seed S0 (design §12.5). Once the compiler is
self-hosted it is C1 or later. No wasm runtime is ever needed to build the wasm runtime.

## 5 State and hot reload

### 5.1 What an instance holds

Ignite has no mutable globals. Constants are pure functions (slice §4.3 "Constants")
and the grammar has no global variable (slice §3). Resources are created by the system, not by a
component (slice §11.4). An instance's linear memory therefore holds only the arena of the task
it runs: values of in-flight calls and locals. Between two calls into an application component,
the instance holds nothing that a later call can observe. This property makes request-level
swaps lossless, and the reload design rests on it. A change that adds mutable global state
would break it. Actors (`docs/actors.md`) hold state, and that document owns its migration.

### 5.2 Resources live in the runtime

A `Store<T>` resource is a `Store<Bytes>` inside the runtime, keyed by the system's resource
name (resource `todos` of `system TodoService` in `examples/todo/todo.ignite`). Each value is held in a *stored encoding*: the canonical
ABI layout of `T`'s WIT type, with pointers replaced by inline length-prefixed payloads, so the
bytes are self-contained. `std.wasm.canon` converts between guest memory and the stored
encoding by type descriptor. The runtime tags the store with the WIT type hash of `T`. A reload
replaces instances and never touches the store unless the type hash changes. Listeners stay
bound across reloads, since they are runtime resources too.

**Rejected:** keeping stores inside a long-lived "state instance". It would make the store's
code reloadable and its memory untouchable at the same time, and Wasmtime frees memory only
when the whole `Store` drops [W15].

### 5.3 Schema changes

When `T` of a `Store<T>` changes, the compiler compares the new WIT type with the running one.
The driver fetches the running type with `status`.

| change | outcome |
|---|---|
| identical | no migration |
| record gains a field with a default (slice §4.7) | automatic: each value is decoded with the old type and the default is filled in. The default is pure, so this is deterministic. |
| anything else (field removed, type changed, variant changed, rename) | needs a declared `migrate`. Without one: E080, and the reload is refused. |

A rename is a removal plus an addition, as with identities in design §3.4. A migration is a
pure function declared in source and named by the resource:

```ignite
type TodoV1 { id: Int, title: Text, done: Bool }

enum State { Open, Done }
type Todo { id: Int, title: Text, state: State }

fn migrate_todos(old: TodoV1) -> Todo =
  Todo { id = old.id, title = old.title, state = if old.done { Done } else { Open } }

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo> { migrate = migrate_todos }
  component server: Service {
    listener = ingress
    protocol = Http
    application = HttpApp { state = todos, handle }
    executable = "todod"
  }
}
```

The resource grammar already allows properties (`docs/slice.md` §3 "Grammar", rule `resource`). New rules:

- `migrate` must be a pure fn `fn(Old) -> T` (E010 for an effectful one, as for defaults).
- `Old`'s WIT type must equal the running store's type hash: E081 ``migration source `TodoV1`
  does not match the running shape of `todos` ``, with the running shape printed as Ignite.

So the migration is type-checked against what is actually running, not against an old source
file. It is exported by the application component (section 3.3) and runs in a fresh instance,
value by value, during the swap of section 5.4. When the reload commits, `migrate` and `TodoV1`
can be deleted from the source. A native build (Rust or LLVM target) ignores `migrate`, since a
new process starts with empty stores.

The design keeps the general model in §7.3: "A schema migration
may require forward recovery rather than rollback. Old binaries refuse state they cannot
interpret." Actor state migration (`code_change`) is `docs/actors.md`; it should reuse E080 and
E081 and the stored encoding.

### 5.4 Swap points

There are two kinds of swap, chosen by which component changed.

**Quiescent swap (application components).** The runtime keeps per task the instance of the
application version it last called. At each cut-point call it compares that version with the
component's current version and, when they differ, instantiates the current one. The swap is at
a call boundary, where the old instance holds nothing (section 5.1). For a Telnet session, the
next line is answered by the new `execute` and the session stays open. For HTTP, the next
request on a keep-alive connection uses the new `handle`. A schema-changing reload runs as
follows:

1. Close admission to the component's cut points: new calls wait.
2. Wait for in-flight calls, up to a deadline (default 5 s; a call over it traps with "reload
   deadline").
3. Snapshot the store's encoded values.
4. Run `migrate` over them.
5. Switch the version and reopen admission.

The old and new versions never touch one store with different schemas at the same time. Design
§2 allows two versions against the same resources only under "an
explicitly supported coordination model". The coordination model here: store operations are
serialized by the runtime, and versions coexist only while the store schema is unchanged.

**Drain swap (protocol components).** This reuses the design's admission handoff and old-session
drain (design §11.8; `DrainRollout` in design §11.3). The
runtime owns the listener and the accept loop, so it is the fronting acceptor that design §11.8
requires: new connections instantiate the new protocol version, and open sessions keep their old
instance until they end or the drain deadline passes (`on_deadline = "disconnect_remaining"`, a
forced close classified as in design §11.10). Like Erlang, which keeps a
current and an old version of a module and purges the old one on the next load [W25], at most
two versions of a component run at once. A third load while the old one still drains closes the
oldest sessions first, with the forced-close classification. Unlike Erlang, the switch does not
depend on fully qualified calls [W25]: the swap points are fixed by the cut and by admission.

**Rejected:**

- Mid-call swap with stack migration (DSU-style). It needs frame mapping between versions and
  breaks G1.
- Swapping only on explicit `code_change` messages (Erlang style [W26]). It belongs to actors
  (`docs/actors.md`), and the Service/application split gets request-level swaps without
  anything in user code.
- Embedder-only swaps with no defined swap point. Wasmtime documents the primitives, a new
  `Store` and a `Linker` instantiation [W15, W20], and leaves the routing to the embedder; this
  design uses the same primitives. wasmCloud's `update component` moves a running component to a
  newer build [W30]; the source cited does not describe where in-flight calls switch.

### 5.5 Interface changes between components

The interface of a cut point is its WIT signature, normalized and hashed. The canonical ABI
lays out each value by its type [W9], and instantiation type-checks the supplied imports against
the declared ones [W15]. Ignite therefore counts only an unchanged interface hash as compatible. An unchanged hash means only one side is reloaded. Any change is breaking: for example,
`Reply` in `std.console` gaining a field changes both `server` and `server.application`. A
breaking change makes a coordinated reload:

1. The driver rebuilds every component whose interface hash changed and ships them as one batch.
2. The runtime pre-links the new pair.
3. New sessions get the new protocol instance bound to the new application version.
4. Open sessions keep the old pair until they drain.

The batch is atomic: if one member fails validation or linking, none is swapped. Semver
matching of interface names, as Wasmtime's linker does [W20], is rejected: it would accept two
interfaces that are equal in name only.

### 5.6 Rollback

A committed version stays loaded as `previous` for an observation window after each swap. The
window lasts for the first 100 calls or 10 s, whichever is longer (provisional, see section
1.2). During the window, any of these rolls the component back:

- a trap in the new version;
- a failed health probe, once probes exist (design §11.3 `promptProbe`);
- `rollback` sent by the driver.

The runtime then:

1. Switches the cut points back to `previous`. A drain swap redirects only new admissions.
2. When a migration ran, restores the snapshot of section 5.4 and reports the number of writes
   discarded. This is the development mode's choice. Under `ignite apply` it would contradict the
   forward-recovery rule of design §7.3, so a non-development host instead reports "recovery
   required" (design §7.4) and keeps the new version.
3. Marks the version rejected, so that the same hash is not loaded again until the source
   changes.

Design §4.2 applies: rollback is a state transition with its own
failure cases. A rollback that cannot restore (the snapshot is lost because the runtime was
restarted) reports and stops admission to the component.

### 5.7 Equivalence with the native targets

A program that is never reloaded must behave on wasm exactly as on the Rust and LLVM targets:
same protocol traces and same exit statuses (slice §6.1). The test suites of section 9 check
this. Reload is outside the language: in a trace, each version segment must be equivalent to a
native run of that version. A store migration is a step between segments.

Task failure is one existing divergence. On the Rust target, a panic ends its task's thread
(`docs/slice.md` §7, "Substrate `ignite-io`"). On the LLVM target, `ignite_rt_panic` exits the process with status
101 (`docs/llvm.md` §4 "Values and memory"). The wasm target follows the Rust target: a trap ends the task.
Section 11, Q1 asks for one rule for all three targets.

## 6 Watcher and incremental compile

| question | chosen | rejected | why |
|---|---|---|---|
| where the watcher runs | the `ignite` driver (Rust); file events through a notify library | the runtime | The compiler is Rust until M6, and the runtime then needs no file system authority. |
| frontend granularity | whole program plus std on every change | per-module incremental checking | The frontend runs in milliseconds on these sizes, and slice §1 checks a program completely before any target runs. |
| backend granularity | per component | per module or per fn | The component is the unit of swap. |

**Dependency tracking.** After typing, the driver computes each component's *reachable set*:
every function, constant, field default, record, enum and alias, after monomorphization,
reachable from its roots (section 3.1). The modules involved are the program and any std module
that contributes a definition. A change in `std/text.ignite` affects exactly the components whose
reachable set includes a changed definition, not every component that imports the module.

**Cache key** (SHA-256 over):

1. the compiler's build identity;
2. the target and codegen mode (`wasm32-lime1`, `llvm-O0` or `emitter`);
3. the component's reachable definitions, each printed canonically from its typed AST (no spans,
   no comments, names fully qualified, sorted by mangled name, llvm §3);
4. the WIT text of its imports and exports;
5. the granted effect set.

A comment-only edit is a cache hit, so no reload happens. As in Unison, where a definition is
identified by a hash of its syntax tree and a cache keyed that way "is never invalidated" [W27],
the key covers content only. Unlike Unison, names are part of the content: Ignite symbols are
named in the IR (`docs/llvm.md` §3 "Representation", symbol names). Artifacts are kept in `target/ignite-wasm/<system>/cache/
<key>.wasm`, as the LLVM target's stamps already are (`docs/llvm.md` §1 "Pipeline"). The key doubles as
the version identity the runtime reports. The runtime's version number is a counter per
component.

## 7 Concurrency and async

| question | chosen | rejected | why |
|---|---|---|---|
| tasks in a component | one instance per Ignite task | wasm threads with shared memory; one instance with green threads on stack switching | Ignite tasks share no values (spawn deep-copies, llvm §4), so separate memories lose nothing. Threads are Tier 2 and unsupported on Pulley, and stack switching is Phase 3 [W5, W16]. |
| guest suspension | in the host: a guest task is a runtime task | CPS or state-machine lowering in the guest; Asyncify [W24]; stack switching | The interpreter's guest stack is data in `Buffer`s, so suspending the runtime task suspends the guest. One mechanism serves native and wasm. |
| async boundary | synchronous `ignite:*` imports | CM async, `stream<T>`/`future<T>` [W10] | The guest never needs to see readiness or completion. |

**Interpreter tier.** Each guest task runs `run` (section 4.1) inside one runtime Ignite task.
When the guest calls an import such as `tcp.read`, the host bridge calls `stream.read(n)`:

- **Slice 4:** it blocks that task's OS thread, as any Ignite task does (slice §7).
- **Slice 5:** tokio is the event loop inside `ignite-io`, and the LLVM target lowers Ignite tasks
  to coroutines run as tokio tasks (`AGENTS.md`, Layout, second rule). The runtime is an LLVM-target program, so
  its tasks, and the guests inside them, suspend that way with no extra work.

A guest that computes without calling an import would never suspend under tokio. The
interpreter therefore calls `Task.yield_now()` (G6) every 10 000 fuel units.

**Compiled tier (phase 6).** Guest code compiled to native code cannot suspend inside the
runtime's coroutine frame. Each compiled guest task runs on a bounded worker thread, and host
imports block on the event loop from there. This is design §8.7's "blocking FFI runs on bounded
workers". Coroutine lowering of guest code is rejected until a
measurement shows the worker pool to be the bottleneck.

**Phase 1 (Wasmtime scaffold).** One Wasmtime `Store` per task, which isolates the task and
frees its memory when the task ends [W15]. Sync host functions until slice 5, then Wasmtime's
async support.

## 8 Security

- **Memory.** Every guest load and store goes through `Buffer`'s bounds check. An out-of-range
  access is a trap, never a host access. Each instance has a maximum memory (default 64 MiB,
  configurable per component) and a maximum call depth, so a guest traps instead of exhausting
  the host stack.
- **Authority.** Imports are the authority (section 3.2). Handles are per-instance indices with
  a generation, as in the native handle table (module doc and `encode` in `crates/ignite-rt/src/net.rs`). An instance
  can use only the handles it was given.
- **Limits.**
  - The interpreter uses fuel: deterministic, matching design §6.3's determinism relative to
    recorded inputs. Wasmtime documents fuel as deterministic and more costly, and epochs as about
    10 % overhead and non-deterministic [W17]. A deterministic counter is nearly free in an
    interpreter's branch handler.
  - Each call into an application component gets a fuel budget (default 10⁹ units). Exhausting it
    traps the call, so the request fails (HTTP 500, Telnet session closed).
  - Protocol tasks have no budget, only the yield of section 7.
  - The compiled tier uses epoch checks at loop headers, since fuel there costs more [W17].
- **Containment.** A trap ends its task only. Its instance is discarded and the error is logged
  as `session ended with error: trap: <kind>` (slice §6.1). Repeated traps go to the component's
  `Restart` policy (`recovery = Restart` in design §11.3). Past `maximum_attempts` within the
  `interval`, the component is failed. If its failures started after a swap, it is rolled back
  (section 5.6); otherwise its listener stops admitting and the runtime reports. Supervision
  trees and restart strategies above the component level are in `docs/actors.md`.
- **Control channel.** It is loopback only. A per-run token, written by the driver to the
  runtime's standard input at start, must be the first line of every connection.
- **Trust.** The sandbox is only as sound as the runtime that enforces it. Design §8.5 forbids
  calling Ignite memory-safe without evidence. Design §8.8 requires an
  isolation mechanism for untrusted modules, "such as a separate process". Wasm instance isolation is proposed as such a mechanism. Until the
  runtime has fuzzing and spec-suite evidence (section 9), untrusted components still run in a
  separate runtime process per trust domain. Phase 6's natively compiled code is generated by the
  trusted translator with explicit bounds checks; `Native.load` is trusted substrate code.

## 9 Testing

| suite | what | from phase |
|---|---|---|
| inline `test` blocks and `assert` (slice 8, landed: `docs/slice.md` §12 "Slice 8: tests"; `crates/ignite/tests/ignite_test.rs`) | `ignite test --target wasm` compiles each test block into an export of a test component and runs it on the runtime, with the runner flags and exit statuses of §12.7 "Runner ABI" (0 passes, 101 for a failed `assert`, a failed `?` or a panic). The results must equal those of `ignite test --target rust` and `--target llvm`. | 1 |
| end-to-end, all targets | `crates/ignite/tests/terminal.rs` and `todo.rs` gain `TARGET=wasm`; every case passes unchanged (slice §9, §11.6) | 1 |
| authority | A fixture whose `respond` is pure yields an application component with no `tcp` import. A hand-edited component that adds `net.connect` is refused (E083). | 1 |
| spec conformance | the official testsuite [W31], converted from `.wast` by external tooling at test time, restricted to `lime1` [W12] | 2 |
| differential | the same components on `ignite-wasmrt` and the Wasmtime scaffold: equal outputs and equal trap kinds | 2 |
| reload: stateless | A Telnet session is open; `respond`'s text changes; the next line has the new text and the session stays open. | 3 |
| reload: in flight | 16 keep-alive connections send continuously during 20 reloads of `handle`: zero failed requests, and every response comes from the old or the new version. | 3 |
| reload: state | Create 3 todos and reload: `GET /todos` returns the same 3. Add `priority: Int = 0`: automatic. Change `done` to `state` with `migrate`: migrated values. Change without `migrate`: E080, and the old version keeps serving. | 4 |
| rollback | A new `handle` traps on its first call: the next request is served by the old version, the store is restored, and the version is reported as rejected. | 4 |
| drain | Edit the protocol component: open sessions finish on the old version, new sessions get the new one, and the deadline closes the rest as forced. | 4 |
| emitter equivalence | All suites above with the emitter instead of the LLVM path and no clang on `PATH` | 5 |
| tier equivalence | spec testsuite and end-to-end suites on the compiled tier equal to the interpreter | 6 |

## 10 Phased roadmap

Sizes are rough line counts of new code, to compare phases. They are not estimates of time.

| phase | deliverable | acceptance test | size | depends on |
|---|---|---|---|---|
| P0 | slices 9 and 10: G1–G4 on both native targets | fixtures for each new builtin and diagnostic; a `match` over 200 `Int` constants lowers to one `switch` (IR golden); `Buffer` load/store round-trips on both targets | 2–3k Rust | slice 8 |
| **P1** | `ignite build --target wasm` through `ignite-llvm` (wasm32 layout, guest arena in IR, lift/lower and pack glue, WIT emission, `wasm-tools component new` [W32]) and `ignite-wasmhost`, a Wasmtime embedding whose host functions call `ignite-io` | `crates/ignite/tests/terminal.rs` and `todo.rs` pass with `TARGET=wasm` on the scaffold host; the terminal application component's import list is `ignite:io/log` only; the authority fixture of section 9 is refused | 2–3k Rust | none |
| P2 | `std/wasm/*` and `runtime/wasmrt`: decoder, validator, interpreter, canonical ABI subset, linker, host bridge | spec testsuite (`lime1` subset) passes; P1's suites pass on `ignite-wasmrt`; differential against the scaffold; throughput of `todod` measured against the LLVM target (decides Q2) | 7–10k Ignite | P0, P1 |
| P3 | `ignite run --watch`, control channel, per-component cache, quiescent swap of application components | the stateless and in-flight reload tests; save-to-reload latency measured, and section 1.2 replaced by the measurement | 1.5k Ignite, 1k Rust | P2 |
| P4 | stored encoding, `migrate`, E080 and E081, snapshot and rollback, drain swap, coordinated reload | the state, rollback and drain tests | 1.5–2k Ignite, 0.5k Rust | P3 |
| P5 | Code IR serialization (Rust frontend) and a direct wasm emitter in Ignite, run inside the runtime | P1–P4 suites with the emitter and no clang; reload latency at most half of P3's | 1.5k Rust, 5–7k Ignite | P1 (ABI), Q3 |
| P6 | baseline compiler: validated wasm → LLVM IR with bounds checks and epoch checks → shared object → `Native.load` (G7); interpreter as first tier | tier equivalence; `todod` throughput at least 5× the interpreter's | 4–6k Ignite, 0.3k Rust | P2 |

**What unblocks what.**

- P1 fixes the WIT world, the type mapping and the guest ABI. Every later phase builds on it, and
  it can start now, in parallel with P0.
- P0 and P1 together unblock P2.
- P2 → P3 → P4 is the reload line.
- P5 and P6 are independent of each other and of P3/P4.

**Conflict** with `AGENTS.md`, Layout, first rule ("the only hand-written Rust a compiled program runs is
`crates/ignite-io` … or `crates/ignite-rt`"). P1's `ignite-wasmhost` runs Wasmtime. It is a
scaffold: from P2 on it is used only as the differential oracle in tests and is never the host
that `ignite run` starts.

## 11 Risks and open questions

| # | question | waits on |
|---|---|---|
| Q1 | Task failure diverges: the Rust target ends the task (`docs/slice.md` §7), LLVM exits 101 (`docs/llvm.md` §4), wasm ends the task. Which is the language rule? | the slice that settles task failure, before P2's equivalence gate |
| Q2 | Is an interpreter written in Ignite fast enough for the dev loop? Pulley, a tuned interpreter, expects about 10× below native [W18]. | P2's `todod` throughput measurement. If it is below 1/50 of native, P6 moves before P5. |
| Q3 | What is the Ignite Code IR, and how is it serialized? | design §8.5 Code IR definition (M3/M5) |
| Q4 | Recursive types cross a boundary as an opaque `list<u8>` (section 3.4). Is that acceptable, or should crossing be refused? | P1 interface review; any recursive-type support in WIT [W8] |
| Q5 | `Buffer`'s shared-handle semantics against value semantics (section 4.3) | Timo's language decision for slice 9 |
| Q6 | Store migration and actor `code_change` must share one encoding and one diagnostic set | `docs/actors.md` settling its migration model, before P4 |
| Q7 | Assemblies: how does a non-development assembly place components on `ignite-wasmrt` (design §5, §11.3 `assembly local`)? | the slice that introduces assemblies |
| Q8 | Interop: when do foreign components and WASI 0.3 (`async func`, `stream<T>`, `future<T>` [W10]) enter, and with them the full component binary format? | a concrete foreign component to run; after P4 |

## 12 Conflicts with stated decisions

| proposal | conflicts with | resolution here |
|---|---|---|
| latency targets in section 1.2 | design §1.5 | targets only; replaced by the P1/P3 measurement |
| `Buffer` shares storage | llvm §4 | storage outside arenas, handle copied like `TcpStream` |
| runtime in Ignite expands the language | design §12.6 | G1–G4 also serve the self-hosted compiler; Timo's direction |
| Wasmtime scaffold host | `AGENTS.md`, Layout, first rule | P1 only; a test oracle afterwards |
| WASI triple rejected | would conflict with `AGENTS.md`, Layout, second rule | `wasm32-unknown-unknown` with `ignite:*` imports |
| two versions share a store during a drain | design §2 | allowed only with an unchanged schema; runtime-serialized operations |
| snapshot rollback after a migration | design §7.3 | development mode only; elsewhere forward recovery and "recovery required" |

## 13 References

| ID | Source | Use |
|---|---|---|
| W1 | [WebAssembly Core Specification 3.0](https://webassembly.github.io/spec/core/) | core semantics |
| W2 | [Spec change history](https://webassembly.github.io/spec/core/appendix/changes.html) | 2.0 and 3.0 feature lists |
| W3 | [Implementation limitations](https://webassembly.github.io/spec/core/appendix/implementation.html) | lazy validation allowed |
| W4 | [Validation algorithm](https://webassembly.github.io/spec/core/appendix/algorithm.html) | validator |
| W5 | [WebAssembly proposals](https://github.com/WebAssembly/proposals) | phases of stack switching, threads, shared-everything threads |
| W6 | [Stack switching explainer](https://github.com/WebAssembly/stack-switching/blob/main/proposals/stack-switching/Explainer.md) | `cont.new`, `resume`, `suspend`, `switch` |
| W7 | [Why the Component Model](https://component-model.bytecodealliance.org/design/why-component-model.html) | shared-nothing components |
| W8 | [WIT](https://component-model.bytecodealliance.org/design/wit.html) | WIT types, worlds |
| W9 | [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) | lifting, lowering, per-instance handle table |
| W10 | [WASI 0.3](https://bytecodealliance.org/articles/WASI-0.3) | native async, `stream`, `future`; Wasmtime 46 |
| W11 | [wasi-sdk](https://github.com/WebAssembly/wasi-sdk) | `wasm32-wasip1/p2/p3` targets |
| W12 | [LLVM: the lime1 CPU](https://llvm.googlesource.com/llvm-project/+/35cce408eef1a253df12c0023c993d78b180b1f3) | fixed guest feature set |
| W13 | [LLVM PR 85802](https://github.com/llvm/llvm-project/pull/85802) | `wasm-component-ld` for `wasm32-wasip2` |
| W14 | [rustc wasm32-wasip2](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html) | the target emits a component |
| W15 | [Wasmtime architecture](https://docs.wasmtime.dev/contributing-architecture.html) | Engine, Store, Instance; memory freed when the Store drops |
| W16 | [Wasmtime stability tiers](https://docs.wasmtime.dev/stability-tiers.html) | proposal support per compiler, Winch, Pulley |
| W17 | [Wasmtime: interrupting execution](https://docs.wasmtime.dev/examples-interrupting-wasm.html) | fuel and epochs |
| W18 | [Wasmtime: Pulley](https://docs.wasmtime.dev/examples-pulley.html) | interpreter, about 10× slowdown |
| W19 | [Wasmtime baseline compilation RFC (Winch)](https://github.com/bytecodealliance/rfcs/blob/main/accepted/wasmtime-baseline-compilation.md) | single-pass baseline compiler |
| W20 | [Wasmtime component `Linker`](https://docs.rs/wasmtime/latest/wasmtime/component/struct.Linker.html) | `instantiate_pre`, semver matching |
| W21 | [Wasmer features](https://docs.wasmer.io/runtime/features) | Singlepass, Cranelift, LLVM backends |
| W22 | [WAMR](https://github.com/bytecodealliance/wasm-micro-runtime) | classic and fast interpreters, AOT, Fast JIT to LLVM JIT tiering |
| W23 | [wasmi](https://github.com/wasmi-labs/wasmi) | interpreter with fuel metering |
| W24 | [Binaryen Asyncify](https://github.com/WebAssembly/binaryen/blob/main/src/passes/Asyncify.cpp) | unwind and rewind by instrumentation |
| W25 | [Erlang code loading](https://www.erlang.org/doc/system/code_loading.html) | current and old code, purge on the third load |
| W26 | [Erlang gen_server `code_change`](https://www.erlang.org/doc/apps/stdlib/gen_server.html) | state migration on upgrade |
| W27 | [Unison: the big idea](https://www.unison-lang.org/docs/the-big-idea/) | hash-identified definitions, a cache never invalidated |
| W28 | [wasi-threads](https://github.com/WebAssembly/wasi-threads) | legacy status |
| W29 | [shared-everything-threads](https://github.com/WebAssembly/shared-everything-threads) | successor threads proposal |
| W30 | [wasmCloud `update component`](https://docs.rs/crate/wash-lib/0.25.1/source/src/cli/update.rs) | component update in a running host (prior art) |
| W31 | [WebAssembly testsuite](https://github.com/WebAssembly/testsuite) | spec conformance tests |
| W32 | [wasm-tools](https://github.com/bytecodealliance/wasm-tools) | `component new` wrapping a core module |
