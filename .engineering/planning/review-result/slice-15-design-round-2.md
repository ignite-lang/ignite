---
format: aep.planning-md/3
id: review-result:slice-15-design-round-2
kind: review-result
status: active
title: Design critic, slice 15 round 2
relations:
- reviews: epic:slice-15-protocols-p2
- reviews: story:slice-15-contract
- reviews: story:slice-15-frontend
- reviews: story:slice-15-rfc-importer
- reviews: story:slice-15-claim-model
- reviews: story:slice-15-catalogues
- reviews: story:slice-15-conformance-command
- reviews: story:slice-15-protocol-claims
- reviews: story:rustgen-type-parameter-shadowing
revision: 1
---
approve

What I read: 8 artifacts. I ran `aep plan artifact show` on the epic, the seven stories and `story:rustgen-type-parameter-shadowing`. I also ran `relations`, `graph`, `waves --status draft` and `validate`, which reports valid. I read the round-1 design and parallel-safety results to see which findings had to land. In the repo I read `docs/protocols.md` 3.1 to 3.3, 3.6, 3.7, 6.1 and the P2 row of section 7. I also read `crates/ignite/src/lib.rs` (module list and `Checked::check`), `crates/ignite-check/Cargo.toml`, the `LAYERS` table in `crates/ignite-check/tests/layering.rs`, and the AST struct literals under `crates/`.

Round-1 findings, each confirmed in the current bodies:
- **E092 and duplicate-id E091:** both are now in `story:slice-15-claim-model`. The frontend keeps only E091's first trigger, which is syntactic, and its Out of Scope line says so.
- **Importer to frontend:** there is now a `depends_on` edge from `story:slice-15-rfc-importer` to `story:slice-15-frontend`. The importer's acceptance now checks the golden with `ignite check`.
- **Both-target test:** the frontend acceptance now runs `ignite test` on both targets for a clause-bearing fixture.

Shape of the current set:
- **No cycle:** I walked every edge in the printed graph, including the slice 13 and 14 artifacts outside the set. Inside the set there are 11 `depends_on` edges, plus the 7 `decomposes` edges to the epic.
- **No serialising chain:** waves run 1, 2, then 3 with claim-model and importer side by side, then 4 with catalogues and conformance-command side by side, then 5. The two ordering-only edges both state their reason in the body. The shared `main.rs`, `tests/main.rs` and `lib.rs` module lines justify conformance-command to importer. The shared test binary justifies frontend to shadowing.
- **No split abstraction:** the E091 code is split across the frontend (non-literal constant, syntactic, in `ignite-check`) and claim-model (duplicate id, needs the evaluator). The seam follows the crate boundary and the contract's D3 assigns it. The importer writes the vocabulary text and the catalogues apply it, so each can be demonstrated alone.
- **No hidden dependency:** I traced 14 acceptance reads to a producer in the set. Nine are covered by a direct edge: importer to contract and to frontend, frontend to contract, claim-model to frontend, catalogues to frontend and to importer, conformance-command to claim-model, and protocol-claims to catalogues and to conformance-command. Five are covered transitively: the contract decisions D3, D4, D5, D7 and D8 read by claim-model, catalogues, conformance-command and protocol-claims, and protocol-claims reading E090 from claim-model through conformance-command. None lacks an edge.
- **No horizontal slice:** each story ends in its own checkable fixture, and the frontend fixture builds on both targets.

What I could not establish:
- Whether `ignite rfc check` reads the digest back from the `Spec` constant by walking the parsed AST or through the evaluator's literal-constant evaluation. The importer body does not say. If it uses the evaluator, the importer needs an edge to `story:slice-15-claim-model`. I left it out of the findings because the AST read is plausible and the contract's D6 can settle it.
- Whether `Model::check` runs the claim check on a `protocol` inside a std file that `ignite check` receives as the program. `story:slice-15-protocol-claims` needs that. I did not read `ignite-eval`'s system iteration, and D3 may settle it.
- Out of my lane:
  - `story:slice-15-frontend`'s acceptance fixture lists four of the six clauses and never `satisfies` (acceptance critic).
  - The edge from `story:slice-15-frontend` to `story:rustgen-type-parameter-shadowing` rests on "same test binary". `golden.rs` and `units.rs` are different files and `waves` reports no collision. I left it to the parallel-safety critic; it adds no wave depth.
  - Neither the merge to main nor the `v0.4.0` tag is in any story's acceptance (scope critic).

```findings
[]
```
