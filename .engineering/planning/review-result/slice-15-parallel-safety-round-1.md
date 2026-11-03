---
format: aep.planning-md/3
id: review-result:slice-15-parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, slice 15 round 1
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
needs-revision

**Findings**

story:slice-15-conformance-command — it and story:slice-15-rfc-importer both add a subcommand in `crates/ignite/src/main.rs` (the clap `Commands` enum, main.rs:47) and a `mod` line in `crates/ignite/tests/main.rs`. Both also need a `mod` declaration in `crates/ignite/src/lib.rs`, which is inferred. Neither body mentions the other and no `depends_on` edge joins them. `aep plan artifact waves` puts the importer in wave 2 and the command in wave 4 only by depth, so the command can start as soon as claim-model lands. Two ways out: an edge from conformance-command to rfc-importer recording the shared file as the reason, or giving each subcommand its own dispatch file. — .engineering/planning/story/slice-15-conformance-command.md:43-45, .engineering/planning/story/slice-15-rfc-importer.md:41-42

story:slice-15-frontend — its scope says "AST fields added" (`FnDecl`, `TestDecl`, `RecordDecl`, `Field`, `ProtocolDecl`), but it does not name `crates/ignite-rustgen/tests/units.rs`. That file builds these AST structs as full literals without `..`: `FnDecl` at units.rs:218, `RecordDecl` at :277, `Field` at :281, `ConstDecl` at :1019, `ProtocolDecl` at :1148. Adding a field breaks that file's compilation (cited). It is a module of `crates/ignite-rustgen/tests/main.rs`, the same test binary as the `golden.rs` that story:rustgen-type-parameter-shadowing edits (cited). Nothing orders the two, so the proposed wave 1 can run beside the frontend. Fix: name `units.rs` in the frontend scope and either add an edge from the frontend to the rustgen story or split the AST change from the clauses. — .engineering/planning/story/slice-15-frontend.md:39-41, crates/ignite-rustgen/tests/units.rs:218

story:rustgen-type-parameter-shadowing — its acceptance has an "or the checker reports the shadowing" branch. That branch lands in `crates/ignite-check` (the frontend's surface). It also needs a new diagnostic row in `docs/slice.md` section 8 and a free code, which story:slice-15-contract is editing and allocating (E090 to E092) in the same wave. The body names neither collision. Fix: commit to the rustgen-only branch, or state the dependency and code allocation, or order it against the contract and the frontend. — .engineering/planning/story/rustgen-type-parameter-shadowing.md:38-41, .engineering/planning/story/slice-15-contract.md (Scope: section 8 rows)

story:slice-15-claim-model — its scope names `crates/ignite/src/lib.rs`, where `Checked::check` (lib.rs:1115) calls the evaluator. The importer (`mod rfc` under `src/rfc/`) and the conformance command (`src/conformance.rs`) must also declare their modules in that file. That is inferred, not cited. There is no edge between claim-model and the importer. The body does not say so. The fix is an edge from claim-model to the importer (or the reverse) naming `lib.rs`, or each story naming its module-declaration file. — .engineering/planning/story/slice-15-claim-model.md:42, .engineering/planning/story/slice-15-rfc-importer.md:41

story:slice-15-rfc-importer — its acceptance reads vocabulary that `story:slice-15-frontend` creates in `std/rfc/rfc.ignite`: the `Level` variant `Must`, and the `Spec` and `Requirement` field names the golden module must use. It depends only on the contract, and the contract's "decides at least" list does not fix the vocabulary. Both stories sit in wave 2. Missing: an edge from the importer to the frontend, or the contract fixing the vocabulary text the importer targets. — .engineering/planning/story/slice-15-rfc-importer.md:52, .engineering/planning/story/slice-15-frontend.md:34,46

**What I read.** 8 artifacts plus the epic, using `aep plan artifact show` on each, `aep plan artifact waves --status draft` and `aep plan artifact graph`. In the repo I read `crates/ignite/src/{main,lib}.rs`, `crates/ignite/tests/main.rs`, `crates/ignite-syntax/src/ast.rs`, `crates/ignite-rustgen/tests/{main,units}.rs`, `crates/ignite-check/tests/layering.rs` and `docs/protocols.md` 3.1 to 3.2 and section 7.

**What I could not establish.**
- Surfaces: all 8 are cited in the bodies, none are only inferred and none are unplaceable. The module-declaration files in `lib.rs` are my own inference, as noted above.
- Acceptance reads: I traced 9 to a producer in the set, and 8 have a `depends_on` edge. The missing one is the importer reading the frontend's vocabulary (the fifth finding).
- Collisions the CLI flagged that I did not turn into findings:
  - conformance-command against protocol-claims on `tests/conformance.rs` is ordered by their edge.
  - The conformance and importer collisions on `main.rs` and `tests/main.rs` are the first finding.
- Out of my lane:
  - Every new `std.rfc.*` module needs a row in `LAYERS` in `crates/ignite-check/tests/layering.rs` and in `docs/layering.md`; the layering test fails without one. No catalogues, frontend or contract body names `docs/layering.md` or the table. This is an acceptance and scope gap for the design or scope critics. It is ordered by the existing edges, so it does not set my verdict.
  - Whether the claim-model scope's "or the check driver" is precise enough is a design question.

```findings
- file: .engineering/planning/story/slice-15-conformance-command.md
  line: 43
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it and story:slice-15-rfc-importer both add a subcommand in crates/ignite/src/main.rs and a mod line in crates/ignite/tests/main.rs (cited, both bodies; a mod declaration in crates/ignite/src/lib.rs is inferred), neither body says so and no depends_on edge orders them; waves separates them only by depth. Either an edge recording the shared file as its reason, or splitting the dispatch so they no longer share it"
- file: .engineering/planning/story/slice-15-frontend.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "adding AST fields breaks crates/ignite-rustgen/tests/units.rs, which builds FnDecl (l.218), RecordDecl (l.277), Field (l.281), ConstDecl (l.1019) and ProtocolDecl (l.1148) as full literals; the scope omits that file, and it shares a test binary with golden.rs edited by story:rustgen-type-parameter-shadowing with no edge between them (cited)"
- file: .engineering/planning/story/rustgen-type-parameter-shadowing.md
  line: 41
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the 'or the checker reports the shadowing' branch lands in crates/ignite-check and needs a docs/slice.md section 8 row and a free code, colliding with story:slice-15-frontend and story:slice-15-contract (the contract allocates E090 to E092 and edits section 8) in the proposed wave 1; the body names neither collision (cited acceptance, collision surfaces inferred from the alternative branch)"
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 42
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it edits crates/ignite/src/lib.rs (Checked::check, lib.rs:1115, cited) while story:slice-15-rfc-importer and story:slice-15-conformance-command must declare their new modules there (inferred); there is no edge between claim-model and the importer and the body does not say so"
- file: .engineering/planning/story/slice-15-rfc-importer.md
  line: 52
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reads the Level variant Must and the Spec and Requirement field names that story:slice-15-frontend creates in std/rfc/rfc.ignite, it depends only on the contract, and the contract's decides-at-least list does not fix the vocabulary, so both sit in wave 2 with no depends_on edge from reader to producer"
```
