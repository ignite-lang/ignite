# Fixed-width integers, fixed-size lists and a capacity hint

**Status: design, not implemented.**
**Date:** 8 November 2026
**Story:** `story:fixed-width-ints` of `epic:native-parity` (goal set by Timo, 2026-11-08: Ignite
programs within 1.1x of plain Rust).

This document decides. Each section states the chosen design and the alternatives it rejects.
"slice §n" is `docs/slice.md` (§25.2 wrapping methods, §25.4 `l[i] = v`, §29 bitwise operators
of wave 7, §24 the typed document), "llvm §n" is `docs/llvm.md`.

## 1 The problem, measured

The codecs benchmark (slice §29.5: SHA-1 and base64 over 1 MiB, 16 rounds,
`crates/ignite/tests/fixtures/bitwise/codecs.ignite` against `codecs.rs`) is 2.10x plain Rust on
the LLVM target after the alias work of wave P1 (2.94x before it). What the reference has and
Ignite lacks:

| reference | Ignite today | cost (epic body, `perf record`) |
|---|---|---|
| `u32` with `wrapping_add` | `Int` and `& 0xFFFFFFFF` after every sum (`.wrapping_add(x) & MASK`) | one `and` per term; 64-bit registers for 32-bit data |
| `u32::rotate_left` (one `rol`) | `(x << n \| x >> 32 - n) & MASK`: two shifts with range `select`s, `or`, `and` | about 5 instructions per rotate, 3 rotates per round |
| `[u32; 80]` on the stack, constant length | `List<Int>` of 80: heap header, `len` loaded, bounds check per `w[t]` | the loop reloads the data pointer and checks 5 indexes per round |
| `Vec::with_capacity` | `push` from empty, doubling | 30.6k page faults against 1.0k (5.8x in the codecs run), 200 `brk`; kernel 10 % of the profile |

SHA-1 is 64 % of the profile, base64 20 %.

## 2 Decision: the minimal set

Three features reach the reference's shape for codecs; each removes one row above.

| # | feature | removes |
|---|---|---|
| F1 | `List.with_capacity(n)`, `Bytes.with_capacity(n)` | the page faults and the copies of doubling |
| F2 | `U32`: a 32-bit modular word; `+ - *` wrap; bitwise operators of §29 with a logical `>>`; `rotate_left`, `rotate_right`; explicit conversions | the masks and the rotate sequence |
| F3 | fixed-size lists `[T; N]` of scalars, inline, constant length | the header reload and the bounds checks LLVM can prove |

Not in the minimal set, each on the same rules when a consumer needs it:

- `U8`: `Byte` already is the unsigned 8-bit type. It gains `wrap_byte` (section 3.4) and
  nothing else here. No alias `U8`.
- `U64`: SHA-512, 64-bit hashes. Same rules as `U32` (section 7).
- `I32`, a signed fixed width: wasm G1 (`docs/wasm.md` §4.3) needs it; it adds signed division
  and an arithmetic `>>`, and is designed with wasm P0 (section 7).
- A big-endian load `b.u32_be(at)`: the reference's `from_be_bytes`. Only if measurement after
  F1 to F3 is still above 1.1x (Q3).

## 3 `U32`

### 3.1 Values and operators

`U32` holds 0 to 2^32 - 1. It is a word, not an integer: its arithmetic is modulo 2^32. That is
the reason it exists, so `+`, `-`, `*` wrap and never fail; `Int` keeps failing on overflow
(vision T6, slice §25.2).

| operation | `U32` | note |
|---|---|---|
| `a + b`, `a - b`, `a * b` | modulo 2^32 | never fails |
| `-a` | `0 - a` modulo 2^32 | |
| `a / b`, `a % b` | unsigned | `b == 0` is the `Int` division failure, same report |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | unsigned | |
| `a & b`, `a \| b`, `a ^ b`, `~a` | on 32 bits | §29 operators, extended |
| `a << n`, `a >> n` | `n: Int`; `>>` is logical; `n` outside `0..32` gives 0 | §29.2's rule at width 32 |
| `a.rotate_left(n)`, `a.rotate_right(n)` | `n: Int`, taken modulo 32 (`n & 31`) | never fails |
| `a.to_text()`, `debug()` | decimal | |

### 3.2 Literals and typing

- An integer literal is typed by its expected type, as for `Byte` today (llvm §2):
  `let k: U32 = 0x5A827999`. Outside 0 to 2^32 - 1 it is E044 (`integer literal out of range
  for U32`), extended from `Byte`. A literal with no expected type is `Int`.
- A binary operator's operands have one type; the literal side takes the other side's type
  (`x & 0xFF`, `x + 1`). Mixing widths is E040 with the existing text (`expected U32, found
  Int`); there is no implicit widening.
- Shifts and rotates take an `Int` amount whatever the left type, so `x << 8` and `x << n`
  need no conversion. The result has the left operand's type.
- Patterns: integer literal and range patterns on a `U32` scrutinee are typed as `U32`; E046
  splits `U32` into intervals as it does `Int` and `Byte`.
- `U32` is a builtin type name (`ignite_check::BUILTIN_TYPES`): declaring a type `U32` is E004.

### 3.3 Conversions

Explicit only. Widening is total; narrowing is checked or wrapping, by name.

| from \ to | `Int` | `U32` | `Byte` |
|---|---|---|---|
| `Int` | | `n.to_u32() -> Option<U32>`, `n.wrap_u32() -> U32` (low 32 bits) | `to_byte` (exists), `n.wrap_byte()` |
| `U32` | `x.to_int()` | | `x.to_byte() -> Option<Byte>`, `x.wrap_byte()` |
| `Byte` | `to_int` (exists) | `b.to_u32()` | |

### 3.4 Target form

```ignite
fn rounds(w: [U32; 80], h: [U32; 5]) -> [U32; 5] {
  var a = h[0]
  var b = h[1]
  var c = h[2]
  var d = h[3]
  var e = h[4]
  var t = 0
  while t < 80 {
    var f = b ^ c ^ d
    var k: U32 = 0xCA62C1D6
    if t < 20 {
      f = b & c | ~b & d
      k = 0x5A827999
    } else if t < 40 {
      k = 0x6ED9EBA1
    } else if t < 60 {
      f = b & c | b & d | c & d
      k = 0x8F1BBCDC
    }
    let temp = a.rotate_left(5) + f + e + k + w[t]
    e = d
    d = c
    c = b.rotate_left(30)
    b = a
    a = temp
    t = t + 1
  }
  [h[0] + a, h[1] + b, h[2] + c, h[3] + d, h[4] + e]
}
```

Against `codecs.ignite`: no `& MASK`, no `wrapping_add` chain, one `rol` per rotate, and `w` a
320-byte value on the stack instead of a `List<Int>` on the heap. `k` needs its annotation: a
literal with no expected type is `Int`.

## 4 Fixed-size lists `[T; N]`

### 4.1 Syntax

| form | meaning |
|---|---|
| type `[T; N]` | a list of exactly `N` values of `T` |
| expression `[v; N]` | `N` copies of `v` |
| expression `[a, b, c]` where `[T; 3]` is expected | a list literal of that length; another length is E040 |

`N` is an `Int` literal or an `Int` constant, at least 1 and at most 65,536 bytes of elements
(`N` times the element size); otherwise E058 (`fixed-size list length must be a constant from 1
to 65536 bytes`). `;` is a new token inside `[ ]` only.

### 4.2 Typing and behaviour

- `T` is a scalar: `Int`, `U32`, `Byte`, `Bool` (later `U64`). Any other element type is E058.
  This keeps drop, region and handle glue out of F3; lifting it is section 7.
- A value, like a record of `N` fields: assignment and argument passing copy, last-use moves
  (slice §20.3) apply. `==` compares elementwise.
- `w[i]` reads, `w[i] = v` writes (slice §25.4; E006 through a `let`). An index outside
  `0..N` is the `index` failure with the same report as a `List`'s. A constant index outside
  `0..N` is E059 at check time (`index 80 out of range for [U32; 80]`).
- `w.len()` is the constant `N`; `for x in w` iterates; `w.to_list()` gives a `List<T>`.
- No `push`, `append`, `clear` or `set` beyond the index form: E041.
- A `const` may be a fixed-size list (`const K: [U32; 64] = [0x428a2f98, ..]`), literal data.
- `[T; N]` is a type, not `List<T>`: a parameter of type `List<U32>` does not take one (E040).
  There are no length generics.

### 4.3 Bounds checks

The frontend does no range analysis. It emits each check in a form the optimiser removes when
the index is provably in range: an unsigned compare of the index with the constant `N`, the
failure branch cold and `noreturn` (`story:llvm-alias-and-cold`). Under `while t < 80` the
check of `w[t]`, and of `w[t - 3]` when `t` starts at 16, is folded by LLVM's induction
variable and correlated-value passes at `-O2`; rustc does the same on the Rust target. A
constant index needs no check at all (E059 covers the out-of-range one).

## 5 Capacity hint

| Ignite | type | meaning |
|---|---|---|
| `List.with_capacity(n)` | `(Int) -> List<T>`, `T` from the expected type as `List.empty()` | an empty list with room for `n` elements |
| `Bytes.with_capacity(n)` | `(Int) -> Bytes` | the same for bytes |

Only speed is observable: `len()` is 0, `push` beyond `n` still grows. `n < 0` is taken as 0.
The evaluator ignores the hint.

```ignite
fn base64(data: Bytes) -> Text {
  var out = Bytes.with_capacity((data.len() + 2) / 3 * 4)
  ..
}
```

## 6 Layers

### 6.1 Per layer

| layer | `U32` | `[T; N]` | `with_capacity` |
|---|---|---|---|
| `ignite-syntax` | no change (a type name) | type `[T; N]`, expression `[v; N]`; token `;` in brackets | none (a static call) |
| `ignite-check` | `U32` in `BUILTIN_TYPES`; `rotate_left`, `rotate_right`, `wrap_u32`, `wrap_byte`, `to_u32` in `BUILTIN_METHODS` | E058 | `with_capacity` as a static of `List` and `Bytes` |
| `ignite-types` | `Type::U32`; literal typing and E044; operator table; E040 on mixed widths | `Type::Array(T, N)`; E058, E059; moves treat it as a value | builtins `ListWithCapacity`, `BytesWithCapacity` |
| `ignite-eval` | `Value::U32(u32)`, `u32::wrapping_*`, `rotate_*` | the evaluator's list value with the length fixed by the type | the hint ignored |
| Rust target | `u32`, `wrapping_add` and friends, `rotate_left((n & 31) as u32)`, a shift by a constant emitted directly | `[T; N]`, index through `ignite_io::checked::at` so the report stays Ignite's | `Vec::with_capacity(max(n, 0) as usize)` |
| LLVM target | `i32` (size 4, align 4 in records and list elements); `add`/`sub`/`mul` without overflow intrinsics; `lshr`; `llvm.fshl.i32(x, x, trunc n)` for rotate (the intrinsic takes the amount modulo 32) | `[N x T]` inline in frames and records, by pointer to the caller's copy when passed (copied before a write); constants as `internal constant` globals | one arena allocation of `n` elements |
| typed document (slice §24) | type `kind` `U32`, no further keys | type `kind` `Array`, `args` `[T]`, `length` `N`; expression `kind` `repeat` with `value` and `count` | a `call` with a `builtin` callee |
| system model (slice §15.5) | a JSON number | a JSON array | none |
| apply identity | `CODEGEN_REVISION` bumped once per story that changes generation | | |

The typed document adds kinds and keys only, so it stays `ignite-typed/0.1` (§24.3). How a
`U64` literal above `Int`'s range appears in `value` is decided with `U64`.

### 6.2 With wave 7's operators (slice §29)

§29.2 makes every non-`Int` operand of `& | ^ << >> ~` E040. F2 lifts that for `U32` only:
both sides `U32` for `& | ^`, a `U32` left side with an `Int` amount for the shifts, a `U32`
operand for `~`. `Byte` stays E040 until a consumer needs it. Precedence is unchanged.
§29.4's agreement test gains a `U32` column (the evaluator, both targets, Rust's `u32`).

### 6.3 `std.digest` (`story:std-digest`)

`story:std-digest` depends on `story:bitwise-operators` and asks for 1 MiB within 3x of the
Rust target. Written on F1 to F3 it can target the epic's 1.1x instead: SHA-1 and SHA-256 with
`[U32; 80]`, `[U32; 64]` and `const K: [U32; 64]`; the incremental form a record holding
`h: [U32; 8]` and the pending `Bytes`; base64 with `Bytes.with_capacity`. Recommended: it
depends on F1 to F3 (add the relation when it is next planned), and the codecs fixture moves
onto `std.digest` once both exist.

## 7 Later, on the same rules

| item | rule |
|---|---|
| `U64` | as `U32` at width 64; `wrap_u64`, `to_u64`; the SHA-1 length field and SHA-512 |
| `I32` | modular `+ - *` like `U32`; signed `/`, `%`, ordering; arithmetic `>>`; wasm G1 adopts these names (`rotate_left`, not wasm's `rotl`) |
| non-scalar `[T; N]` | elements with drop or handle glue; needs the region rules of llvm §4 per element |
| `Byte` bitwise | §29 operators on `Byte` with the same width rules |

## 8 Acceptance

Measured with the epic's method (codecs time and `perf stat` against `codecs.rs`), numbers in
each commit body:

| # | measurement | target |
|---|---|---|
| 1 | codecs, LLVM `--release`, median of 9 / plain Rust | at most 1.1 after F1 to F3 |
| 2 | codecs page faults / plain Rust | at most 1.5 after F1 |
| 3 | IR at `-O2` of the SHA-1 schedule and round loops over `[U32; 80]` | no call to `ignite_rt_fail_index` (an LLVM golden) |
| 4 | `U32` agreement: evaluator, Rust target, LLVM target against Rust's `u32` | §29.4's method, every operator, edges 0, 1, 2^31, 2^32 - 1, shifts and rotates by -1, 0, 31, 32, 33 |
| 5 | diagnostics | fixtures for E044 (`U32`), E040 (mixed widths, `[T; 3]` from a 4-literal), E058, E059 |
| 6 | `task gate` | green on both targets |

## 9 Draft stories

| id | title | features | expected effect (inferred) |
|---|---|---|---|
| `story:list-capacity` | `List.with_capacity` and `Bytes.with_capacity` | F1 | page faults to about the reference's; the kernel's 10 % of codecs |
| `story:u32-words` | `U32`: modular 32-bit words with rotate and explicit conversions | F2 | SHA-1 rounds to the reference's instruction count, minus the bounds checks |
| `story:fixed-size-lists` | `[T; N]` fixed-size lists of scalars with constant-length bounds checks | F3 | the remaining reloads and checks of `w[t]` and the alphabet lookups |

Order: F1 first (smallest, independent), F2, then F3 (its examples use `U32`). The codecs
fixture is rewritten on each as it lands, and row 1 of section 8 is measured after each.
`story:fixed-width-ints` stays the umbrella whose acceptance is row 1. Each story writes its
`docs/slice.md` section (the contract) from this document and its tests with it.

## 10 Open questions

| # | question | recommended default |
|---|---|---|
| Q1 | `+` on `U32` wraps, where `+` on `Int` fails: one operator, two meanings? | yes: the type says "word"; a checked word sum is `x.to_int() + y.to_int()` |
| Q2 | Fixed-size list syntax `[T; N]` (Rust) or `Array<T, N>` | `[T; N]`: no length generics needed, and readers know it |
| Q3 | Add `b.u32_be(at)` now? | no; only if row 1 misses 1.1x after F1 to F3 |
| Q4 | E058's bound | 65,536 bytes per fixed-size list (frames stay small; task stacks of `docs/readiness-io.md` are 2 MiB) |
| Q5 | Rename `Byte` to `U8`? | no; `Byte` stays the name, `U8` is not added |
