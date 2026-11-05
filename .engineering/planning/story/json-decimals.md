---
format: aep.planning-md/3
id: story:json-decimals
kind: story
status: active
title: std.json reads and writes numbers with a fraction or an exponent
relations:
- decomposes: epic:ignite-lsp
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T09:01:27Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-05T09:01:27Z", actor: "human:timo", revision: 5}
---
# std.json reads and writes numbers with a fraction or an exponent

Found by the adversary pass over `std.protocol.jsonrpc` (commit 159ac77, 2026-11-05): a valid
JSON-RPC request whose params hold `0.5` or `1e2` is answered with Parse error -32700, because
`std/json.ignite:8-10` refuses a fraction or exponent and `Json` has integer numbers only. LSP
`decimal` values (the `Color` structure, 5 uses in metaModel.json) take this path, and so does
any client that sends a number written with a fraction.

## Outcome

`std.json.parse` accepts every number RFC 8259 §6 allows and keeps it without loss; `encode`
writes it back. A program that only uses integers sees no change.

## Acceptance

1. `parse("0.5")`, `parse("1e2")`, `parse("-1.25E-3")` succeed; `encode` of each returns text
   that parses to the same value.
2. Integers parse to the same `Json` value as today; existing std and example tests pass on both
   targets.
3. The jsonrpc test "a request whose params carry a fraction or an exponent is not a parse
   error" (removed from 80f3f905's follow-up fix, text kept in this story's evidence) passes.

## Open decision

Decided 2026-11-05 by sys-mon under Timo's mandate: option A, a new variant `Decimal(Text)`.

- Parsing: a number token becomes `Number(Int)` only when it has no fraction, no exponent and
  fits `Int`; every other number token becomes `Decimal(Text)` holding the token's exact text.
  No float conversion anywhere.
- Encoding: a `Decimal` is written as its text unchanged, so parse then encode is
  byte-identical for every number token.
- Equality is structural: `Number(1) != Decimal("1.0")`, `Decimal("1.0") != Decimal("1.00")`,
  `Decimal("1e2") == Decimal("1e2")`. Numeric comparison is the caller's business.
- Every exhaustive match over `Json` gains a `Decimal` arm; the files are listed in the commit and
  in the Y9 review note.

## The removed test

Removed from `std/protocol/jsonrpc.ignite` in 0b28ae4e (adversary test of 80f3f905); restore it
when this story lands:

```ignite
// JSON-RPC 2.0 bodies are any JSON text: a number with a fraction or an
// exponent (an LSP `decimal`, as in a color) is valid JSON, so the request
// must not be answered with the parse error.
test "a request whose params carry a fraction or an exponent is not a parse error" {
  assert decode(r#"{"jsonrpc":"2.0","id":1,"method":"m","params":{"red":0.5}}"#) != Err(standard_error(PARSE_ERROR))
  assert decode(r#"{"jsonrpc":"2.0","id":1,"method":"m","params":[1e2]}"#) != Err(standard_error(PARSE_ERROR))
}
```

## Removed claim tests

Removed from `std/protocol/jsonrpc.ignite` in 831b72d4 (adversary tests of 9c3520cf); restore them when this story lands and turn rows R4_1_1, R4_6 and R5_9 of the JsonRpc claim from `unsupported` back to satisfied:

```ignite
// "The Server MUST NOT reply to a Notification" (R4_1_1): a notification
// whose params hold a fraction or an exponent is valid JSON (RFC 8259
// section 6) and a valid Notification, so it is not answered at all, not
// with a parse error.
test "a notification with a fractional or exponent param is not answered" verifies [JSON_RPC_2.R4_1_1] {
  let (client, server) = connected()?
  spawn JsonRpc.serve(server, JsonRpcLimits {}, echo_app())
  write_message(client, r#"{"jsonrpc":"2.0","method":"note","params":[0.5]}"#)?
  write_message(client, r#"{"jsonrpc":"2.0","method":"note","params":{"x":1e3}}"#)?
  write_message(client, r#"{"jsonrpc":"2.0","id":1,"method":"echo","params":["after"]}"#)?
  _ = client.close()
  assert replies_of(client)? == [r#"{"jsonrpc":"2.0","id":1,"result":["after"]}"#]
}

// "The Server MUST NOT reply to a Notification, including those that are
// within a batch request" (R4_1_1, its own text): a batch of notifications
// only is answered with nothing, batches supported or not.

// "It MUST be the same as the value of the id member in the Request
// Object" (R5_9): a valid request whose params hold a fraction is answered
// under its id 7, not under null with a parse error; the id is readable.
test "a request with a fractional param is answered under its id" verifies [JSON_RPC_2.R5_9] {
  let (client, server) = connected()?
  spawn JsonRpc.serve(server, JsonRpcLimits {}, echo_app())
  write_message(client, r#"{"jsonrpc":"2.0","id":7,"method":"echo","params":[1.5]}"#)?
  _ = client.close()
  assert replies_of(client)? == [r#"{"jsonrpc":"2.0","id":7,"result":[1.5]}"#]
}

// R5_9 and R4_6 for ids that are JSON numbers (R4_4 admits any Number; R4_5
// only discourages fractions): a fractional id and a 20-digit id come back
// as sent.
test "a request whose id is a fraction or has 20 digits is answered under that id" verifies [JSON_RPC_2.R4_6, JSON_RPC_2.R5_9] {
  let (client, server) = connected()?
  spawn JsonRpc.serve(server, JsonRpcLimits {}, echo_app())
  write_message(client, r#"{"jsonrpc":"2.0","id":1.5,"method":"echo","params":[]}"#)?
  write_message(client, r#"{"jsonrpc":"2.0","id":12345678901234567890,"method":"echo","params":[]}"#)?
  _ = client.close()
  assert replies_of(client)? == [
    r#"{"jsonrpc":"2.0","id":1.5,"result":[]}"#,
    r#"{"jsonrpc":"2.0","id":12345678901234567890,"result":[]}"#
  ]
}
```

## Adversary findings

Adversary pass over daac1848 and 4f39e312, 2026-11-05 (failing tests in 5aecab7a). The RFC 8259 §6
grammar held: 12 valid and 22 invalid tokens, exponent case and sign kept, ids echoed exactly, a
1 MB array of decimals in 0.13 s (rust) and 0.10 s (llvm). Findings, fixed in e9871722:

- medium: a 19-digit integer inside the 64-bit Int became a `Decimal`; now `Number` from
  -9223372036854775808 to 9223372036854775807, `Decimal` outside, checked on the digit text.
- medium: `-0` printed as `0`; it now parses to `Decimal("-0")` and round-trips.
- low: `Decimal(text)` built by hand is unchecked; added `decimal(text) -> Result<Json,
  JsonError>` and a doc note.
- found on the way: the generated std crate version `0.0.0-<hash>` was refused by cargo when the
  hash was all digits with a leading zero; now `0.0.0-h<hash>`, `CODEGEN_REVISION` bumped
  (renumber to main + 1 at merge, agreed with the ignite session).

JsonRpc claim: yes (0 gaps, 0 failures, 0 inconclusive) on both targets; R4_1_1, R4_6, R5_9
satisfied.
