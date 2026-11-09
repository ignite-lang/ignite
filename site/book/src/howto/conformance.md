# Check a protocol against its RFC

A protocol can claim that it conforms to a specification. The specification is a catalogue
module with one constant per requirement. Declarations name the requirements they implement
with `satisfies`, tests name the ones they check with `verifies`, and `ignite conformance`
prints the matrix and whether the claim holds. This page imports part of RFC 9112, claims it
for a small protocol and checks the claim.

The standard library already carries catalogues for RFC 854, 855, 1143, 9110, 9112, JSON-RPC
2.0 and LSP 3.17 ([`std.rfc`](../reference/std/rfc.md#catalogues)). Import your own for any
other RFC, or for a section the standard catalogue leaves out.

## Import a catalogue

`ignite rfc import` reads a local RFCXML v3 file and opens no network connection. Download
the XML from the RFC Editor (`https://www.rfc-editor.org/rfc/rfc9112.xml`), then:

```sh
ignite rfc import rfc9112.xml --sections 2.2 --package lines.rfc9112 --out lines/rfc9112.ignite
```

The module holds one `Spec` constant with the source's digest and one `Requirement` per
sentence with a BCP 14 keyword:

```text
// RFC 9112: HTTP/1.1
// ignite rfc import rfc9112.xml --sections 2.2 --package lines.rfc9112
// Every party is guessed from its sentence; review each `party` line.

package lines.rfc9112

import std.rfc.{Level, Party, Source, Requirement, Spec}

const RFC9112: Spec = Spec {
  rfc = 9112
  title = "HTTP/1.1"
  url = "https://dx.doi.org/10.17487/rfc9112"
  digest = "sha256:ce82219c272374005115c909244a79e587687df3858edca4112b8563c02ccf19"
  requirements = [
    R2_2_1,
    …
    R2_2_10
  ]
}

const R2_2_1: Requirement = Requirement {
  rfc = 9112
  section = "2.2"
  ordinal = 1
  level = Must
  party = Recipient
  text = "A recipient MUST parse an HTTP message as a sequence of octets in an encoding that is a superset of US-ASCII [USASCII]."
}
…
```

The importer guesses each `party` from the sentence. Read every `party` line before you rely
on it.

`ignite rfc check` compares the catalogue's digest with the source file. It is silent and
exits 0 when they match:

```sh
ignite rfc check lines/rfc9112.ignite rfc9112.xml
```

A changed source exits 1:

```text
lines/rfc9112.ignite: digest sha256:ce82219c272374005115c909244a79e587687df3858edca4112b8563c02ccf19 differs from sha256:d50e509aebdf933cb191d4f40d56dc8dba8561c45a2ebbeda5f184fb3fb80576, the digest of changed.xml
```

## Claim the requirements

`lines/lines.ignite` is the module `lines` beside its catalogue. Its protocol conforms to
`RFC9112`, narrowed by `scope` to the recipient's requirements of section 2.2:

```ignite,fragment
package lines

import std.rfc.{Party}
import lines.rfc9112.{RFC9112}

type Echo {
  prefix: Text = "> "
}

const CR: Byte = 13
const LF: Byte = 10
const SP: Byte = 32

// The lines of `input`: CR LF or a bare LF ends one, a bare CR becomes a space.
fn split_lines(input: Bytes) -> List<Text> satisfies [RFC9112.R2_2_1, RFC9112.R2_2_2, RFC9112.R2_2_4] {
  var lines = []
  var line = Bytes.empty()
  var i = 0
  while i < input.len() {
    let b = input[i]
    if b == LF {
      lines.push(line.to_text())
      line = Bytes.empty()
    } else if b == CR {
      if i + 1 < input.len() && input[i + 1] == LF {
        lines.push(line.to_text())
        line = Bytes.empty()
        i = i + 1
      } else {
        line.push(SP)
      }
    } else {
      line.push(b)
    }
    i = i + 1
  }
  lines
}

protocol Lines conforms [RFC9112] scope { parties = [Recipient], sections = ["2.2"] } {
  fn serve<S: ByteStream>(stream: S, application: Echo) -> Result<Unit, IoError> requires [
    stream.read, stream.write
  ] {
    match stream.read(4096)? {
      Data(bytes) => {
        for line in split_lines(bytes) {
          stream.write_all("{application.prefix}{line}\r\n".to_bytes())?
        }
      }
      Eof => {}
    }
    stream.close()
  }

  unsupported RFC9112.R2_2_9 "no header fields: whitespace after the first line is data"
}

test "CR LF and a bare LF end a line" verifies [RFC9112.R2_2_1, RFC9112.R2_2_2] {
  assert split_lines(b"a\r\nb\nc\r\n") == ["a", "b", "c"]
}

test "a bare CR becomes a space" verifies [RFC9112.R2_2_4] {
  assert split_lines(b"a\rb\r\n") == ["a b"]
}

test "the server echoes each line over a pipe" verifies [RFC9112.R2_2_1] {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Lines.serve(server, Echo {})
  client.write_all(b"one\r\ntwo\n")?
  let reply = match client.read(4096)? {
    Data(bytes) => bytes.to_text()
    Eof => ""
  }
  assert reply == "> one\r\n> two\r\n"
}
```

The block is marked `fragment` because it imports the catalogue module beside it.

| clause | where | says |
|---|---|---|
| `conforms [RFC9112]` | the protocol | the protocol claims every requirement of the spec in scope |
| `scope { parties = […], sections = […] }` | after `conforms` | only these parties and sections are in scope |
| `satisfies [RFC9112.R2_2_1, …]` | a fn, role, type or field | the declaration implements the requirements |
| `verifies [RFC9112.R2_2_1, …]` | a test | the test is evidence for the requirements |
| `unsupported REF "reason"`, `not_applicable REF "reason"` | the protocol body | the requirement is a known gap, or does not apply |

The exact rules are in [Grammar](../reference/grammar.md) and
[`std.rfc`](../reference/std/rfc.md#claims).

## Give every MUST a status

`ignite check` refuses a MUST in scope with no status. Without the `unsupported` line:

```text
lines/lines.ignite:40:26: error[E090]: requirement `RFC9112.R2_2_9` (MUST, section 2.2) is in scope of `Lines` and has no status; add `satisfies`, `unsupported` or `not_applicable`
```

A `satisfies` or `verifies` that names a requirement of a spec no protocol of the module
conforms to is [E092](../reference/diagnostics.md#e092). See
[E090](../reference/diagnostics.md#e090) for the other case, two statuses on one requirement.

## Print the matrix

`ignite conformance` checks the file, runs its tests once on the LLVM target and prints one
block per protocol and spec:

```sh
ignite conformance lines/lines.ignite
```

```text
rfc9112 HTTP/1.1, protocol Lines, parties Recipient, sections 2.2
R2_2_1   2.2  MUST      satisfied    split_lines; test "CR LF and a bare LF end a line" passed, test "the server echoes each line over a pipe" passed  pass
R2_2_2   2.2  MAY       satisfied    split_lines; test "CR LF and a bare LF end a line" passed                                                         pass
R2_2_3   2.2  MUST NOT  none         -                                                                                                                 out of scope
R2_2_4   2.2  MUST      satisfied    split_lines; test "a bare CR becomes a space" passed                                                              pass
…
R2_2_9   2.2  MUST      unsupported  "no header fields: whitespace after the first line is data"                                                       gap
R2_2_10  2.2  SHOULD    none         -                                                                                                                 out of scope
claim conformance of Lines to rfc9112: no (1 gaps, 0 failures, 0 inconclusive)
```

The columns are the id, section, level, status, evidence and verdict. An `unsupported` MUST is
a `gap`, so the claim does not hold and the command exits 1. A satisfied requirement passes
when every test that verifies it passed.

`Lines` has no header fields, so R2_2_9 does not apply to it. With
`not_applicable RFC9112.R2_2_9 "a Lines message has no header fields"` in place of the
`unsupported` line, the row's verdict is `n/a` and the claim holds, exit 0:

```text
claim conformance of Lines to rfc9112: yes (0 gaps, 0 failures, 0 inconclusive)
```

## The standard library's claims

The Telnet module claims RFC 854 and RFC 855. In a clone of the repository, with the column
padding shortened below:

```sh
ignite conformance std/protocol/telnet.ignite
```

```text
rfc854 Telnet Protocol Specification, protocol Telnet, parties all, sections all
RP2_1   p2   MAY         none            -                                                                     unclassified
RP2_2   p2   MUST NOT    satisfied       decode_within; test "DO is answered WONT and WILL is answered DONT" passed  pass
…
RP4_2   p4   REQUIRED    not_applicable  "a hope about the design of options; Telnet defines no option"        n/a
…
claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)

rfc855 Telnet Option Specifications, protocol Telnet, parties all, sections all
…
claim conformance of Telnet to rfc855: yes (0 gaps, 0 failures, 0 inconclusive)
```

A SHOULD or MAY with no status is `unclassified` and does not break the claim.

Every flag and exit status: [`ignite rfc import`](../reference/cli.md#ignite-rfc-import),
[`ignite rfc check`](../reference/cli.md#ignite-rfc-check),
[`ignite conformance`](../reference/cli.md#ignite-conformance).
