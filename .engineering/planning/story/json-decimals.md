---
format: aep.planning-md/3
id: story:json-decimals
kind: story
status: draft
title: std.json reads and writes numbers with a fraction or an exponent
relations:
- decomposes: epic:ignite-lsp
revision: 2
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

Adding a `Json` variant breaks every exhaustive match over `Json` (`design:ignite-lsp` §2 row
"JSON"). Options: a new variant `Decimal(Text)` that keeps the source text, or changing
`Number(Int)` to a number type with both forms. Decide before implementation.

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
