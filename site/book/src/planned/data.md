# Data formats: JSON, YAML and TOML from one value model <span class="badge design">design</span>

Planned: turning Ignite values into JSON, YAML and TOML text and back, with no encoder and
decoder written by hand for each type. Released in v0.15.0: the value model `std.data` and its
bridge to `std.json` (`read`, `write`, `to_data`, `of_data`). The codecs, YAML, TOML and the
derivation are not built; the code below is the planned form and does not compile today.
The design, with its measurements and the rejected alternatives, is
[`docs/serde.md`](https://github.com/ignite-lang/ignite/blob/main/docs/serde.md).

## Read YAML, write JSON

```ignite,fragment
import std.data.{codec, decode, encode_with, Codec, Shape, Case}
import std.yaml
import std.json

type Rule {
  name: Text
  match_path: Text
  rate_limit: Option<Int>
  action: Action
}

enum Action { Allow, Deny(Text), Redirect { to: Text, status: Int } }

fn load(text: Text) -> Result<List<Rule>, Text> {
  let value = yaml.read(text).map_err(|e| e.message)?
  let rules: Codec<List<Rule>> = codec()
  decode(rules, value).map_err(|e| e.describe())
}

fn wire(rules: List<Rule>) -> Text =
  json.write(encode_with(codec(), rules, Shape { keys = Case.Camel, ..Shape.default() }))
```

`codec()` is written by the compiler from the type `Rule`. A YAML document whose third rule
has a number where `match_path` expects text fails with the path and what was expected:
`$[2].match_path: expected Text, found Int`.

## Four layers

| layer | module | what it is | compiler change |
|---|---|---|---|
| value model | `std.data` | `Value`, the one tree every format reads into and writes from; `Shape` for wire names and enum tagging; `DecodeError` with a path; limits on depth and size | none |
| codecs | `std.data` | `Codec<T>`, a pair of functions, and combinators to build one for any type by hand | none |
| formats | `std.json`, `std.yaml`, `std.toml` | `read` text into a `Value`, `write` a `Value` as text | none |
| derivation | `codec<T>()` | the codec the combinators would build, written by the compiler | one frontend pass |

The first three layers are plain Ignite in `std` and need no compiler change. Derivation comes
last and removes the hand-written codec.

- **Names without attributes.** `Shape` renames keys (`Case.Camel`, `Kebab`), chooses how enums
  are tagged (`External`, `Internal("type")`, …) and whether unknown members are ignored or
  refused, per call rather than per type.
- **Safe YAML.** YAML 1.2.2 under the core schema. Anchors, aliases, custom tags, complex keys
  and the `<<` merge key are refused with an error that names the line.
- **JSON as it is.** `std.json` has `read` and `write` over `Value` since v0.15.0, and numbers
  that do not fit `Int` keep their exact text.

## Plan

| step | content | state |
|---|---|---|
| value model | `std.data`'s `Value`, `Shape`, errors and limits; the bridge to `std.json` | released in v0.15.0 |
| codecs | `Codec<T>` and its combinators, in `std` only | planned (wave 9) |
| YAML | `std.yaml`, the safe subset | planned (wave 11) |
| derivation, TOML | `codec<T>()` written by the compiler; `std.toml` | designed |

The [roadmap](../roadmap.md) shows where these steps sit among the others.
