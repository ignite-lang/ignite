# std.digest

SHA-1 (RFC 3174) and SHA-256 (FIPS 180-4) over `Bytes`, written in Ignite on the 32-bit words
`U32`. `sha1` and `sha256` digest one value. A `Digest` digests input that arrives in pieces.

- Layer: base. It imports [std.encoding](encoding.md) for its tests only.
- Source: [`std/digest.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/digest.ignite).
- Import: `import std.digest.{sha1, sha256}`; for the incremental form
  `import std.digest.{Digest, update, finish}`.

## Functions

```ignite,fragment
fn sha1(data: Bytes) -> Bytes
fn sha256(data: Bytes) -> Bytes
```

| function | what it does |
|---|---|
| `sha1` | the 20-byte SHA-1 digest of `data` |
| `sha256` | the 32-byte SHA-256 digest of `data` |

Print a digest with `hex` of [std.encoding](encoding.md).

## Incremental digests

```ignite,fragment
enum Algorithm { Sha1, Sha256 }

type Digest {
  algorithm: Algorithm
  state: List<U32>
  pending: Bytes
  length: Int
}

fn Digest.sha1() -> Digest
fn Digest.sha256() -> Digest
fn update(self: Digest, data: Bytes) -> Digest
fn finish(self: Digest) -> Bytes
```

| item | meaning |
|---|---|
| `Digest` | the words of the state after every whole block, the bytes of the block not yet whole (fewer than 64), the bytes taken so far |
| `Digest.sha1()`, `Digest.sha256()` | the state before any input |
| `update` | the state after `data`; by UFCS `d.update(data)` |
| `finish` | the digest of everything taken |

`Digest.sha256().update(a).update(b).finish()` is `sha256` of `a` and `b` joined. `update`
compresses every whole block of `data` where it lies and copies at most 63 bytes. `update` and
`finish` are free functions, called by UFCS, so a caller imports them with `Digest`.

## Example

```ignite
import std.digest.{Digest, sha1, sha256, update, finish}
import std.encoding.hex

test "the digests of abc" {
  assert hex(sha1(b"abc")) == "a9993e364706816aba3e25717850c26c9cd0d89d"
  assert hex(sha256(b"abc")) == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
  assert sha256(b"abc").len() == 32
}

test "a digest of pieces is the digest of the whole" {
  let pieces = Digest.sha256().update(b"ab").update(b"c").finish()
  assert pieces == sha256(b"abc")
}
```
