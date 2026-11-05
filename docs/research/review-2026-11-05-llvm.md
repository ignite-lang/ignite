# LLVM target review, 2026-11-05


All findings inferred from code; none run on a target (builds were paused for disk). Reproducers type-check (`ignite check` exit 0): `llvmreview/reproducers/{probe,probe2,probe3}.ignite`. A test failing on `llvm` and passing on `rust` confirms a finding.

| id | kind | file:line | finding | fix |
|---|---|---|---|---|
| M1 | miscompile | `crates/ignite-llvm/src/lower.rs:4361-4366`, `region.rs:268-283` | a stream a task passes on to a second task (`spawn b(stream)` on a parameter) is released when the first task returns; the second task's lookup fails "was released", the peer reads Eof. Rust keeps the `Arc` alive (`ignite-rustgen/src/lib.rs:3857-3871`). Reproducer `probe3.ignite` | owner count per handle-table entry: `spawn_with` registers every stream argument of the child, release at zero |
| L1 | leak | `lower.rs:4365,4388` (only release sites), `rt/net.rs:344-347` | a stream never moved into a task keeps its fd and table slot for the process lifetime; `close()` shuts down write only. Accept-and-handle-inline or connect in a loop grows the fd table | release at the last use of a fresh stream binding; `close()` on a last-use receiver releases |
| L2 | leak, small | `rt/fail.rs:147-270` | `longjmp` skips frames holding `String` temporaries per failed task | build the report in a block, move it into `fail` |
| P1 | perf | `lower.rs:2745-2751, 3637-3645`, `rt/text.rs:44-52` | every `Text +`/`concat` is a runtime call with a heap `Vec` and two copies; chains copy the prefix per step | one arena alloc of the summed length, one memcpy per operand |
| P2 | perf | `lower.rs:2090-2095, 2102-2110` | a constant is re-evaluated at every mention (RFC catalogues rebuilt); Rust target the same | literal constants as globals; once-initialised otherwise |
| P3 | perf | `lower.rs:4131-4132, 2256-2262, 665-672` | `list.map` deep-copies each element; fn-value calls copy every aggregate argument | borrow unmodified closure-literal parameters (`region::modified`) |
| P4 | perf | `lower.rs:2016-2022` | closure captures always copied, never moved at last use | memcpy when `types.moves` marks the last use |
| P5 | perf | `lower.rs:4296-4299, 4341-4345` | borrowed aggregate passed to `spawn` copied in parent and child | pass the pointer; parent waits for `spawn_ready` |
| P6 | perf | `rt/net.rs:287-315`, `rt/arena.rs:235-240, 397-403, 164-178` | a read > 16 KiB (HTTP `READ_SIZE` 65536) is an untrimmed 64 KiB `calloc` block per read and empties the bump state | per-task read buffer, copy `count` bytes |
| P7 | perf | `rt/text.rs:100-107, 35-42, 60-87`, `rt/stdio.rs:38-44` | join two heap allocs, trim full UTF-8 pass, stdout `to_vec` per write | sum and copy once; ASCII trim; `Stdout::write_from(&[u8])` |
| P8 | perf (known O11) | `lower.rs:1685-1724`, `glue.rs:369-372` | `Map.put` O(n) memmove; Text key probe a runtime call | inline length-aware memcmp compare |
| P9 | perf, trivial | `glue.rs:55` | `icmp ult %end, %li` sends an exact fit to the slow path | `ule` |

Notes, checked, no finding: enum-key compare by tag (`glue.rs:373-377`) is dead for payload enums because E106 rejects them (verified with `ignite check`) and would miscompile if E106 were relaxed; arena "zeroed" (`docs/llvm.md:323`) holds for fresh blocks only, nothing relies on it; loop regions, worker-thread thread-locals and the handle table: no race found.

To run the reproducers later, from a tree with `target/debug/ignite` and the runner runtime:
`slot.sh LOG TREE target/debug/ignite test probe.ignite probe2.ignite probe3.ignite --target=all --runtime target/ignite-test/rt/debug/libignite_rt.a`

## Reproducers

The three programs of the review, as written (`ignite check` exit 0 each, not run).

### `probe.ignite`

```ignite fragment
enum Color {
  Red,
  Green,
  Blue,
}

type Rec {
  name: Text
  tags: List<Text>
  n: Int
}

type Pair {
  a: Text
  b: Text
}

const GREETING: Text = "hello, world".concat("!")
const TAGS: List<Text> = ["a", "b", "c"]

fn filler(i: Int) -> Text = "................................................................".concat(Int.to_text(i))

fn takes(xs: List<Int>) -> Int {
  var ys = xs
  ys.push(99)
  return ys.len()
}

fn four(a: Text, b: Text, c: Text, d: Text) -> Text = a + b + c + d

fn shout(t: Text) -> Text = t.concat("!")

fn apply_all(xs: List<Text>, f: fn(Text) -> Text) -> List<Text> = xs.map(f)

fn count_greeting(n: Int) -> Int {
  var total = 0
  var i = 0
  while i < n {
    total = total + GREETING.len() + TAGS.len()
    i = i + 1
  }
  return total
}

fn make_adder(n: Int) -> fn(Int) -> Int = |x| x + n

test "text add does not alias the left operand" {
  let base = "ab".concat("cd")
  let s = base + "e"
  let t = base + "1"
  let u = base + "2"
  assert s == "abcde"
  assert t == "abcd1"
  assert u == "abcd2"
  assert base == "abcd"
  var acc = "x"
  var i = 0
  while i < 5 {
    let before = acc
    acc = acc + "y"
    assert before.len() == i + 1
    i = i + 1
  }
  assert acc == "xyyyyy"
}

test "text concat chain" {
  assert four("a", "b", "c", "d") == "abcd"
  let a = "p"
  let b = a.concat("q").concat("r")
  assert a == "p"
  assert b == "pqr"
}

test "list push keeps value semantics" {
  let a = [1, 2]
  var b = a
  b.push(3)
  var c = a
  c.push(4)
  assert a == [1, 2]
  assert b == [1, 2, 3]
  assert c == [1, 2, 4]
  assert takes(a) == 3
  assert takes(a) == 3
  assert a.len() == 2
}

test "list map copies and transforms" {
  let xs = ["a", "b"]
  let ys = apply_all(xs, shout)
  assert ys == ["a!", "b!"]
  assert xs == ["a", "b"]
  let zs = xs.map(|t| t.concat(t))
  assert zs == ["aa", "bb"]
}

test "record update does not alias" {
  let r = Rec { name = "n", tags = ["t"], n = 1 }
  var s = Rec { n = 2, ..r }
  s.tags.push("u")
  assert r.tags == ["t"]
  assert s.tags == ["t", "u"]
  assert r.n == 1
  assert s.name == "n"
}

test "closure captures by value" {
  var xs = [1]
  let f = || xs.len()
  xs.push(2)
  assert f() == 1
  assert xs.len() == 2
  let add = make_adder(5)
  assert add(1) == 6
}

test "bytes of" {
  let b = Bytes.of([65, 0, 255, 66])
  assert b.len() == 4
  assert b.at(0) == 65
  assert b.at(1) == 0
  assert b.at(2) == 255
  assert b.at(3) == 66
  assert b.to_text() == "A".concat(Bytes.of([0]).to_text()).concat(Bytes.of([255]).to_text()).concat("B")
}

test "int boundaries" {
  let max = 9223372036854775807
  let min = -9223372036854775807 - 1
  assert max - 1 + 1 == max
  assert min + 1 - 1 == min
  assert min / 1 == min
  assert min % 7 == -1
  assert -7 % 3 == -1
  assert -7 / 2 == -3
  assert Int.parse("9223372036854775807") == Some(max)
  assert Int.parse("-9223372036854775808") == Some(min)
  assert Int.parse("9223372036854775808") == None
  assert Int.parse("+5") == None
  assert Int.parse("-0") == Some(0)
  assert Int.to_text(min) == "-9223372036854775808"
}

test "byte arithmetic" {
  let a = 250
  let b = a + 5
  assert b == 255
  assert 3 - 3 == 0
  assert 16 * 15 == 240
  assert 255 / 2 == 127
  assert 255 % 16 == 15
}

test "text equality with nul and lengths" {
  let z = Bytes.of([0]).to_text()
  let zz = Bytes.of([0, 0]).to_text()
  assert z != zz
  assert z == Bytes.of([0]).to_text()
  assert "" == ""
  assert "" != z
  assert "abc" != "abd"
  assert "abc" != "ab"
}

test "map key order by kind" {
  var bools: Map<Bool, Int> = Map.empty()
  bools.put(true, 1)
  bools.put(false, 0)
  assert bools.keys() == [false, true]
  var bytes: Map<Byte, Int> = Map.empty()
  bytes.put(200, 1)
  bytes.put(3, 2)
  bytes.put(127, 3)
  assert bytes.keys() == [3, 127, 200]
  var colors: Map<Color, Int> = Map.empty()
  colors.put(Blue, 1)
  colors.put(Red, 2)
  colors.put(Green, 3)
  assert colors.keys() == [Red, Green, Blue]
  var ints: Map<Int, Int> = Map.empty()
  ints.put(5, 1)
  ints.put(-9223372036854775807 - 1, 2)
  ints.put(-1, 3)
  assert ints.keys() == [-9223372036854775807 - 1, -1, 5]
  var texts: Map<Text, Int> = Map.empty()
  texts.put("b", 1)
  texts.put("", 2)
  texts.put("a".concat(Bytes.of([255]).to_text()), 3)
  texts.put("aa", 4)
  assert texts.keys() == ["", "a".concat(Bytes.of([255]).to_text()), "aa", "b"]
  var again = texts
  again.put("b", 9)
  again.remove("")
  assert texts.get("b") == Some(1)
  assert texts.len() == 4
  assert again.len() == 3
  assert again.get("b") == Some(9)
}

test "text ops" {
  assert " \t a b \n".trim() == "a b"
  let nbsp = Bytes.of([194, 160]).to_text()
  assert nbsp.concat("x").concat(nbsp).trim() == "x"
  assert "abc".strip_prefix("ab") == Some("c")
  assert "abc".strip_prefix("abc") == Some("")
  assert "abc".strip_prefix("b") == None
  assert ["a", "b", "c"].join("-") == "a-b-c"
  assert ["a"].join("-") == "a"
  let empty: List<Text> = []
  assert empty.join("-") == ""
  assert Bytes.of([255, 65]).to_text() == Bytes.of([239, 191, 189]).to_text().concat("A")
  assert "héllo".len() == 6
}

test "constants are stable" {
  assert GREETING == "hello, world!"
  assert TAGS == ["a", "b", "c"]
  assert count_greeting(3) == 48
}

test "carried values survive many releases" {
  var recs: List<Rec> = []
  var names: Map<Text, Int> = Map.empty()
  var last = make_adder(0)
  var pair = Pair { a = "", b = "" }
  var i = 0
  while i < 30000 {
    let t = filler(i)
    if i % 1000 == 0 {
      recs.push(Rec { name = t, tags = [t, "k"], n = i })
      names.put(t, i)
      last = make_adder(i)
      pair = Pair { a = t, b = pair.a }
    }
    let junk = [t, t, t]
    let junk2 = junk.map(|x| x.concat(t))
    i = i + 1
  }
  assert recs.len() == 30
  assert recs[29].name == filler(29000)
  assert recs[0].tags == [filler(0), "k"]
  assert recs[15].tags[0] == filler(15000)
  assert names.len() == 30
  assert names.get(filler(29000)) == Some(29000)
  assert last(1) == 29001
  assert pair.a == filler(29000)
  assert pair.b == filler(28000)
}

test "nested loops with carried inner lists" {
  var rows: List<List<Text>> = []
  var i = 0
  while i < 200 {
    var row: List<Text> = []
    var j = 0
    while j < 300 {
      row.push(filler(j))
      let junk = filler(i + j)
      j = j + 1
    }
    rows.push(row)
    i = i + 1
  }
  assert rows.len() == 200
  assert rows[199][299] == filler(299)
  assert rows[0][0] == filler(0)
}

test "for over a list modified elsewhere" {
  var xs = ["a", "b", "c"]
  var out: List<Text> = []
  for x in xs {
    xs.push(x.concat("!"))
    out.push(x)
  }
  assert out == ["a", "b", "c"]
  assert xs.len() == 6
  var total = 0
  for (k, v) in [("a", 1), ("b", 2)] {
    total = total + v + k.len()
  }
  assert total == 5
}

test "match on rest patterns" {
  let xs = ["a", "b", "c", "d"]
  let r = match xs {
    [first, ..rest] => first.concat(rest.join(""))
    [] => ""
  }
  assert r == "abcd"
  match xs {
    [a, b, ..mid] => {
      var m = mid
      m.push("e")
      assert m == ["c", "d", "e"]
      assert a.concat(b) == "ab"
    }
    _ => { assert false }
  }
  assert xs == ["a", "b", "c", "d"]
}

test "slices and copies" {
  let xs = [1, 2, 3, 4, 5]
  let mid = xs.slice(1, 4)
  assert mid == [2, 3, 4]
  var m2 = mid
  m2.push(9)
  assert mid == [2, 3, 4]
  assert xs == [1, 2, 3, 4, 5]
  let t = "hello"
  let bs = t.to_bytes()
  assert bs.slice(1, 3).to_text() == "el"
  assert bs.slice(0, 0).to_text() == ""
  assert bs.slice(5, 5).to_text() == ""
}
```

### `probe2.ignite`

```ignite fragment
import std.net.{Pipe, PipeStream}
import std.io.IoError

type IoResult<T> = Result<T, IoError>

fn grow(xs: List<Int>) -> Int {
  xs.push(1)
  return xs.len()
}

fn reply(stream: PipeStream, tag: Text, items: List<Text>) -> IoResult<Unit> requires [stream.read, stream.write] {
  match stream.read(4096)? {
    Eof => Ok(())
    Data(bytes) => {
      stream.write_all(tag.concat(bytes.to_text()).concat(items.join(",")).to_bytes())?
      _ = stream.close()
      Ok(())
    }
  }
}

fn crash(stream: PipeStream, xs: List<Int>) -> IoResult<Unit> requires [stream.read, stream.write] {
  match stream.read(4096)? {
    Eof => Ok(())
    Data(bytes) => {
      let n = xs[bytes.len()]
      stream.write_all(Int.to_text(n).to_bytes())?
      Ok(())
    }
  }
}

test fn rest(client: PipeStream) -> IoResult<Bytes> {
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => return Ok(out)
      Data(bytes) => out.append(bytes)
    }
  }
}

test "a mutated parameter does not change the caller's value" {
  let a = [1, 2]
  assert grow(a) == 3
  assert grow(a) == 3
  assert a == [1, 2]
}

test "many tasks over pipes" {
  var i = 0
  var items = ["x"]
  while i < 200 {
    let (client, server) = Pipe.pair()
    client.set_idle_timeout(5000)?
    items.push(Int.to_text(i))
    spawn reply(server, "t".concat(Int.to_text(i)), items)
    client.write_all(b"hi")?
    let out = rest(client)?
    assert out.to_text() == "t{i}hi".concat(items.join(","))
    i = i + 1
  }
  assert items.len() == 201
}

test "a failing task closes its stream" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn crash(server, [1, 2])
  client.write_all(b"12345")?
  let out = rest(client)?
  assert out.len() == 0
  let (c2, s2) = Pipe.pair()
  c2.set_idle_timeout(5000)?
  spawn crash(s2, [1, 2, 3, 4, 5, 6])
  c2.write_all(b"12345")?
  assert rest(c2)?.to_text() == "6"
}

test "self append and push" {
  var xs = [1, 2, 3]
  xs.append(xs)
  assert xs == [1, 2, 3, 1, 2, 3]
  var ts = ["a", "b"]
  ts.push(ts[0])
  ts.append(ts)
  assert ts == ["a", "b", "a", "a", "b", "a"]
  var t = "ab"
  t = t + t
  assert t == "abab"
}
```

### `probe3.ignite`

```ignite fragment
import std.net.{Pipe, PipeStream}
import std.io.IoError

type IoResult<T> = Result<T, IoError>

fn echo_once(stream: PipeStream) -> IoResult<Unit> requires [stream.read, stream.write] {
  match stream.read(4096)? {
    Eof => Ok(())
    Data(bytes) => {
      stream.write_all(bytes)?
      _ = stream.close()
      Ok(())
    }
  }
}

fn hand_on(stream: PipeStream) -> IoResult<Unit> requires [stream.read, stream.write] {
  spawn echo_once(stream)
  Ok(())
}

// Waits about 50 ms: a pipe end with an idle timeout reads Eof.
test fn pause() -> IoResult<Unit> {
  let (a, b) = Pipe.pair()
  a.set_idle_timeout(50)?
  _ = a.read(1)?
  _ = b.close()
  Ok(())
}

test fn rest(client: PipeStream) -> IoResult<Bytes> {
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => return Ok(out)
      Data(bytes) => out.append(bytes)
    }
  }
}

test "a stream handed on to a second task outlives the first task" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn hand_on(server)
  pause()?
  client.write_all(b"hi")?
  assert rest(client)? == b"hi"
}
```
