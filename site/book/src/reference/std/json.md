# std.json

JSON values (RFC 8259). `encode` writes the compact form, `parse` reads one value surrounded
by optional whitespace, and the accessors read object members (`body.text("title")`). A
number without fraction or exponent of at most 18 digits is a `Number(Int)`; every other
number is a `Decimal` holding the token's exact text, never converted, so `encode` writes it
back unchanged. Strings are decoded completely, `\uXXXX` with surrogate pairs included; a lone
surrogate is an error.

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

const MAXIMUM_DIGITS: Int = 18
const MAXIMUM_DEPTH: Int = 64
```

- `Json` is one JSON value. An object is a list of members in their order; a name may repeat.
- `JsonError` says why `parse` failed and at which byte offset of its input.
- `Parsed` is a value and the offset just after it, the result of the internal parse steps.
- A `Number` has at most `MAXIMUM_DIGITS` digits; a longer integer is a `Decimal`, as is any
  number with a fraction or an exponent. Values compare by structure: `Number(1)`,
  `Decimal("1.0")` and `Decimal("1.00")` are three different values.
- Arrays and objects nest at most `MAXIMUM_DEPTH` levels.

## Encoding and parsing

```ignite,fragment
fn encode(value: Json) -> Text
fn parse(text: Text) -> Result<Json, JsonError>
fn quote(text: Text) -> Text
```

| function | what it does |
|---|---|
| `encode` | the compact JSON text of `value`: no whitespace, members in their order |
| `parse` | the one JSON value of `text`, with whitespace allowed around it |
| `quote` | `text` as a JSON string literal: quote, backslash and the control characters escaped, every other character as it is |

Parse errors carry messages such as `unexpected end of input`, `unexpected character`,
``expected `,` or `]` ``, `expected a digit`, `leading zero in a number`, `unterminated
string`, `control character in a string` and `nested deeper than 64`.

## Reading values

```ignite,fragment
fn get(value: Json, name: Text) -> Option<Json>
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
| `get` | the first member `name` of an object; `None` when there is none or `value` is not an object |
| `text`, `int`, `bool` | member `name` when it is a string, a number, or `true`/`false` |
| `as_text`, `as_int`, `as_bool`, `as_list` | the value itself when it is of that kind |

## Example

```ignite
import std.json.*

test "parse, read and encode JSON" {
  let body = parse(r#"{"title":"milk","done":true,"id":7,"tags":["a"],"id":8}"#)?
  assert body.text("title") == Some("milk")
  assert body.bool("done") == Some(true)
  assert body.int("id") == Some(7)
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
