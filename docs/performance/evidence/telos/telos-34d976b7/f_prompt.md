You work in /home/timo/projects/telos, a Rust compiler for the Telos language. Read `AGENTS.md` and `docs/slice.md`, especially sections 2 to 5 and 10, which cover the slice-6 surface syntax.

You are F. Your job is small fixes that the slice-6 source rewrite surfaced. Another agent, P, works at the same time on `crates/telos-io`, `crates/telos-rt`, `crates/telos-llvm`, `crates/telos-load`, `std/protocol/telnet.telos`, `Taskfile.yml` and `docs/llvm.md`; don't edit those. You own `crates/telos-syntax/**`, `crates/telos-check/**`, `crates/telos-types/**` and `crates/telos-rustgen/**`, plus `docs/slice.md` sections 2 to 6, 8 and 10, `std/console.telos` and `examples/terminal/terminal.telos`.

Fixes:
1. `Process.exit(0)` is typed Never, but the Rust target lowers it to a `Result`, so source has to write `Process.exit(0)?`. Make the Rust lowering diverge: `telos_io::exit(code)` returns `!`. Then `Process.exit(0)` without `?` compiles on both targets. Test it with a golden or a build-and-run test that does not use `?`. P will remove the `?` from telnet.telos afterwards; tell me when your fix lands.
2. A bare variant resolves only if its enum is imported, even when the expected type already decides it. For example, `Continue` where an `Action` is expected needs `import std.console.Action`. Resolve bare variants against the expected type's enum, whether or not that enum is imported. Without an expected type, the name lookup rules stay as they are, including E005. Remove the now-unneeded `Action` import from `examples/terminal/terminal.telos` and `std/console.telos` if they no longer need it, and add fixtures.
3. Add string and byte escapes for vertical tab and form feed (`\v`, `\f`), plus a test.
4. Long `requires` lists on protocol roles have to break across lines. Check whether a newline after each `,` inside `requires [ ... ]` already works. If it does not, make it work, as inside `( )` and `[ ]`. Report which.

Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-g`. The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them. Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.

Your gate: `cargo test -p telos-syntax -p telos-check -p telos-types -p telos-rustgen -p telos` is green, `cargo clippy --workspace --all-targets -- -D warnings` is clean, and `cargo fmt --all --check` is clean. If either of the last two fails only in P's files, report it and do not fix it.

Final report, at most 8 lines: each fix with its test, and test counts.
