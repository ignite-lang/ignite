# Statements and expressions

This page says what each statement and each control-flow expression does. Their syntax is in
the [Grammar](grammar.md). Types and operators are in [Types](types.md), `?` and `spawn` in
[Effects](effects.md), `assert` in [Tests and assertions](tests.md).

## Blocks

A block is `{ .. }` with statements separated by line breaks or `;`. The last expression of a
block is its value, and the value of a fn body. A block whose last statement is no expression
has the value `Unit`. A fn written `fn f(..) -> T = e` has the single expression `e` as its
body.

A block is not an expression on its own. It appears as a fn body, a `test` body, a branch of
`if`, an arm of `match`, a loop body or a closure body.

## Bindings

| statement | meaning |
|---|---|
| `let x = e` | an immutable binding |
| `var x = e` | a mutable binding |
| `let x: T = e` | with a type. Without one, the type comes from `e` |
| `let (a, b) = e` | a tuple pattern. `var (a, b) = e` makes both mutable |
| `let _ = e`, `_ = e` | evaluate `e` and drop its value |

A binding lives to the end of its block. A second `let x` in the same block shadows the first
from that point on. The pattern of `let` and `var` must be irrefutable: a name, `_` or a tuple
of those. Anything else is E001: use `match`.

A call whose value is not used is allowed as a statement, a `Result` included.

## Assignment

`place = e` assigns to a `var` or to a field chain rooted in one: `total = total + 1`,
`p.x = 5`, `p.origin.x = 0`. `l[i] = x` replaces a list element, as `l.set(i, x)` does.
Assigning to a parameter, a `let` binding, a `for` element or a pattern binding of `let` or
`match` is E006. The in-place list methods `push`, `set`, `append` and `clear` also work on a
parameter ([Lists](types.md#lists)). Assignment is a statement, not an expression.

## Copies and handles

Assigning, passing and returning a value copies it, as far as the program can tell. After
`let q = p`, a change to `p` does not change `q`. This holds for records, enums, tuples,
lists, maps, `Text` and `Bytes`.

Handles are shared: a copy of a `TcpStream`, a `Store`, a `Directory` or another capability
refers to the same socket, storage or directory.

The compiler moves a value instead of copying it when the place it comes from is not read
again. A move changes no value a program can observe, so it is an optimisation only. The
compile-time evaluator always copies and stays the reference for both targets.

## `if`

`if cond { .. } else { .. }` is an expression. The condition is a `Bool`. `else if` chains.
Used as a value, both branches have one type. Without `else`, an `if` has the type `Unit`, so
`let v = if n > 0 { 1 }` is E040. `else` must follow the `}` of its `if` on the same line.

## `match`

`match e { pattern => value, .. }` tries the arms in order and evaluates the first that
matches. Arms are separated by line breaks or `,`. An arm's value is an expression or a block.
Every arm has one type. The arms must cover every value of the scrutinee, or the match is
E046. There are no guards: test inside the arm.

| pattern | matches |
|---|---|
| `_` | anything, binds nothing |
| `name` | anything, binds it to `name`, immutable |
| `0`, `-1`, `b'a'`, `"text"`, `true` | that literal |
| `1..=3`, `b'a'..=b'z'` | an `Int` or `Byte` in the inclusive range |
| `LIMIT` | the value of a constant |
| `Some(p)`, `Circle(r)`, `Shape.Square(side)`, `Empty` | a variant, with patterns for its payload. The enum name is optional where the type is known |
| `(p, q)` | a tuple, element by element |
| `[]`, `[p]`, `[p, q]` | a list or `Bytes` of exactly that length |
| `[p, ..]`, `[.., q]`, `[p, ..rest]` | a list or `Bytes` of at least that many elements |
| `p \| q` | either pattern. A binding inside an or-pattern is E049 |

### List patterns and rest

`..` in a list pattern stands for any number of elements, zero included. `..rest` binds them:
a `List<T>` on a `List<T>`, `Bytes` on `Bytes`. A list pattern holds at most one `..`, and
`..name` must be its last element (E076). For E046, a match over a list is checked length by
length, so `[]`, `[_]` and `[_, _, ..]` together cover every list.

## Loops

| statement | meaning |
|---|---|
| `while cond { .. }` | run the body while `cond` is `true` |
| `loop { .. }` | run the body until `break` or `return` |
| `for x in list { .. }` | visit the elements of a `List` in order. `x` is immutable |
| `for byte in bytes { .. }` | visit the bytes of a `Bytes` |
| `for (k, v) in map { .. }` | visit the entries of a `Map` |

`break` leaves the innermost loop and `continue` starts its next round. Neither takes a value.
Loops are statements: they have no value, and `let v = loop { .. }` is E001. `for` over
anything else, a `Text` among them, is E040: iterate over `text.to_bytes()` or a list of parts.

## `return`

`return e` ends the fn with the value `e`. `return` alone returns `Unit`. In a test it ends the
test. `return` has no value when a line break, `;`, `,`, `}`, `)` or `]` follows it.

## Example

```ignite
type Point {
  x: Int
  y: Int
}

enum Shape {
  Circle(Int),
  Square(Int),
  Empty
}

const LIMIT: Int = 3

fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Shape.Square(side) => side * side
  Empty => 0
}

fn grade(n: Int) -> Text = match n {
  0 => "none"
  1..=3 => "few"
  4 | 5 => "some"
  _ => "many"
}

fn is_limit(n: Int) -> Bool = match n {
  LIMIT => true
  _ => false
}

fn describe(xs: List<Int>) -> Text = match xs {
  [] => "empty"
  [one] => "one {one}"
  [first, .., last] => "from {first} to {last}"
}

fn tail(xs: List<Int>) -> List<Int> = match xs {
  [_, ..rest] => rest
  [] => []
}

fn sign(n: Int) -> Int {
  if n < 0 {
    return -1
  }
  if n == 0 { 0 } else { 1 }
}

fn first_even(xs: List<Int>) -> Option<Int> {
  for x in xs {
    if x % 2 == 0 {
      return Some(x)
    }
  }
  None
}

fn count_odd_to(n: Int) -> Int {
  var i = 0
  var total = 0
  loop {
    i = i + 1
    if i > n {
      break
    }
    if i % 2 == 0 {
      continue
    }
    total = total + i
  }
  total
}

test "statements" {
  let x = 1
  let x = x + 1
  assert x == 2
  var p = Point { x = 1, y = 2 }
  p.x = 5
  let q = p
  p.y = 9
  assert p.x == 5 && q.y == 2
  let (a, b) = (1, "two")
  assert a == 1 && b == "two"
  _ = sign(4)
  assert area(Circle(1)) == 3 && area(Shape.Square(2)) == 4 && area(Empty) == 0
  assert grade(2) == "few" && grade(5) == "some" && grade(9) == "many"
  assert is_limit(3)
  assert describe([1, 2, 3]) == "from 1 to 3" && describe([7]) == "one 7"
  assert tail([1, 2, 3]) == [2, 3]
  assert first_even([1, 3, 4]) == Some(4)
  assert count_odd_to(5) == 9
  var seen: List<Text> = []
  let m: Map<Text, Int> = { a = 1 }
  for (k, v) in m {
    seen.push("{k}={v}")
  }
  assert seen == ["a=1"]
}
```
