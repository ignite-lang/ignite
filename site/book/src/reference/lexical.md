# Lexical structure

This page describes how Ignite source text is split into tokens: comments, names,
literals, keywords and punctuation. The [grammar](grammar.md) builds on these tokens.

## Source text

- A source file is UTF-8 text with the extension `.ignite`.
- Spaces, tabs and carriage returns separate tokens and carry no meaning.
- A line break is a token. It ends a statement, a field or a declaration. A run of several
  line breaks counts as one. Where line breaks are skipped and where they matter is
  explained in [Grammar: line breaks and separators](grammar.md#line-breaks-and-separators).
- Non-ASCII characters may appear in string literals and comments only.

## Comments

A comment starts with `//` and runs to the end of the line. There are no block comments.

```ignite
// The number of retries before giving up.
const RETRIES: Int = 3 // a trailing comment

fn retries() -> Int = RETRIES
```

`/* ... */` is not a comment; the `/` is read as division and the file fails with E001.

## Identifiers

An identifier is an ASCII letter or `_`, followed by ASCII letters, digits and `_`:
`[A-Za-z_][A-Za-z0-9_]*`.

| kind of name | convention | examples |
|---|---|---|
| types, enums, variants, protocols, systems | `UpperCamel` | `Reply`, `Action.Close`, `Telnet` |
| fns, parameters, fields, locals | `lower_snake` | `first_word`, `maximum_line` |
| constants | `UPPER_SNAKE` | `MAXIMUM_LINE` |

The parser does not enforce case, with two exceptions that make case meaningful:

- `Name { ... }` is a record literal only when `Name` starts with an uppercase letter.
- In a pattern, a name without arguments that names a constant in scope is a constant
  pattern, whatever its case. Otherwise a single lowercase name without arguments binds a
  new variable, and an uppercase name, or a name with arguments, is a variant.

`_` alone is the wildcard: in patterns, and in `_ = e` to discard a value.

## Keywords

Ignite has 32 keywords. They cannot be used as identifiers for variables, parameters, fns or
types.

| group | keywords |
|---|---|
| modules | `package` `import` |
| declarations | `type` `enum` `fn` `const` `protocol` `test` |
| systems | `system` `resource` `component` `contract` `policy` `assembly` |
| bindings | `let` `var` |
| control flow | `if` `else` `match` `while` `loop` `for` `in` `break` `continue` `return` |
| effects | `requires` `spawn` |
| tests | `assert` (`test` is listed under declarations) |
| literals | `true` `false` |
| retired | `await` |

`await` is still a keyword so that old code gets a clear message: writing it is E001
"remove `await`: effectful calls need no marker". See [Effects](effects.md).

A keyword may still appear where only a name can stand:

- as a segment of a `package` or `import` path (`std.protocol.telnet`);
- as a record field name, in a declaration or a literal (`protocol = TelnetProfile { .. }`);
- after a `.`, as a field or method name;
- as a segment of an effect in `requires` (`app.*`).

```ignite
type Probe {
  protocol: Text
  test: Bool
}

fn active(probe: Probe) -> Bool = probe.test && probe.protocol == "telnet"

test "keywords work as field names" {
  assert active(Probe { protocol = "telnet", test = true })
}
```

A known gap of v0.3.0: `ignite check` also accepts a field named after a Rust keyword, such
as `type` or `match`, and the LLVM target builds it, but the Rust target's generated code
does not compile. Avoid such field names until the Rust target escapes them.

```ignite,fails=E001
fn twice(test: Int) -> Int = test * 2
```

## Integer literals

- Decimal digits, with optional `_` separators: `0`, `42`, `1_000_000`.
- Hexadecimal after `0x`: `0xFF`, `0x7f_ff`.
- There are no suffixes and no octal or binary forms.
- An integer literal has type `Int`, a signed 64-bit integer. Where a `Byte` is expected,
  the same literal is a `Byte` (0 to 255; anything else is E044).
- The largest literal is `9223372036854775807`. The literal `9223372036854775808` is
  allowed only directly after a prefix `-`, so that `-9223372036854775808`, the smallest
  `Int`, can be written. Anything larger is E001.

```ignite
const MILLION: Int = 1_000_000
const MASK: Int = 0xFF
const SMALLEST: Int = -9223372036854775808

test "integer literals" {
  assert MILLION == 1000000
  assert MASK == 255
  assert SMALLEST < 0
}
```

There are no floating-point numbers. A decimal literal such as `99.9` exists only directly
before a unit or `%` (see [quantity literals](#quantity-and-percentage-literals)). Written
alone, `1.5` is read as the integer `1` followed by `.5`, a tuple field access, and fails.

## String literals

A string literal is text between double quotes, of type `Text`. It may not span lines: a
line break before the closing `"` is E001 "unterminated string". For text over several lines,
use a [raw string](#raw-strings) or `\n`.

Escapes in a string literal:

| escape | meaning |
|---|---|
| `\n` | line feed |
| `\r` | carriage return |
| `\t` | tab |
| `\v` | vertical tab (`0B`) |
| `\f` | form feed (`0C`) |
| `\0` | NUL |
| `\\` | backslash |
| `\"` | double quote |
| `\xNN` | the ASCII character `NN`, two hex digits from `00` to `7F` |
| `\u{N}` | the Unicode scalar value `N`, 1 to 6 hex digits |
| `{{` and `}}` | a literal `{` and `}` |

Any other escape is E001, and so is a `\x` above `7F`: text is UTF-8, not raw bytes. Write
non-ASCII characters directly or with `\u{..}`.

```ignite
test "escapes" {
  assert "tab\there".len() == 8
  assert "\x41\u{42}" == "AB"
  assert "caf\u{E9}" == "café"
  assert "quote: \"" == "quote: " + "\""
}
```

## Interpolation

Inside a string literal, `{e}` inserts the value of the expression `e`.

- The expression is parsed like any other. It may contain calls, operators and other string
  literals, including interpolated ones.
- It may not contain a line break.
- The value must have a `to_text` method: `Text`, `Int`, `Byte`, `Bool` and `Bytes` have a
  builtin one, and a module-level fn `to_text(value: T) -> Text` in scope supplies one for
  your own type `T` by [method call syntax](modules.md#method-calls-and-ufcs). Anything
  else is E041.
- `{}` is E001, and so is a lone `}` or a `{` without its `}` on the same line. Write `{{`
  and `}}` for literal braces.
- String literals nest at most 32 interpolations deep (E054).

`"a{x}b"` means `"a" + x.to_text() + "b"`.

```ignite
type Point { x: Int, y: Int }

fn to_text(p: Point) -> Text = "({p.x}, {p.y})"

fn describe(name: Text, count: Int) -> Text = "{name} has {count} item{plural(count)}"

fn plural(n: Int) -> Text = if n == 1 { "" } else { "s" }

test "interpolation" {
  assert describe("cart", 1) == "cart has 1 item"
  assert describe("cart", 3) == "cart has 3 items"
  assert "at {Point { x = 1, y = 2 }}" == "at (1, 2)"
  assert "{{literal}}".len() == 9
}
```

## Raw strings

A raw string has no escapes and no interpolation, and keeps line breaks.

- `r"..."` runs to the next `"`.
- `r#"..."#` runs to the next `"#`; use as many `#` as needed, so the text can hold `"`.
- `br"..."` is a raw [byte string](#byte-literals-and-byte-strings); it holds ASCII only.

```ignite
const PATTERN: Text = r"C:\temp\{name}"
const QUOTED: Text = r#"say "hi""#
const BANNER: Text = r"line one
line two"

test "raw strings" {
  assert PATTERN == "C:\\temp\\{{name}}"
  assert QUOTED == "say \"hi\""
  assert BANNER == "line one\nline two"
}
```

## Byte literals and byte strings

- `b'c'` is the byte value of one ASCII character or escape. It is an integer literal: a
  `Byte` where a `Byte` is expected, an `Int` otherwise. `b'a'` is `97`.
- `b"..."` is a byte string of type `Bytes`, the same as `Bytes.of([..])` with each byte.
  It holds ASCII characters and escapes only; write other bytes as `\xNN`.
- Escapes in both: `\n \r \t \v \f \0 \\ \' \"` and `\xNN` with `NN` from `00` to `FF`.
  `\'` is an escape here and not in text strings. `\u{..}` is not allowed.

```ignite
fn is_digit(b: Byte) -> Bool = match b {
  b'0'..=b'9' => true
  _ => false
}

test "byte literals" {
  assert b'a' == 97
  assert is_digit(b'7') && !is_digit(b'x')
  assert b"hi\r\n" == Bytes.of([104, 105, 13, 10])
  assert b"\xFF".len() == 1
}
```

## Quantity and percentage literals

A quantity literal is a number followed by a unit name on the same line. It has a quantity
type, not `Int`. The unit is an ordinary identifier, not a keyword, so `ms` stays usable as
a variable name.

| type | units | stored as |
|---|---|---|
| `Duration` | `ns`, `us`, `ms`, `second`, `seconds`, `minute`, `minutes`, `hour`, `hours`, `day`, `days` | whole nanoseconds |
| `ByteSize` | `byte`, `bytes`, `KiB`, `MiB`, `GiB` (1024-based), `KB`, `MB`, `GB` (1000-based) | whole bytes |
| `Ratio` | `%` | parts per million: `1%` is 10,000 |

- The number is an integer literal or a decimal literal with at most 4 decimals:
  `200 ms`, `1.5 seconds`, `16 KiB`, `99.9%`.
- `%` is the percent unit when it touches the number and no operand follows it. `10 % 3`,
  `10%3` and `n%2` are remainders; `50%` and `99.9%` are ratios.
- The pair binds tighter than any operator: `2 seconds + 500 ms` adds two durations.
- A unit applies to a literal only. `n ms` with a variable `n` is E001; write `1 ms * n`.
- A literal that is not a whole number of base units (`1.5 ns`), does not fit in an `Int`,
  or has more than 4 decimals is E107.

```ignite
const TIMEOUT: Duration = 1.5 seconds
const BUFFER: ByteSize = 16 KiB
const TARGET: Ratio = 99.9%

fn backoff(attempt: Int) -> Duration = 100 ms * attempt

test "quantity literals" {
  assert TIMEOUT.to_ms() == 1500
  assert BUFFER.to_int() == 16384
  assert TARGET.ppm() == 999000
  assert backoff(3) == 300 ms
  assert 2 seconds + 500 ms == 2500 ms
}
```

The operations on quantity types are in [Types: quantities](types.md#quantities).

## Punctuation

| token | used for |
|---|---|
| `( )` | calls, grouping, tuples, the unit value `()` |
| `{ }` | blocks, record and brace literals, declaration bodies, interpolation |
| `[ ]` | list literals, indexing, list patterns, `requires` lists |
| `< >` | type arguments and type parameters; also comparison |
| `,` | separates arguments, elements, fields and arms |
| `;` | separates statements on one line |
| `.` | field access, method calls, paths, tuple fields (`t.0`) |
| `..` | the base of a record update `R { f = e, ..base }` |
| `..=` | inclusive range patterns `b'a'..=b'z'` |
| `:` | type annotations, bounds `<S: ByteStream>` |
| `=` | bindings, assignment, field values, named arguments, expression bodies |
| `=>` | match arms |
| `->` | return types |
| `?` | error propagation |
| `\|` | or-patterns, closure parameters `\|x\| x + 1`; bitwise or |
| `\|\|` | logical or; a closure without parameters `\|\| 0` |
| `+ - * / %` | arithmetic; `+` also joins `Text` |
| `== != < > <= >=` | comparison |
| `&& \|\| !` | logic |
| `& ^ ~` | bitwise and, exclusive or, not on `Int`; `<<` and `>>` are two touching `<` or `>` |

The precedence of the operators is in the [grammar](grammar.md#operator-precedence).

A lexer error ends the token stream at that point. The parser still reports the syntax errors
before it, so one run can show several errors. All lexical errors use code E001; see
[Diagnostics](diagnostics.md).
