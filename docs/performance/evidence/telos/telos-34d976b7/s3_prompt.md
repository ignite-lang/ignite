You work in /home/timo/projects/telos, a Rust compiler for the Telos language with two targets, Rust and LLVM. Read `AGENTS.md`, then `docs/slice.md` sections 2 to 5 and section 10 (10.1 and 10.2 included). Those describe the new surface syntax of slice 6, which is implemented in the frontend and both backends; `task gate` is green.

You are S3. You are the only agent working in the tree.

Job: rewrite the Telos sources in the new syntax, then make `await` an error.

1. Rewrite `std/console.telos`, `std/protocol/telnet.telos` and `examples/terminal/terminal.telos` in idiomatic slice-6 Telos. Behaviour must not change: the 28 terminal end-to-end cases run on each target (`crates/telos/tests/terminal.rs`), and they stay green on both. Use whatever section 10 offers where it reads better, for example:
   - no `await`;
   - `requires` lists only on protocol roles;
   - expression bodies and tuples in place of `type Step` and `fn step`;
   - constant patterns and or-patterns in `decode`, in place of `if`/`else if` chains;
   - `for b in bytes` in place of index loops, where no look-ahead is needed;
   - defaults (`action: Action = Continue`) and field punning;
   - bare variants where the type is known;
   - interpolation, and `+` on Text;
   - byte literals in place of magic numbers (then drop comments that only explain those numbers);
   - `return` as an expression in match arms;
   - `_ = e` in place of dummy bindings;
   - `type IoResult<T> = Result<T, IoError>`;
   - `application.respond(line)`.

   Keep the doc comments that explain behaviour. Delete comments that only restate code.
2. Acceptance:
   - `std/protocol/telnet.telos` is at most 400 lines (`wc -l`; it is 478 now) and has no line over 100 characters (`awk 'length > 100'`).
   - `examples/terminal/terminal.telos` contains no `return`, no `await` and no `Action.Continue`.
   - Report the before and after line counts of all three files.
3. Flip `await` to an error by following section 10.2: set `AWAIT_IS_ERROR` to `true` in `crates/telos-syntax/src/parser.rs`, and remove the remaining `await` uses in tests. `grep -rln await crates std examples` must then list only the parser's error path and its fixture. Update section 10.2 to say the flip is done.
4. Re-bless the goldens that change:
   - rustgen: `TELOS_BLESS=1 cargo test -p telos-rustgen`;
   - LLVM: `TELOS_BLESS=1 cargo test -p telos-llvm`;
   - the golden copies of the std sources, if the tests keep any.

   Check that the diffs are only the consequence of the source rewrite. In particular, the `terminal` golden must still compile with cargo and clang, which the tests check.
5. Run `task gate`, `task compile` and `task compile-llvm`. Run a piped session (`help`, `echo hi`, `status`, `quit`) against each target's `terminald` on a port outside 2323, and compare the transcripts byte for byte with the pre-rewrite transcript. To capture the pre-rewrite transcript first, copy the current binaries before you change anything.
6. Update `crates/telos-types/tests/terminal.rs` and the other tests that spot-check std or example source text, where the text changed.

Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-g` for direct cargo calls. `task` targets use the repository's `target/`. The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them. Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

If a feature from section 10 does not work as specified while you rewrite, fix it in the frontend or backend crate (you own the whole tree now). Add a test for the fix, and list it in your report.

Final report, at most 15 lines:
- line counts before and after;
- the acceptance results (each check with its number);
- test counts from `task gate`;
- compiler fixes you had to make;
- anything in the new syntax that read worse than expected (a short list).
