# Code review brief (shared by reviewers R1 to R5)

Repository: /home/timo/projects/telos, a compiler for the Telos language, written in Rust.
Read `AGENTS.md` first (layout, crate roles), then `docs/slice.md` sections 1 to 8 for the
language contract. Telos sources live in `std/` and `examples/`.

## Rules

- READ-ONLY on the repository. Do not edit, create, format or delete any file under
  /home/timo/projects/telos. Do not run `cargo fmt`, `task`, `cargo build` or `cargo test`:
  other agents are editing and building the tree right now, and files may change while you
  read. The only file you write is your findings file named in your prompt.
- Allowed commands: `cat`, `sed -n`, `grep`, `rg`, `find`, `wc`, `git` is not available
  (no commits exist). No network.
- Do not write files to /tmp or /dev/shm.
- No heredocs feeding stdin readers and no `python3 -c`.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or
  any other personal token of Timo's.

## What counts as a finding

A finding is a concrete defect or cost, not a style preference:
- correctness: an input that produces a wrong result, a panic, a hang or a wrong diagnostic;
- performance: an algorithm or allocation pattern with a cost that grows worse than needed,
  with the input size that makes it hurt;
- security: memory unsafety, unbounded resource use reachable from a network peer or from
  source text, command or path injection, undefined behaviour in `unsafe` code.

Each finding must carry:
1. `file:line` (as of your read) plus a 1 to 3 line quote of the code, so it survives edits;
2. a failure scenario: the concrete input or state, and the wrong outcome;
3. severity: high (crash, UB, wrong code generated, DoS from the network), medium (wrong
   diagnostic, quadratic cost on realistic input), low (edge case, minor cost);
4. confidence: confirmed (you traced the path end to end in the code) or suspected.

Do not report what you did not trace. Do not pad: 0 findings is a valid result. At most 12
findings, most severe first.

## Output

Write your findings to the file named in your prompt, as a markdown table or numbered list
following the shape above. Your final message: at most 10 lines, the count per severity and
the path of your findings file.
