# Row D (7.3): source files over 3,000 lines, 5 now, 4 at v0.15.0

Tree: `<tree>` (T), HEAD aa1defb2.
Scratch: `<scratch>/diag/counts` (S).
**V** = verified, with the command or file. **I** = inferred, not run.

## 0. The row

| file | lines now | lines at v0.15.0 | `git diff --stat v0.15.0..HEAD` |
|---|---|---|---|
| `crates/ignite-rustgen/src/lib.rs` | 7,484 | 7,157 | +387 -60 |
| `crates/ignite-llvm/src/lower.rs` | 6,073 | 5,752 | +360 -39 |
| `crates/ignite-check/src/lib.rs` | 4,854 | 4,726 | +144 -16 |
| `crates/ignite-types/src/typer.rs` | 4,644 | 4,559 | +120 -35 |
| `std/protocol/http.ignite` | 3,133 | 2,518 | +652 -37 |

V: `wc -l <file>`, `git show v0.15.0:<file> | wc -l`, `git diff --stat v0.15.0..HEAD -- <file>`.
All five grew. The four Rust files were already over at v0.15.0; `http.ignite` is the one that
crossed. `quality/baseline.json` 7.3 says so itself: "story http-app-connection grows
std/protocol/http.ignite from 2,518 to 3,125 lines ... splitting the module is left to a later
story" (V: `grep -n '"7.3"' -A8 quality/baseline.json`).

Counter (V: `crates/ignite/tests/quality.rs:1246-1257`): every `.rs` under `crates/*/src` and
under a `src` component of `tools/`, plus every `.ignite` under `std/`, with
`t.lines().count() > 3000`. Recursive, so a new file is counted on its own lines.

Near the bar, not over (V: `wc -l`): `crates/ignite-syntax/src/parser.rs` 2,981 (unchanged since
v0.15.0), `std/protocol/nats.ignite` 2,856 (new since v0.15.0).

## 1. std/protocol/http.ignite

### 1.1 What is in it

V: `S/d_outline.awk` -> `S/d-http-outline.tsv` (254 rows, one per top-level item with its
leading comment; item lines sum to 2,888, the rest are blank separators).

| kind | items | lines | with a clause |
|---|---|---|---|
| fn | 95 | 1,432 | 20 `satisfies` (504 lines) |
| test | 88 | 886 | 38 `verifies` (413 lines) |
| test fn (test-only helper) | 39 | 345 | - |
| type | 8 | 120 | - |
| enum | 10 | 58 | - |
| protocol | 1 | 32 | `conforms [RFC9112, RFC9110]` |
| const, import, package | 13 | 15 | - |

Inline tests and their helpers: 1,231 lines, 39 % of the file. Code: 1,657 item lines.

Candidate groups by region (V: `S/d_groups.py`, output `S/d-http-groups.txt`):

| group | lines | items | code lines | clause items | tests in range (lines) | items |
|---|---|---|---|---|---|---|
| api | 13-254 | 19 | 225 | 0 (protocol Http) | 0 | Request, Response, Content, HttpApp, Reply, HttpLimits, consts, `protocol Http`, serve_connection, HttpServer, serve_listener, limit_reasons |
| response | 255-345 | 7 | 76 | 0 | 1 (6) | reason, Response.json/plain/bytes/streamed, status_response, method_not_allowed |
| fields | 346-538 | 11 | 182 | 1 | 0 | has_line_break ... is_allowed_field, checked* |
| render | 539-834 | 15 | 155 | 3 | 12 (114) | Framing, render*, inline_framing, streamed_framing*, hex, chunk, send_streamed* |
| conn-options | 835-948 | 6 | 57 | 3 | 7 (44) | header, segments, lists_connection_option*, response_closes*, keep_alive* |
| lexical | 949-1141 | 16 | 162 | 1 | 1 (14) | is_token*, is_ows, trim_ows, is_field_value*, is_target*, absolute_form, is_host ... |
| head | 1142-1383 | 8 | 173 | 3 | 4 (57) | resolve_host*, target_too_long*, parse_head*, RequestLine, request_line, target_parts, origin_parts, head_authority |
| read | 1384-1751 | 18 | 335 | 7 | 1 (14) | read_head*, read_body*, next_request*, chunk_size*, read_line, read_chunk_data, read_chunked*, coding_status*, parse_length* |
| session | 1752-1853 | 5 | 97 | 2 | 0 | reject*, linger, asked, session*, respond |
| connection | 1854-2050 | 12 | 185 | 0 | 0 | Holds, HoldEvent, keep_holds, wait_held, drain_holds, upgrade, oversized, discard |
| tests | 2051-3133 | 0 | 0 | - | 101 (982) | the last 285 lines (2848-3132) are the http-app-connection tests |

`*` = carries `satisfies`.

Cross-group references (V, textual, same script; a few are word matches in strings): the api
types are used by every group, and api reaches back into session and connection
(`serve_connection -> session`, `Http -> Holds`); session <-> connection
(`session -> upgrade/oversized`, `oversized -> reject/respond`); render <-> session
(`send_streamed -> asked`). So api, render, session and connection are one cycle and cannot be
separate modules (import cycles among std modules are E032, `docs/slice.md` 4.1). The only
acyclic cut is a **leaf**: lexical + the clause-free field checks + the request-line helpers +
`reason` + `hex`/`chunk`, which use nothing but `std.text` (V: the split module checks with
`import std.text.*` alone, 1.4).

### 1.2 The rules that decide the split

| rule | source | consequence |
|---|---|---|
| module `std.a.b` is `<std>/a/b.ignite`; `package` must equal the path (E031) | V `docs/slice.md` 4, 4.1 | `std/protocol/http/x.ignite` is `std.protocol.http.x` and may sit beside `http.ignite`. Precedent: `std/rfc.ignite` + `std/rfc/*.ignite` |
| "A module exports every record, alias, enum, fn, const and protocol it defines. Imports are not re-exported." | V `docs/slice.md` 4.1 | No private items, no re-export. Every non-test item that leaves `http.ignite` changes its public path. `Request`, `Response`, `HttpApp`, `Reply`, `HttpLimits`, `Http`, `serve_*` must stay |
| `satisfies`/`verifies` count "for every protocol of its module that conforms"; a clause in a module without a conforming protocol is E092 | V `docs/slice.md` 19.5, 19.7 | The 20 `satisfies` fns and the 38 `verifies` tests must stay in the module of `protocol Http` |
| test-only items are usable only by tests of their own module and are not exported (E050, E051) | V `docs/slice.md` 4.1 (line 247), 12.2 | Inline tests moved to another module lose the 39 test helpers; they would have to be duplicated. Moved tests can still reach every non-test item (all are exported) |
| every `std.protocol.*` is protocol layer by prefix; rule 4 exempts the prefix from needing a table row | V `docs/layering.md` 20-38, `crates/ignite-check/tests/layering.rs` `prefix_layer` | A submodule needs no layering row and may be imported by `http.ignite` (same layer) |

### 1.3 The split (recommended)

New file `std/protocol/http/syntax.ignite`, `package std.protocol.http.syntax`,
`import std.text.*`; `http.ignite` gains `import std.protocol.http.syntax.*`.
35 clause-free, type-independent items move (V list: `S/d-move-names.txt`; original lines):

- reason 263-317
- has_line_break 346, is_list_field 348-357, is_auth_field 359-364, is_date_field 366-371,
  list_elements 373-398, has_empty_element 400-412, has_bws 414-445, is_digits 447-454,
  is_imf_fixdate 456-477, is_allowed_field 479-498
- HEX_DIGITS 646, hex 648-667, chunk 669-675
- is_token_byte 949-954, is_token 956, is_token_in 958-970, is_ows 972, trim_ows 974-976,
  trimmed_in 978-989, lowered_in 991-1001
- is_field_value_in 1021-1032, is_target 1034-1035, is_target_in 1037-1048,
  absolute_form 1050-1067, scheme_length 1069-1079, starts_lowered 1081-1095,
  is_reg_name_byte 1097-1102, is_host 1104-1140
- RequestLine 1249-1255, request_line 1257-1275, target_parts 1277-1287,
  origin_parts 1289-1304, head_authority 1306-1321
- hex_digit_value 1494-1500

Everything else stays: all types of the API, `protocol Http`, every `satisfies` fn
(`checked`, `is_field_value`, `parse_head`, ...), every test and test helper.

| file | lines | headroom to 3,000 |
|---|---|---|
| `std/protocol/http.ignite` | 2,636 | 364 |
| `std/protocol/http/syntax.ignite` | 510 | - |

V: `S/d_split.py` on a copy ("moved 35 items, 463 lines"), `wc -l`. The result is kept as
`S/d-http-split.diff` (http.ignite, 462 lines removed, 1 added) and `S/d-http-syntax.ignite`.

Public-path answer: **unchanged for everything an importer uses.** The 35 names are exported
today (there are no private items), so strictly `std.protocol.http.reason` becomes
`std.protocol.http.syntax.reason`, and likewise the other 34. No in-repo importer uses any of
them (V below). If `reason` should keep its path, leave it in `http.ignite`: 55 lines, result
2,691 (I, arithmetic).

### 1.4 Verification on a copy (built binary, `--std` on the copy, `--out` under S)

Binary `T/target/debug/ignite` (mtime 2026-11-10 23:20, HEAD committed 23:08; the tree's only
uncommitted changes are AGENTS.md and Cargo.toml). `S/d-exp` is removed; the summary is
`S/d-http-verify.txt`, the driver `S/d_run.sh`.

| check | result |
|---|---|
| `ignite check --std <copy> http.ignite`, `syntax.ignite` | V exit 0, no output |
| negative control: same `http.ignite` without the new import | V 59 diagnostic lines, E003 `unknown name reason`, `is_token`, ... (so `--std` reads the copy) |
| `ignite check --std <copy>` of all 11 importers (10 fixtures under `crates/ignite/tests/fixtures`, `examples/todo/todo.ignite`; list `S/d-importers-ignite.txt`) | V exit 0 each, same as against the original std |
| `ignite conformance http.ignite` (llvm), original vs split | V byte-identical matrix (`cmp`), 209 lines, exit 1 both: "rfc9112: no (1 gaps, 0 failures, 0 inconclusive)", "rfc9110: no (2 gaps, ...)". Rows 4.1 and 4.2 do not move |
| `ignite test --target llvm http.ignite syntax.ignite` on the split | V 88 passed, 0 failed (original: 88 passed) |
| `ignite test --list`, original vs split | V identical, 88 tests: the `std/protocol/http.ignite::<test>` unit names in `quality/baseline.json` stay |
| tree untouched | V `git status --short`: only AGENTS.md and Cargo.toml, as before |

Not run: `--target rust` tests of the split (I: fine, `std.rfc` + `std.rfc.*` is the same
parent-with-children shape in the generated crate); the `layering::` cargo tests (I: pass;
`protocol_names()` adds the derived name `http.syntax`, V `layering.rs:247-258`, and nothing
under `crates/*/src` contains that string, V `git grep -i 'http\.syntax'` empty); the full gate.

### 1.5 What else names http.ignite and has to change with the split

| place | change |
|---|---|
| `quality/baseline.json` 7.3 | bar 5 -> 4 with a decision note; nothing else in the file (unit names unchanged) |
| `AGENTS.md` Layout | one new row for `std/protocol/http/syntax.ignite`; the http row stays |
| `site/book/src/reference/std/http.md` | documents `reason` once and links the source path (line 10): say where the lexical fns live; `reference/std/index.md:38` lists the module |
| `docs/slice.md` | `reason` 7 and `hex` 3 backticked mentions (V `S/d_mentions.sh`): check which mean the http fns |
| `docs/layering.md` rule 3 text "(today telnet, http, jsonrpc, resp, nats)" | I: optional wording; no table row needed (rule 4) |
| `crates/ignite/tests/quality.rs:1280` `CONFORMING` | none: `std/protocol/http.ignite` keeps the protocol and the claims |
| `crates/ignite/tests/http_chunked_adversary.rs` `DECODERS` | none: `read_chunk_data`, `read_chunked`, `read_line`, `read_body`, `next_request` stay |
| goldens of rustgen and llvm | none reference `protocol.http` (V `git grep` empty) |
| external `ignite-lsp` repository | not checked (outside the tree); it is affected only if it names one of the 35 fns |

### 1.6 Alternatives, not recommended

- Move tests without `verifies` (50 tests, 473 lines) to a test-only module (precedent
  `std/sync.ignite`): 17 helpers (133 lines) are shared with `verifies` tests and would be
  duplicated (V `S/d_testdeps.py`), and the baseline unit names of the moved tests change.
- Let clauses of a submodule count for the parent module's protocol, or let a sibling test
  module use test-only items: a compiler and `docs/slice.md` change (19.5, 12.2), a story of its
  own. It would allow moving the 1,231 test lines, which is what will be needed when
  `http.ignite` grows again; 364 lines of headroom is about half of what wave 10 added.

## 2. The four Rust files

Outlines: `S/d-rs-ignite-*.tsv` (V `S/d_rs_outline.py`). Part sizes and the edit counts below:
`S/d-rs-cross.txt` (V `S/d_rs_cross.py`; the counts are textual upper bounds).

All four have the same shape: one `impl` block holds most of the file. The technique is the
one the crates already use: a child module with its own `impl X` block (V
`crates/ignite-check/src/clauses.rs:19` `impl Checker`, `crates/ignite-types/src/typer/{clauses,effects,system}.rs`
`impl Typer<'_>`). A child module sees the parent's private fields and fns; a method defined in
a child and called from the parent or a sibling needs `pub(super)`, and parent free items need
a `use super::...`. No behaviour changes, so the existing goldens and tests are the proof.

### 2.1 crates/ignite-types/src/typer.rs (4,644)

`impl<'a> Typer<'a>` 407-4644 (4,238 lines); no test module. Siblings: `typer/system.rs` 1,886,
`typer/effects.rs` 392, `typer/clauses.rs` 126.

| part | lines moved | size |
|---|---|---|
| `typer.rs` (setup, names, signatures, resolve_kind, bodies, block, stmt) | stays | 2,075 |
| `typer/expr.rs` (expr_kind, closure, record_update, name, list; field, try, unary, binary, operator) | 2075-2637, 3738-4244 | 1,070 |
| `typer/call.rs` (call, expression_call, path_call, enum_variant, builtin_variant, method, list_map) | 2638-3737 | 1,100 |
| `typer/pattern.rs` (branches, pattern, range_pattern, variant_pattern) | 4245-4643 | 399 |

Edits: at most 21 methods to `pub(super)`, about 20 parent items to import. Hard part: none
beyond that; `path_call` (270 lines) and `method` (165) move whole. Smallest of the four.

### 2.2 crates/ignite-check/src/lib.rs (4,854)

Builtin tables 88-580, `Symbols` 582-689, `impl Checker` 877-4172 (3,296), free fns 4174-4521,
`mod tests` 4523-4854 (332). Sibling: `clauses.rs` 358.

| part | lines moved | size |
|---|---|---|
| `lib.rs` (tables, Symbols, check_program, system, effects inference, types, roots, free fns) | stays | 2,026 |
| `collect.rs` (glob_imports, import, import_name, collect, associated_*, protocol_roles) | 878-1691 | 814 |
| `expr.rs` (stmt, expr, method, closure, static_type, call, path_call, pattern) | 2490-4171 | 1,682 |
| `tests.rs` (`#[cfg(test)] mod tests;`) | 4523-4854 | 332 |

Edits: at most 21 methods to `pub(super)` (tests part not counted), about 43 parent items to import (the builtin tables
are used everywhere). Constraints: `SUBSTRATE_MODULES` and `RFC_MODULE` stay in `lib.rs`,
because `docs/layering.md:70-71` allow-lists those std names for that file (rule 6); the file
itself stays, AGENTS.md defines the warm gate by `touch crates/ignite-check/src/lib.rs`.

### 2.3 crates/ignite-llvm/src/lower.rs (6,073)

`impl Lower` 216-5889 (5,674 lines); `fn builtin` alone is 938 lines (4484-5421). Siblings:
`glue.rs` 1,416, `ir.rs` 1,053, `own.rs` 964, `lib.rs` 914, `layout.rs` 723, `region.rs` 234.

| part | lines moved | size |
|---|---|---|
| `lower.rs` (Ctx, Lower, plumbing, calls, block, stmt, loops, regions, tail fns) | stays | 1,686 |
| `lower/expr.rs` (expr_kind, records, maps, tuple, closure, constant, name, construct, method, field, try) | 1502-2787 | 1,286 |
| `lower/ops.rs` (assert, show, binary, text concat, arithmetic failures, patterns) | 2788-3953 | 1,166 |
| `lower/builtin.rs` (runtime calls, capability, channel, builtin, list_map, spawn) | 3954-5888 | 1,935 |

Edits: at most 35 methods to `pub(super)`, about 18 parent items to import. Hard part: size of
the diff and `builtin` (one 938-line match; moves whole, splitting it is a separate change).
The IR goldens (`crates/ignite-llvm/tests/golden`) must stay byte-identical. No std-name
literal in the file (V `grep '"std\.'`), so layering rule 6 is not touched.

### 2.4 crates/ignite-rustgen/src/lib.rs (7,484)

The crate is this one file (V `ls crates/ignite-rustgen/src`). Project and std packaging
63-1510, writer and naming helpers 1512-1941, `impl Generator` 2044-5775 (3,732) and 5777-6606
(830), free helpers 6608-6785, resources and `executable_main` 6787-7300, Cargo manifests
7358-7484.

| part | lines moved | size |
|---|---|---|
| `lib.rs` (public API, std packaging, Generator struct, ownership and borrow helpers) | stays | 2,261 |
| `names.rs` (Writer, ident, rust_type, literals) | 1512-1941 | 430 |
| `decl.rs` (rust_ty, alias, record, enumeration, function, test_decl, assert, function_line) | 2689-3205 | 517 |
| `expr.rs` (block, stmt, expr, literals, fn_call, element access, call) | 3206-4575 | 1,370 |
| `call.rs` (spawn, static_call, method, indexed_mutation) | 4576-5216 | 641 |
| `pattern.rs` (match_expr, if_expr, pattern, test, extract) | 5217-5774 | 558 |
| `glue.rs` (eq and debug glue, closures, store, chunked, free helpers) | 5777-6785 | 1,009 |
| `system.rs` (resources, executable_main, Cargo manifests) | 6787-7484 | 698 |

Edits: at most 84 methods to `pub(super)`/`pub(crate)`, about 31 parent and 53 sibling items to
import. Hard part: no module structure exists yet, the free helpers are used across all parts,
and `impl Generator` alone (3,732) forces at least two cuts through it. Row 1.3 counts
`PatternKind::` and `Builtin::` over the whole crate `src`, so a move does not change it (I,
`quality.rs:1170-1198`). The Rust goldens must stay byte-identical.

## 3. Ranking and what each step buys

| step | file | effort | 7.3 after |
|---|---|---|---|
| 1 | `std/protocol/http.ignite` -> + `http/syntax.ignite` | small; verified on a copy; 1 new file, docs rows | 4 (the v0.15.0 value) |
| 2 | `ignite-types/src/typer.rs` | small to medium; 3 new child modules beside 3 existing | 3 |
| 3 | `ignite-check/src/lib.rs` | medium; 3 new modules, many table imports | 2 |
| 4 | `ignite-llvm/src/lower.rs` | medium to large; 3 new child modules, 4,400 lines move | 1 |
| 5 | `ignite-rustgen/src/lib.rs` | large; 7 new modules, 5,200 lines move | 0 |

Step 1 is enough for the row. Steps 2-5 are pure moves (I: not compiled here; the edit counts
are textual upper bounds), each independent of the others and each its own commit; they should
not run in parallel with stories that edit the same file. The risk that the row regresses
again sits in `parser.rs` (2,981), `nats.ignite` (2,856) and `http.ignite` itself (2,636).
