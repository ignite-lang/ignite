---
format: aep.planning-md/3
id: approval-record:accepted-2026-11-12-language-and-rust-target
kind: approval-record
status: draft
title: 'Eight drafts accepted: five language and std stories, three Rust-target stories'
relations:
- decides: story:list-sort
- decides: story:list-contains-and-filter
- decides: story:compound-assignment
- decides: story:option-try-in-fns
- decides: story:std-set
- decides: story:rust-target-handle-method-keeps-parameter-lent
- decides: story:rust-target-literal-arguments-built-once
- decides: story:rust-target-text-literals-and-field-copies
revision: 1
---
# Eight drafts accepted on 2026-11-12: five language and std stories, three Rust-target stories

Decided on 2026-11-12 through the session Timo authorised that day to decide low-risk and easily reverted questions on his behalf. The coordinator read Timo's typed authorisation before applying the answers. Both questions listed their options; both answers were "A".

Question 1, the next language and std wave. Option A read: "accept `list-sort`, `list-contains-and-filter`, `compound-assignment`, `option-try-in-fns`, `std-set`." Reason given with the answer: the five most frequent workarounds counted in a real tool, and small units.

Question 2, three Rust-target drafts from the first user's allocation profile. Option A read: "accept all three, order: handle methods, literal arguments, then text literals and field copies."

What it accepts:

- `story:list-sort`, `story:list-contains-and-filter`, `story:compound-assignment`, `story:option-try-in-fns`, `story:std-set`: requested by the author of a command-line tool written in Ignite.
- `story:rust-target-handle-method-keeps-parameter-lent`, `story:rust-target-literal-arguments-built-once`, `story:rust-target-text-literals-and-field-copies`, in that order. The third starts with its design step, and its design is put to Timo again before it reaches `main`.

The order: `story:rust-target-text-reads-borrow` and `story:shared-list-costs-back` (accepted earlier) come first; then the three Rust-target stories and the five language and std stories, as the waves derive from their scopes; then the protocol clients (`story:std-websocket`, `story:http-client-role`, `story:data-codec-combinators`).

Not decided here: the other twelve drafts of 2026-11-12 (`std-regex`, `std-url-percent-encoding`, `data-xml`, `std-gzip-and-tar`, `map-composite-keys`, `enum-variants-with-named-fields`, `module-qualified-calls`, `glob-imports-resolve-by-receiver`, `declaration-words-as-identifiers`, `parameters-changed-in-place`, `file-named-at-run-time`, `child-output-and-files-as-streams`), the items a read-only audit of the store marked as needing a person, and accepting `story:rust-target-shared-list-storage`.
