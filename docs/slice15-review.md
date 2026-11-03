# Slice 15 contract review

Status: review of the drafted contract, 2026-11-03; findings resolved in section 19 (see "Resolution").

Independent review of `docs/slice.md` section 19 (slice 15, protocols P2: RFC catalogues,
linking clauses, E090 to E092, `ignite rfc import` and `check`, `ignite conformance`) as
committed in `52b1300` on `s15/contract`. 2 blockers, 10 should-fix, 17 notes. Line numbers into
`docs/slice.md` refer to `52b1300`; code is cited by symbol, with the line at the slice base
where it helps.

Reviewed against `docs/vision.md` §6 (X1 to X24, L1 to L4) and invariants 1 to 9;
`docs/protocols.md` §3 and §7 row P2; `AGENTS.md`; `docs/layering.md`; and the code of the base
(`std/`, `crates/ignite-{syntax,check,types,rustgen,llvm,io,eval}`, `crates/ignite`). Four claims
were checked at run time with the `ignite` binary of 2026-11-02 16:48 in a scratch directory
(module coexistence, unreferenced constants on both targets, failure-site positions in Rust
emission, clause-only imports). The LLVM string and debug code is unchanged since that binary
(`git log` of `crates/ignite-llvm/src/ir.rs` and `lower.rs`). Every other finding is read from
source. The RFC sources of 19.9 were fetched and their digests, sizes and requirement counts
recomputed.

## Vision §6

| item | result | where |
|---|---|---|
| X1 backend primacy | respects: `ignite conformance` defaults to `rust` | D8, line 5751 |
| X2 interpreter | respects: `ignite-eval` evaluates the catalogue constants and the claim model at check time | 19.2 line 5201, 19.7 line 5412 |
| X3 IR layer | respects: no new IR, the claim model is internal, output is text (JSON deferred) | D8, 19.10 |
| X4 priority order | not applicable | |
| X5 panics end a thread | not applicable | |
| X6 diagnostic codes | respects: E090 to E092 lie in T4's protocol range E090 to E099; E093 to E095 and E098 stay reserved, E096 keeps its §3.3 meaning, E097 unused | 19.12 line 5667, section 8 lines 858 and 915 to 917 |
| X7 `assert`, not `expect`/`fail` | respects: `verifies` marks ordinary `test` blocks with `assert` | 19.5, 19.10 |
| X8 todo example | respects: `examples/todo/` unchanged | 19.16 |
| X9 substrate libraries | respects: hand-written XML reader, no new crate; `sha2` is already a dependency of `crates/ignite` | D6 line 5749 |
| X10 `await` | not applicable | |
| X11 declaration inventory | respects: the clause words are contextual, not keywords (invariant 6) | 19.5 line 5339 |
| X12 `Text` | respects: every textual field is `Text` | 19.3 |
| X13 design §11.2 | not applicable | |
| X14 archive path | not applicable | |
| X15 title | respects: the title says slices 1 to 15 | `docs/slice.md` line 1 |
| X16 telnet bound | not addressed: the bound is exceeded and U6 grows the file; 19.14 does not record it (finding 25) | 19.14 |
| X17 keyword count | respects: no keyword is added | 19.5 |
| X18 cite symbols | respects: section 19 cites symbols, no line numbers into sources | 19.2 |
| X19 layout table | partly: row 7 asks for layout rows, but not all that the slice adds (finding 26) | 19.0 row 7 |
| X20 to X24 | not applicable | |
| L1 to L4 | not applicable (actors, wasm, roots) | |

## Findings

### Blockers

1. **Blocker. `ignite emit` with and without clauses cannot be byte-identical (19.0 row 12,
   line 5140).**
   - Evidence:
     - Rust emission embeds the source file name and the line and column of every failure
       site: `position` (`crates/ignite-rustgen/src/lib.rs` line 2219) and
       `escaped_site_file` (line 1697). Observed: `fn add(a: Int, b: Int) -> Int = a + b`
       emits `ignite_io::checked::add(a, b, "site.ignite", 1, 33)`.
     - `claims.ignite` and `stripped.ignite` differ in name, and deleting a clause moves every
       later line. Their output therefore differs wherever an arithmetic, index or `?` site
       follows a clause.
     - LLVM IR carries `DIFile` and `DILocation` for the same positions.
     - `ignite emit` has no `--target` (`Command::Emit` in `crates/ignite/src/main.rs`);
       `emit_with_std` produces Rust only. "once per target" has no command on LLVM.
   - Required change:
     - Keep one file name: `clauses/claims.ignite` and `clauses/stripped/claims.ignite`.
     - Strip by replacing each clause with spaces and newlines of the same shape, so that
       every remaining token keeps its line and column. State this in row 12.
     - For LLVM, compare the `.ll` files `ignite build --target llvm` writes, or call
       `ignite::emit_llvm` from the test, and name which.

2. **Blocker. The LLVM target keeps the text of unreferenced constants (19.0 row 5, line 5133;
   D2, line 5745; 19.2, line 5199).**
   - Evidence:
     - `lower_decls` (`crates/ignite-llvm/src/lib.rs`) lowers every `Decl::Const` of every
       loaded module through `Lower::define_const` (`lower.rs` line 290). A `Text` literal
       goes through `literal_ref` (line 620) to `Module::string` (`ir.rs` line 575), which
       records `@.str.N`.
     - `Module::prune` (`ir.rs` line 319) prunes functions only. `Module::finish` (line 597)
       writes every entry of `strings` and every debug node, pruned or not.
     - Observed: an unreferenced `const DIGEST: Text = "sha256:probe-unreferenced-constant-text"`
       in a std module is absent from the Rust emission, but `std.ll` keeps
       `@.str.0 = private unnamed_addr constant [39 x i8] c"sha256:probe-unreferenced-constant-text"`
       and `!4 = distinct !DISubprogram(name: "std.cat.DIGEST", …)`.
     - `telnet.ignite` and `http.ignite` will import their catalogues for `conforms`, so the
       terminal and todo programs load the catalogue modules, and the five digests land in
       `std.ll`. Row 5 fails on LLVM as written.
     - 19.2's "An unreferenced constant is not emitted" holds for Rust only.
   - Required change, one of:
     - Prune `strings` and the debug nodes of pruned functions in `Module::finish`. That is a
       backend change: bump `apply::plan::CODEGEN_REVISION` and give the change to a unit
       (U1 or U6) in 19.16.
     - Or define row 5 on the linked executables (`--gc-sections` drops unreferenced data)
       and say that `std.ll` may carry catalogue text.
   - Correct the 19.2 row in either case.

### Should-fix

3. **Should-fix. `ignite-eval` has no `Symbols`, so the claim model cannot read them (19.6
   line 5407, 19.7 lines 5418 to 5419, 19.13, 19.0 row 18).**
   - Evidence:
     - `ignite_eval::Module` holds `path`, `source`, `program` and `types`
       (`crates/ignite-eval/src/lib.rs`). `Loaded::modules` (`crates/ignite/src/lib.rs`,
       around line 1068) builds it without `Symbols`.
     - `crates/ignite-eval/Cargo.toml` depends on `ignite-syntax` and `ignite-types` only and
       has no dev-dependencies; its tests are `fuel.rs` and `value.rs`, which build no
       program from source. Row 18's tests in `ignite-eval` have no frontend to call.
     - "No change in `crates/ignite/src/` is needed" is then false.
   - Required change:
     - Record the resolved constant of each `conforms`, `satisfies` and `verifies` entry in
       `TypedProgram` (`ignite-types` already sees `Symbols`), or add `symbols` to
       `ignite_eval::Module`. The second changes `Loaded::modules`, which U3 also edits in W3;
       name the owner.
     - Put the claim-model tests in `crates/ignite/tests`, or add the dev-dependencies to
       `ignite-eval` with a line in the commit message.

4. **Should-fix. Three fixture harnesses do not fit the planned fixtures (19.0 rows 9, 11 and
   14; 19.16 U1, U2).**
   - `plan::fixtures_match_expected` (`crates/ignite/tests/plan.rs` line 337) runs every
     `e0*` fixture it selects and asserts exit 1, empty stdout and `error[E0xx]` on stderr.
     `e090_complete_claim`, which must stay silent, breaks that assertion.
   - The fixture std of `crates/ignite-check/tests/fixtures.rs` asserts the module list
     `["std.greeting", "std.protocol.hello", "std.salute"]`; that of
     `crates/ignite-types/tests/fixtures.rs` asserts `["std.net", "std.shapes",
     "std.shapes.protocol"]`. Rows 9 and 11 need `std.rfc` in both, and
     `e003_unknown_requirement` needs a catalogue module too.
   - `valid_clauses` is named as one fixture but is needed in three crates.
   - Required change: list the two module-list assertions and the harness filter in U1's and
     U2's scope, and give the silent fixture a name the filter skips (`ok_complete_claim`).

5. **Should-fix. A fixture catalogue cannot be a sibling, and two `Spec`s in one module are E091
   (19.4 line 5274, 19.0 rows 14 to 17).**
   - Evidence: `plan.rs` runs each fixture as a lone program in the fixtures directory. It has
     no package, so `parse_siblings` finds no sibling, and an import of a non-std module is
     E031. The catalogue must sit in the program module, which contradicts "nothing else" in
     19.4. The E092 fixtures need two specs, which is the E091 case "two `Spec` constants in
     one module".
   - Required change: restrict "nothing else" to modules under `std/rfc/`, and give the E092
     fixtures a `--std` fixture directory with two catalogues, passed through the first-line
     `// ignite <sub> args` that `plan.rs` already reads.

6. **Should-fix. The E091 check of a conformed catalogue in another module has no position
   (19.4, second trigger; 19.0 row 17).**
   - Evidence: `Loaded::check` (`crates/ignite/src/lib.rs` line 1115) renders every
     diagnostic of `Model::check` against `self.source()`, the program's source. A malformed
     catalogue in `std/rfc/` would be printed with a span into the wrong file.
   - Required change: run the catalogue rules only on the `Spec` constants of the checked file
     (row 25 already checks every std catalogue), or position the diagnostic at the `conforms`
     entry and name the catalogue in the message.

7. **Should-fix. Row 17 asks for one E091 per case, but the cases overlap (19.4, 19.0 row 17,
   line 5145).**
   - Evidence: constant names are unique per module (E004), and a name encodes section and
     ordinal. Two requirements with one section and ordinal therefore always carry a name that
     does not match, and produce two cases at once. The case "ordinal under 1" is in 19.4's
     list but not in row 17.
   - Required change: state a precedence (name mismatch first, duplicate never alone) and add
     the ordinal case to row 17.

8. **Should-fix. Verdict rule 5 lets a run pass with a test that did not run (19.10 line 5615).**
   - Evidence: "`pass` when a test passed" is met by one passing test of several verifying
     tests. `docs/protocols.md` §3.3 line 293: "a skip is not a failure, and any skip makes
     the result inconclusive". The runner has no skip verdict (`Verdict` in
     `crates/ignite-io/src/test.rs` line 161), so "did not run" means filtered out or absent.
     19.14 does not record a deviation.
   - Required change: `pass` only when every verifying test of the requirement ran and passed,
     at least one; else `inconclusive`. Or record the deviation in 19.14.

9. **Should-fix. Exit statuses are undefined at the edges (19.10 step 3, D8, 19.0 row 37).**
   - "A build or run failure is exit 3" does not say whether the test executable's own exit 1
     (a failing test) is a run failure.
   - D8 and row 37 want diagnostics as 2 and a failed build as 3. `Error::exit_status`
     (`crates/ignite/src/lib.rs`) maps diagnostics to 1, cargo and clang to 2, IO to 3.
   - `ignite rfc import` names exit 2 for reader errors (row 23) but no status for a missing
     or unreadable file.
   - Required change: define a run failure (spawn error, signal, an exit status other than 0
     or 1, or no `test result:` line); state that `main.rs` maps the errors itself, as `run`
     does with `RUN_DIAGNOSTIC`; give the IO statuses of `ignite rfc`.

10. **Should-fix. D2's guarantee does not hold for a program's clause-only imports (D2,
    line 5745).**
    - Evidence: a program's `use` lines are roots of the Rust reachability walk. Observed: a
      program with `import std.rfc.rfc1.{R1_1}` and no use of `R1_1` emits
      `use ignite_std::rfc::rfc1::R1_1;` and `pub fn R1_1()` with its text.
      `docs/protocols.md` §6.1 line 660 says catalogues emit nothing.
    - Required change: weaken D2 to "a catalogue reaches generated code only through a name
      the program imports or uses", or drop imports that only clauses use in rustgen (a
      backend change: bump `CODEGEN_REVISION`).

11. **Should-fix. The layering sentence contradicts the table and nothing enforces it (19.11
    line 5649).**
    - Evidence: `docs/layering.md`'s table lets a general module import a leaf; 19.11 says "No
      general or base module may" import the catalogues. `layering::` in
      `crates/ignite-check/tests/layering.rs` has no such rule. 19.11 names only `layer_of`
      (line 150) for the new prefix, but rule 4 goes through `StdModule::layer` (around line
      109) and the text of rule 4 in `docs/layering.md` names the prefixes too.
    - Required change: add a rule 6 to `docs/layering.md` and to `layering.rs` together, or
      drop the sentence; name `StdModule::layer` beside `layer_of`.

12. **Should-fix. The level of a prose requirement is not defined (19.9, 19.0 row 27).**
    - Evidence: RFC 854 and 855 predate BCP 14. `RP14_1` takes Must from "need be"; no rule in
      19.9 says which words give which level. U4 writes the 854 catalogue and U6's matrix
      depends on which ids are MUST-class, yet row 27 fixes only RFC 855 and `RP14_1`.
    - RFC 855 page 1, "Options will be publicized by publishing their documentation as RFCs;
      inventors of options may, of course, publicize them in other ways as well.", is a MAY by
      the table's own rule and is missing.
    - Required change: write the mapping (must, required, necessary, need be: MUST; should,
      desirable: SHOULD; may: MAY), list the MUST-class ids of RFC 854 in U4's commit for
      review, and include or exclude the 855 sentence explicitly.

### Notes

13. **Note. `Checked::check` does not exist (19.2 line 5203, 19.7 line 5419).** The function is
    `Loaded::check` (`crates/ignite/src/lib.rs` line 1115).

14. **Note. The struct-literal row of 19.2 (line 5196) is inaccurate.**
    `crates/ignite-types/tests/surface.rs` builds no AST struct literal.
    `crates/ignite-rustgen/tests/units.rs` does (around lines 218, 277, 1019, 1148), the
    `#[cfg(test)]` module of `crates/ignite-check/src/lib.rs` builds `FnDecl` (line 3851) and
    `ConstDecl` (line 4025), and the parser builds all of them. U1 also changes the message
    of `crates/ignite-syntax/tests/fixtures/e001_protocol_member.expected` ("expected `fn` or
    `}` in a protocol, found `type`"); say so.

15. **Note. `test_build` is private, but its API is public (19.2 line 5205).** `lib.rs` line 22
    re-exports `TestGroup`, `TestOptions`, `build_tests`, `test`, `test_groups` and
    `test_targets` as `pub`. `ignite conformance` can call them without a visibility change.

16. **Note. The catalogue vocabulary path is stale elsewhere.** 19.3 puts `std.rfc` in
    `std/rfc.ignite`, and `52b1300` corrected `docs/protocols.md` line 232. Its §7 row P2
    (line 716) and the stories `story:slice-15-frontend` (summary, scope path, acceptance) and
    `story:slice-15-contract` (D1) still say `std/rfc/rfc.ignite`. Correct them with U0.

17. **Note. Minimums against system overrides are dropped silently (19.7).**
    `docs/protocols.md` §3.6 line 400 checks each `minimum` "against the default and against
    every system's override". 19.7 checks the default only; record it in 19.14.

18. **Note. The result lines must be read from a bounded region (19.10).** `run_tests`
    echoes failing children's stdout and stderr under `failures:`, so a child can print a line
    of the form `test x ... ok`. Parse only from `running N tests` to the first blank line. The
    full suites hold TCP tests; comparing whole outputs across targets risks flakes.

19. **Note. Row 28 refers to "the texts of row 2", but row 2 paraphrases `R3_2_6`.** Quote the
    sentence verbatim: "A server MUST respond with a 400 (Bad Request) status code to any
    HTTP/1.1 request message that lacks a Host header field and to any request message that
    contains more than one Host header field line or a Host header field with an invalid field
    value."

20. **Note. Row 3 does not say whether each row's evidence names both tests** (the DO/WILL test
    and the subnegotiation test of `decode_within`). Say "each".

21. **Note. `conforms` takes a `path` in the production (19.5) but its resolution treats entries
    as idents.** Say whether `conforms std.rfc.rfc854.SPEC` is allowed or only an imported name.

22. **Note. Row 30 does not say what the recorded header path is relative to.** Name the
    working directory or the repository root.

23. **Note. The block header's section list has no separator (19.10).** Say `, `.

24. **Note. The D4 subset of RFC 9110 leaves out §8.6 (Content-Length) and §15 (status codes),
    which `http.ignite` implements.** Record the omission in 19.14 or add them.

25. **Note. Vision X16 is not addressed.** `std/protocol/telnet.ignite` has 762 non-test lines at
    the base (awk count of lines outside top-level `test` blocks; slice 14's item 14 recorded
    about 655), against the bound of 400. U6 adds clauses to it. Add an item to 19.14 as slice
    14 did.

26. **Note. Row 7's documentation list is short.** `AGENTS.md` also needs command rows for
    `ignite rfc import`, `ignite rfc check` and `ignite conformance`, a layout row for
    `crates/ignite/src/rfc/`, the new commands in the `crates/ignite/` row, and the
    `std/rfc.ignite` and `std/rfc/` rows. `docs/vision.md` §4.1 row "15+ | B (P2 on), C, D, E,
    G, H" needs its slice-15 state, and no unit owns `docs/vision.md`.

27. **Note. Licence statements differ per source (19.9, Q3).** RFC 1143 says "This RFC is
    Copyright 1990, Daniel J. Bernstein. However, distribution of this memo in original form
    is unlimited."; RFC 854 and 855 predate the IETF Trust. Quoting requirement text into
    catalogues may need each source's own statement cited. Unclear; ask.

28. **Note. The 19.17 measurements need Rust tooling.** `AGENTS.md` requires tools in Rust
    (`tools/ignite-perf`); name the mode. `task test-ignite` passes every `std/**/*.ignite`, so
    the catalogues are compiled and tested on both targets; name that as an expected cause of
    the inline rise.

29. **Note. The Rust mirror enums of `Level`, `Party` and the statuses in `ignite-eval` are a std
    vocabulary inside a compiler crate.** Record them in `docs/layering.md`'s table of what a
    crate may know, as `std.deploy`'s hooks are.

## Verified, no finding

- `std.rfc` in `std/rfc.ignite` beside `std/rfc/rfcN.ignite`: `module_path`
  (`crates/ignite/src/lib.rs` line 305) and `std_file` (line 502) map both, and module paths
  are flat keys in `ignite-check`'s `import`. Observed: a scratch std with `std/rfc.ignite`
  and `std/rfc/rfc1.ignite` passes `ignite check` of a program using both, of each file alone,
  and `ignite emit` yields `pub mod rfc { … pub mod rfc1 { … } }`. Precedent:
  `crates/ignite-types/tests/fixtures/std` holds `std.shapes` and `std.shapes.protocol`.
- Rust drops unreferenced std constants: `reachable` (`crates/ignite-rustgen/src/lib.rs`
  line 708) and the filtered `use` lines of `LoweredModule::render`. Observed: the probe
  constant is absent from the Rust emission.
- `Model::check` reaches the protocols of a std file given as the program:
  `load_std_reachable` and `load_program_std` add the file's own module, and
  `Loaded::modules` inserts the program last, replacing the std copy. The early return
  `if program.systems().is_empty() && program.assemblies().is_empty()`
  (`crates/ignite-eval/src/model.rs` line 1168) must not skip claims; 19.7 says so.
- `ignite test` reporting: `running N tests`, one `test <module>/<name> ... ok`, `FAILED` or
  `FAILED (<reason>)` line per test, a `failures:` section, then `test result:`; exit 0 or 1.
  Verdicts are passed, failed, exited, signal and timed out (`Verdict::word`, line 188). No
  skip exists; one filter only.
- E090 to E092 are free: nothing emits them and section 8 defines them only in the slice-15
  rows. `Level`, `Party`, `Source`, `Requirement`, `Spec` and the clause words collide with no
  builtin (`BUILTIN_TYPES`, line 97) and no std name; `satisfies` in `std/sli.ignite` (line
  22) is a function, which a contextual word does not shadow.
- No XML or network crate is in `Cargo.lock`; `sha2 = "0.10"` is in `crates/ignite/Cargo.toml`.
- Section 3's protocol production is as 19.2 quotes it.
- `CODEGEN_REVISION = "ignite-codegen/0.1.0-2"` (`crates/ignite/src/apply/plan.rs` line 17).
- Layering rules 3 and 5 hold for leaf catalogues imported by `std.protocol.*`.
- One integration-test binary per crate: every new test file of 19.16 is a `mod` of its
  crate's `tests/main.rs`.
- The RFC sources: every sha256 digest and size of 19.9 matches. RFC 9112 has 149 `bcp14`
  marks (11 in §1.1, 137 in §2 to §9, 1 in §10); RFC 9110 has 445, of which the D4 subset
  holds 75. `R3_2_6` is the Host sentence; `R7_1_2` is the chunked MUST, since §7.1's first
  marked requirement is an OPTIONAL; RFC 854's "With the current set-up" begins on page 14,
  so `RP14_1` is right.
- The current-state rows on Telnet and HTTP: `decode_within` (telnet line 395) and the tests
  "DO is answered WONT and WILL is answered DONT" (line 486) and "a subnegotiation is skipped
  up to IAC SE" (line 511); `HttpLimits` (line 57), `parse_head` (line 330), `next_request`
  (line 460); 37 and 41 inline tests, as row 6 requires.

## Resolution

Folded into `docs/slice.md` section 19 on `s15/contract`, 2026-11-03. Section numbers below
are of that text.

| finding | resolution |
|---|---|
| B1 (1), row 12 emit identity | 19.0 row 12: one path, one working directory, the clause characters blanked to spaces so every token keeps its line and column; `ignite emit` stdout and every `.ll` of `ignite build --target llvm` compared byte for byte |
| B2 (2), LLVM keeps unreferenced constants | steering option 1, the smaller correct one: on-demand constant lowering in `ignite-llvm` (19.13, D2), owned by U1 with `crates/ignite-llvm/src/` and the `CODEGEN_REVISION` step (19.16); new row 39; row 5 checks the `.ll` files; row 38 allows only the removal of unreferenced constants in the `.ll` goldens; 19.2 row on reachability corrected. Prefix exclusion rejected in D2 |
| S1 (3), `Symbols` not in `ignite-eval` | clause resolutions recorded in `TypedProgram` (19.6, 19.13 `ignite-types`, D3); `ignite_eval::Module` and `Loaded::modules` unchanged; `ignite-check` dev-dependency of `ignite-eval` for row 18 (19.13) |
| S2 (4), fixture harnesses | rows 9 and 11 name the fixture-std module lists of both crates and a `valid_clauses` per crate; silent fixture renamed `claim_ok_complete`, checked by row 41 outside the filter (19.12); U1 and U2 scopes list them (19.16) |
| S3 (5), sibling catalogue, two `Spec`s | 19.4: "nothing else" only under `std/rfc/`, a program module may declare one `Spec`; E092 fixtures run with `--std claims_std` (19.12), whose `rfc.ignite` row 40 keeps equal to `std/rfc.ignite` |
| S4 (6), E091 position in another file | 19.4 and 19.7: catalogue consistency runs on the constants of the checked file only; row 25 covers the std catalogues |
| S5 (7), overlapping E091 cases | 19.4: numbered precedence (ordinal under 1, then name mismatch, then listing, then `rfc`); a duplicate is E004 or the name mismatch; row 17 lists the ordinal case |
| S6 (8), pass with a test not run | 19.10 verdict rule 5: any failure is `fail`, any test not run is `inconclusive`, `pass` needs at least one test and all passed; row 33 adds the not-run case through the verdict function |
| S7 (9), exit statuses | 19.10 exit table and run-failure definition; 19.8 exit statuses of `rfc import` and of `rfc check` without SOURCE; D8; mapped in `main.rs` (19.13); row 37 adds the signal case |
| S8 (10), clause-only imports | D2 restated: a catalogue reaches generated code through a name the program imports or an expression uses; rows 5 and 12 assert what holds |
| S9 (11), layering sentence | 19.11: the sentence removed; `StdModule::layer` named beside `layer_of`; rule 4's text gets the prefix |
| S10 (12), prose levels | 19.4 "Level words" table and the strongest-word rule; the two excluded sentences listed; RFC 855 page-1 sentence added as `RP1_4`, later ids `RP1_5`, `RP1_6`; row 27 checks levels against the table |

Notes: 13 (`Loaded::check`), 14 (struct literals), 15 (`test_build` re-exports), 16 (19.14 item
8, `protocols.md` P2 row), 17 (19.14 item 11), 18 (19.10 step 3), 19 (row 28 quotes `R3_2_6`
in row 2), 20 (row 3 "each"), 21 (19.5 `conforms` takes names), 22 (row 30), 23 (19.10: one
space, since `, ` would collide with the header's own commas), 24 (19.14 item 13, D4), 25 (19.14 item 14, 19.18 Q4, not split per steering), 26 (row
7, U6 owns `docs/vision.md`), 27 (D5: only RFC 9112 and 9110 vendored, Q3 closed), 28 (19.17
through `tools/ignite-perf`), 29 (19.11 paragraph in `docs/layering.md`, U2).
