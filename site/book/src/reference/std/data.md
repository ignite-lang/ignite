# std.data

The value model every data format reads into and writes from. A format module turns text into
a `Value` and back. A codec turns a typed value into a `Value` and back. So N formats and M
types need N plus M functions. JSON is the one format today: [std.json](json.md#the-stddata-bridge)
reads and writes a `Value`.

- Layer: base. It imports nothing.
- Source: [`std/data.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/data.ignite).
- Import: `import std.data.{Value, Shape, DataLimits, DecodeError, FormatError}`.

Codecs, YAML and TOML are not implemented. `Shape` and `DecodeError` are the vocabulary they
will use; nothing reads `Shape` today. The [roadmap](../../roadmap.md) has their status.

## Value

```ignite,fragment
enum Value {
  Null,
  Bool(Bool),
  Int(Int),
  Decimal(Text),
  Text(Text),
  Bytes(Bytes),
  Moment(Text),
  List(List<Value>),
  Map(List<(Text, Value)>)
}
```

| variant | holds |
|---|---|
| `Decimal(text)` | any number `Int` cannot hold, as its exact text |
| `Moment(text)` | a date, a time or a date-time, as RFC 3339 text |
| `Map(members)` | members in document order; a format refuses a duplicate key |

`Value`'s variants share names with `Json`'s (`Null`, `Bool`, `Decimal`). A module that
imports both writes `Value.Null` or `Json.Null` where the expected type does not decide (E005).

## Shape and limits

```ignite,fragment
enum Case { Same, Camel, Pascal, Kebab, ScreamingSnake }

enum Tagging {
  External,
  Internal(Text)
}

enum Unknown { Ignore, Deny }

type DataLimits {
  depth: Int = 64
  size: ByteSize = 16 MiB
  entries: Int = 1000000
}

type Shape {
  keys: Case = Same
  variants: Case = Same
  tagging: Tagging = External
  unknown: Unknown = Ignore
  renames: List<(Text, Text)> = []
  limits: DataLimits = DataLimits {}
}

fn Shape.default() -> Shape
```

| item | meaning |
|---|---|
| `Case` | the spelling of field or variant names on the wire; `Same` keeps the declared name |
| `Tagging` | where an enum's variant tag goes: `External`, a map with one member named by the tag; `Internal(member)`, a member of the payload map |
| `Unknown` | what decoding does with a map member that no field names |
| `DataLimits` | the bounds of one document: nesting depth of lists and maps, size in bytes, entries of one list or map |
| `Shape` | the wire names and rules a codec applies; `renames` holds exceptions such as `("Type.field", "wireName")` and `("Type.Variant", "tag")` |
| `Shape.default()` | `Shape {}`: declared names, external tags, unknown members ignored |

## Errors

```ignite,fragment
type DecodeError {
  path: Text
  expected: Text
  found: Text
}

type FormatError {
  message: Text
  line: Int
  column: Int
  offset: Int
}

fn describe(error: DecodeError) -> Text
fn format_error(text: Text, message: Text, offset: Int) -> FormatError
```

| item | meaning |
|---|---|
| `DecodeError` | the first value a codec could not decode: where (`$.items[2].title`), what it expected and what it found |
| `describe` | `<path>: expected <expected>, found <found>`; by UFCS `error.describe()` |
| `FormatError` | why a format could not read or write a document: for a read the first bad byte, `line` and `column` 1-based, `column` in characters, `offset` in bytes; a write gives 0 for all three |
| `format_error` | the `FormatError` at byte `offset` of `text`, its line and column counted |

The positions are those `ignite check --json` gives.

## Example

```ignite
import std.data.*

test "the default shape keeps names and tags externally" {
  let shape = Shape.default()
  assert shape.keys == Same
  assert shape.tagging == External
  assert shape.limits == DataLimits {}
}

test "errors name their place" {
  let error = DecodeError { path = "$.a[1].b", expected = "Int", found = "Text" }
  assert error.describe() == "$.a[1].b: expected Int, found Text"
  assert format_error("a\nbc", "m", 3) == FormatError { message = "m", line = 2, column = 2, offset = 3 }
}
```
