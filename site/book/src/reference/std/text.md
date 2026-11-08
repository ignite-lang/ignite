# std.text

Text operations written in Ignite. You import the ones you need and call them as methods:
`line.split(" ")`. Offsets are byte offsets into the UTF-8 encoding, and needles match byte
for byte. Case and whitespace rules are ASCII only.

- Layer: base.
- Source: [`std/text.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/text.ignite).
- Import: `import std.text.{split, trim_end}`, or `import std.text.*` for every name.

The builtin text methods `len`, `is_empty`, `trim`, `starts_with`, `strip_prefix`,
`concat` and `pieces` (which `split` is) need no import and are not repeated here.

## Bytes

```ignite,fragment
fn is_whitespace(b: Byte) -> Bool
fn is_digit(b: Byte) -> Bool
fn digit_value(b: Byte) -> Option<Int>
```

| function | what it does |
|---|---|
| `is_whitespace` | ASCII whitespace: space, tab, LF, VT, FF, CR |
| `is_digit` | `b'0'` to `b'9'` |
| `digit_value` | the value of a decimal digit byte, `None` for any other byte |

## Searching

```ignite,fragment
fn find(text: Text, needle: Text) -> Option<Int>
fn find_from(text: Text, needle: Text, start: Int) -> Option<Int>
fn contains(text: Text, needle: Text) -> Bool
fn ends_with(text: Text, suffix: Text) -> Bool
fn bytes_find_from(bytes: Bytes, needle: Bytes, start: Int) -> Option<Int>
fn bytes_match_at(bytes: Bytes, needle: Bytes, at: Int) -> Bool
```

| function | what it does |
|---|---|
| `find` | the byte offset of the first `needle`; `"a=b".find("=")` is `Some(1)` |
| `find_from` | the byte offset of the first `needle` at or after `start` |
| `contains` | whether `needle` occurs in `text` |
| `ends_with` | whether `text` ends with `suffix` |
| `bytes_find_from` | `find_from` on `Bytes`, the builtin `bytes.index_of(needle, start)` |
| `bytes_match_at` | whether `needle` occurs in `bytes` at offset `at` |

An empty needle is found at offset 0 by `find` and at `start` by `find_from`.

## Cutting

```ignite,fragment
fn slice(text: Text, start: Int, end: Int) -> Text
fn split(text: Text, separator: Text) -> List<Text>
fn split_once(text: Text, separator: Text) -> Option<(Text, Text)>
fn strip_suffix(text: Text, suffix: Text) -> Option<Text>
fn lines(text: Text) -> List<Text>
```

| function | what it does |
|---|---|
| `slice` | bytes `start` to `end` (exclusive); an offset inside a UTF-8 character gives U+FFFD for the broken character |
| `split` | the pieces between the separators, empty pieces kept; `"".split(",")` is `[""]`; an empty separator gives `[text]` |
| `split_once` | the text before and after the first `separator`; `None` when it does not occur |
| `strip_suffix` | `text` without `suffix`, `None` when it does not end with it |
| `lines` | the lines of `text`, split at LF with a CR before it dropped; a final line break does not start another line |

## Changing

```ignite,fragment
fn replace(text: Text, from: Text, to: Text) -> Text
fn trim_start(text: Text) -> Text
fn trim_end(text: Text) -> Text
fn to_lower(text: Text) -> Text
fn to_upper(text: Text) -> Text
fn equals_ignore_case(a: Text, b: Text) -> Bool
fn repeat(text: Text, count: Int) -> Text
```

| function | what it does |
|---|---|
| `replace` | every `from` replaced by `to`; an empty `from` changes nothing |
| `trim_start`, `trim_end` | drop ASCII whitespace at one end |
| `to_lower`, `to_upper` | ASCII letters in lower or upper case; every other byte unchanged |
| `equals_ignore_case` | equal after ASCII case folding (`Content-Length` and `content-length`) |
| `repeat` | `count` copies of `text`; a count of 0 or less gives `""` |

## Example

The assertions are taken from the module's own tests.

```ignite
import std.text.*

test "searching and cutting text" {
  assert find("a=b=c", "=") == Some(1)
  assert "a=b=c".find_from("=", 2) == Some(3)
  assert "hello world".contains("o w")
  assert split("a,b,,c", ",") == ["a", "b", "", "c"]
  assert "Host: example: 1".split_once(": ") == Some(("Host", "example: 1"))
  assert strip_suffix("report.json", ".json") == Some("report")
  assert lines("a\r\nb\n") == ["a", "b"]
  assert slice("aéb", 1, 3) == "é"
}

test "changing text" {
  assert "a.b.c".replace(".", "::") == "a::b::c"
  assert " \t x y  \n".trim_end() == " \t x y"
  assert "MiXeD 42 Ä".to_lower() == "mixed 42 Ä"
  assert "Content-Length".equals_ignore_case("content-length")
  assert "ab".repeat(3) == "ababab"
  assert digit_value(b'7') == Some(7)
}
```
