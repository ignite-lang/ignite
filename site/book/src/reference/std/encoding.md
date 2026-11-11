# std.encoding

The binary-to-text encodings of RFC 4648: base64, base64url and lowercase hex. Encoding is
total. Decoding returns the bytes, or the first character it refuses.

- Layer: base. It imports nothing.
- Source: [`std/encoding.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/encoding.ignite).
- Import: `import std.encoding.{base64, base64_decode, hex}`.

## Functions

```ignite,fragment
type EncodingError {
  offset: Int
  message: Text
}

fn base64(data: Bytes) -> Text
fn base64url(data: Bytes) -> Text
fn base64url_unpadded(data: Bytes) -> Text
fn base64_decode(text: Text) -> Result<Bytes, EncodingError>
fn base64url_decode(text: Text) -> Result<Bytes, EncodingError>
fn base64url_unpadded_decode(text: Text) -> Result<Bytes, EncodingError>
fn hex(data: Bytes) -> Text
```

| function | what it does |
|---|---|
| `base64` | RFC 4648 section 4, `=` padding to a multiple of four characters |
| `base64url` | section 5, `-` and `_` for `+` and `/`, padded |
| `base64url_unpadded` | section 5 without padding, as JWS (RFC 7515) uses it |
| `base64_decode`, `base64url_decode` | the bytes of a padded text in that alphabet |
| `base64url_unpadded_decode` | the bytes of an unpadded base64url text |
| `hex` | section 8 in lowercase, two digits per byte, as a digest is printed |

`EncodingError.offset` is the byte offset of the first character refused, or the text's length
when only the length is wrong. Decoding checks in this order:

| refused | message |
|---|---|
| a padded text whose length is no multiple of 4 | `the length of padded base64 is a multiple of 4` |
| an unpadded text whose length is 1 more than a multiple of 4 | `no base64 text has this length` |
| a `=` that is not one of the last two characters of a padded text | `` `=` only as the last one or two characters `` |
| a character outside the alphabet: the other alphabet's two, a `=` in an unpadded text, line breaks and white space included | `not a base64 character` |
| non-zero bits after the last byte | `the bits after the last byte are not zero` |

The last rule is section 3.5's canonical encoding: every byte sequence has exactly one text.

## Example

```ignite
import std.encoding.*

test "base64 and hex of RFC 4648 section 10" {
  assert base64(b"foobar") == "Zm9vYmFy"
  assert base64(b"fo") == "Zm8="
  assert base64url(b"\xfb\xff") == "-_8="
  assert base64url_unpadded(b"\xfb\xff") == "-_8"
  assert base64_decode("Zm8=") == Ok(b"fo")
  assert hex(b"\x00\x0f\xa9\xff") == "000fa9ff"
}

test "decoding names the first character it refuses" {
  assert base64_decode("Zm9v!A==") == Err(EncodingError { offset = 4, message = "not a base64 character" })
  assert base64_decode("Zh==") == Err(EncodingError { offset = 1, message = "the bits after the last byte are not zero" })
}
```
