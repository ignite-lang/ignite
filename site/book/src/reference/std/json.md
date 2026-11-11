# std.json

JSON values (RFC 8259). `encode` writes the compact form, `parse` reads one value surrounded
by optional whitespace, and the accessors read object members (`body.text("title")`). A
number without fraction or exponent that fits the 64-bit `Int` is a `Number(Int)`, except
`-0`; every other number is a `Decimal` holding the token's exact text, never converted, so
`encode` writes it back unchanged. `Decimal(text)` built by hand is not checked; `decimal`
is the checked constructor. Strings are decoded completely, `\uXXXX` with surrogate pairs
included; a lone surrogate is an error. `read` and `write` bridge JSON to the
[std.data](data.md) value model.

- Layer: base.
- Source: [`std/json.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/json.ignite).
- Import: `import std.json.{Json, parse, encode, text}`, or `import std.json.*`.
  Importing `Json` brings its variants (`Null`, `Str`, `Object`, ...) into scope.

## Types

```ignite,fragment
enum Json {
  Null,
  Bool(Bool),
  Number(Int),
  Decimal(Text),
  Str(Text),
  Array(List<Json>),
  Object(List<(Text, Json)>)
}

type JsonError {
  message: Text
  offset: Int
}

type Parsed = Result<(Json, Int), JsonError>

const MAXIMUM_DIGITS: Int = 19
const MAXIMUM_DEPTH: Int = 64
```

- `Json` is one JSON value. An object is a list of members in their order; a name may repeat.
- `JsonError` says why `parse` failed and at which byte offset of its input.
- `Parsed` is a value and the offset just after it, the result of the internal parse steps.
- A `Number` fits `Int`. An integer beyond it, or of more than `MAXIMUM_DIGITS` digits (the
  digit count of the longest `Int`), is a `Decimal`, and so are `-0` and any number with a
  fraction or an exponent. Values compare by structure: `Number(1)`,
  `Decimal("1.0")` and `Decimal("1.00")` are three different values.
- Arrays and objects nest at most `MAXIMUM_DEPTH` levels.

## Encoding and parsing

```ignite,fragment
fn encode(value: Json) -> Text
fn parse(text: Text) -> Result<Json, JsonError>
fn decimal(text: Text) -> Result<Json, JsonError>
fn quote(text: Text) -> Text
```

| function | what it does |
|---|---|
| `encode` | the compact JSON text of `value`: no whitespace, members in their order |
| `parse` | the one JSON value of `text`, with whitespace allowed around it |
| `decimal` | the checked constructor: the `Number` or `Decimal` `parse` gives for exactly one number token, an error for any other text; `Decimal(text)` built by hand is not checked |
| `quote` | `text` as a JSON string literal: quote, backslash and the control characters escaped, every other character as it is |

Parse errors carry messages such as `unexpected end of input`, `unexpected character`,
``expected `,` or `]` ``, `expected a digit`, `leading zero in a number`, `unterminated
string`, `control character in a string` and `nested deeper than 64`.

## Reading values

```ignite,fragment
fn get(value: Json, name: Text) -> Option<Json>
fn get_all(value: Json, name: Text) -> List<Json>
fn text(value: Json, name: Text) -> Option<Text>
fn int(value: Json, name: Text) -> Option<Int>
fn bool(value: Json, name: Text) -> Option<Bool>
fn as_text(value: Json) -> Option<Text>
fn as_int(value: Json) -> Option<Int>
fn as_bool(value: Json) -> Option<Bool>
fn as_list(value: Json) -> Option<List<Json>>
```

| function | what it does |
|---|---|
| `get` | the last member `name` of an object, as ECMAScript's `JSON.parse` keeps it; `None` when there is none or `value` is not an object |
| `get_all` | every member `name` of an object, in document order; empty when there is none or `value` is not an object |
| `text`, `int`, `bool` | member `name` when it is a string, a number, or `true`/`false` |
| `as_text`, `as_int`, `as_bool`, `as_list` | the value itself when it is of that kind |

A name may repeat in an object, and RFC 8259 section 4 leaves open which member counts. `parse`
keeps every member and `encode` writes every one back; `get`, and with it `text`, `int` and
`bool`, reads the last one, so a server reads `{"channel":"/a","channel":"/b"}` as `/b`, the
value a JavaScript client sees. `get_all` gives every member when the repetition matters.
`read` (the bridge to `std.data`, below) refuses a repeated name instead.

## The std.data bridge

These functions read JSON into a [std.data](data.md) `Value` and write one back, the form every
data format shares.

```ignite,fragment
fn to_data(json: Json) -> Value
fn of_data(value: Value) -> Json
fn read(text: Text) -> Result<Value, FormatError>
fn read_with(text: Text, limits: DataLimits) -> Result<Value, FormatError>
fn write(value: Value) -> Result<Text, FormatError>
fn write_pretty(value: Value) -> Result<Text, FormatError>
```

| function | what it does |
|---|---|
| `to_data` | `Number` as `Int`, `Str` as `Text`, `Array` as `List`, `Object` as `Map`; `Null`, `Bool` and `Decimal` as they are |
| `of_data` | the inverse; `Bytes` as `Str` of its base64, `Moment` as `Str` of its text. `of_data(to_data(j)) == j` for every `j` |
| `read` | `read_with(text, DataLimits {})` |
| `read_with` | the one value of `text` as `parse` reads it, then `to_data`, within `limits` |
| `write` | `encode(of_data(value))` |
| `write_pretty` | as `write`, one item or member per line, two spaces per level, `"name": value`; empty arrays and objects stay `[]` and `{}` |

`read_with` refuses, each a `FormatError` at the byte named:

| refused | message | at |
|---|---|---|
| more than `limits.size` bytes | `document larger than <n> bytes` | offset `n` |
| nesting deeper than `limits.depth` | `nested deeper than <n>` | the bracket |
| more than `limits.entries` items or members in one array or object | `more than <n> entries` | the first one too many |
| a member name twice in one object | ``duplicate member `<name>` `` | its opening quote |
| what `parse` refuses | the message of `parse` | the bad byte |

`write` and `write_pretty` refuse a `Decimal` whose text is no JSON number, with
``decimal `<text>` is not a JSON number``. `parse` and `encode` are unchanged by the bridge:
`parse` keeps a repeated member name and bounds only the depth. JSON has no bytes and no
moments, so a written `Bytes` or `Moment` reads back as `Text`.

```ignite
import std.json.{read, write, write_pretty}
import std.data.Value

test "read and write a data value" {
  let value = read(r#"{"id": 1, "ratio": 0.5, "tags": []}"#)?
  assert value == Value.Map([("id", Value.Int(1)), ("ratio", Value.Decimal("0.5")), ("tags", Value.List([]))])
  assert write(value)? == r#"{"id":1,"ratio":0.5,"tags":[]}"#
  assert write(Value.Bytes(b"fo"))? == r#""Zm8=""#
  assert write_pretty(Value.List([Value.Int(1)]))? == "[\n  1\n]"
  let duplicate = match read(r#"{"a": 1, "a": 2}"#) {
    Ok(_) => ""
    Err(error) => error.message
  }
  assert duplicate == "duplicate member `a`"
}
```

## Example

```ignite
import std.json.*

test "parse, read and encode JSON" {
  let body = parse(r#"{"title":"milk","done":true,"id":7,"tags":["a"],"id":8}"#)?
  assert body.text("title") == Some("milk")
  assert body.bool("done") == Some(true)
  assert body.int("id") == Some(8)
  assert body.get_all("id") == [Number(7), Number(8)]
  assert body.text("id") == None
  assert body.get("tags") == Some(Array([Str("a")]))
  let todo = Object([("id", Number(1)), ("title", Str(r#"say "hi""#)), ("done", Bool(false))])
  assert encode(todo) == r#"{"id":1,"title":"say \"hi\"","done":false}"#
  assert parse(encode(todo))? == todo
}

test "what parse refuses" {
  let error = match parse("1.") {
    Ok(_) => JsonError { message = "", offset = 0 }
    Err(e) => e
  }
  assert error == JsonError { message = "expected a digit", offset = 2 }
  assert parse("1.5")? == Decimal("1.5")
  assert encode(parse("-1.25E-3")?) == "-1.25E-3"
}
```

The [todo example](https://github.com/ignite-lang/ignite/blob/main/examples/todo/todo.ignite)
reads request bodies with `parse` and `text`, and answers with `Response.json` of
[std.protocol.http](http.md), which calls `encode`.
