You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets. Read these first:
- `AGENTS.md`;
- `docs/llvm.md`;
- `docs/friction.md`;
- the coordinator's slice-9 decisions, `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/slice9_decisions.md`;
- `docs/slice.md` section 13, the slice-9 contract. N1, the frontend agent (id `a8b0cd5d6c95756e9`), wrote it and still owns it.

Your role and file ownership are given below the line. The other agents:

| agent | owns |
|---|---|
| N1 | `telos-syntax`, `telos-check`, `telos-types`, `docs/slice.md` |
| N2 | `crates/telos-rustgen/**` |
| N3 | `crates/telos-llvm/**`, `crates/telos-rt/**`, `crates/telos-io/**`, `docs/llvm.md` |

Edit only your own files. Ask N1 by SendMessage for any API you need. Tell the other backend agent about any shared decision, for example the debug-text format or the equality semantics, so both targets print and compare identically.

Implement every row of section 13 in your backend, replacing every `unimplemented!("slice 9: …")` placeholder in your crates. Rows whose backend work matters most:
- closures (9, 13), including `Store.update` with a closure, which must stay atomic;
- structural equality everywhere (7);
- debug text for assert failures and `x.debug()` (11);
- boxing of recursive enum payloads (15);
- range patterns (8);
- tuple `.0` (10);
- associated fns (17);
- Never-typed `break`/`continue` (4);
- 100,000-term chains with no stack overflow in your lowering (14).

Each row gets a golden or a build-and-run test on your target. Normal builds of programs that use no slice-9 feature stay byte-identical to the current goldens, or you explain each diff.

Gate:
- the tests of your crates are green;
- `cargo test -p telos` is green: the terminal, todo, syntax, generics, telos_test and llvm suites;
- `cargo build --workspace --all-targets` succeeds;
- clippy `-D warnings` and fmt are clean.

If the gate fails only in another agent's files, report that and don't fix it.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/<your target dir>`.
- Prefix test runs with `ulimit -c 0;`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.

Final report, at most 12 lines: rows done, test counts, golden or size changes with numbers, and deviations from section 13.

---
