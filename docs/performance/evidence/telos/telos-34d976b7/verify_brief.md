# Verification brief (shared by verifiers V1 to V5)

A Haiku reviewer wrote a findings file about the Telos compiler (Rust). Your job is to check each
finding and give it a verdict. Read `/home/timo/projects/telos/AGENTS.md` and the reviewer brief
`/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/review_brief.md` for context.

## Where to work

- The live repository `/home/timo/projects/telos` is being edited by other agents. Treat it as
  read-only: no edits, no builds, no `cargo` there.
- A source snapshot taken at 11:53 is at
  `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/snap-v/`. Reproduce there.
  You may add scratch input files under `snap-v/verify-<your id>/` only; do not edit the
  snapshot's sources (other verifiers share it).
- Build with `ulimit -c 0; CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
  CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-v`.
  The CLI binary is `target-v/debug/telos` (`telos check FILE`, `telos build FILE --target
  rust|llvm --out DIR`). Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`. Run a command that
  may hang under `timeout 20`.
- Before any build, check `df -h /`; do not build under 20G free.
- No files in /tmp or /dev/shm. No heredocs feeding stdin readers and no `python3 -c`: write
  scripts and inputs with the Write tool and run them by path.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any
  other personal token of Timo's.
- Kill any process you start before you finish, by PID.

## Verdicts

For each finding:
- CONFIRMED: you reproduced it (command and observed output), or traced it end to end in code
  and say which lines.
- REFUTED: the code does not behave as claimed; give the line that shows it.
- PARTIAL: real but the severity, scenario or location is wrong; give the corrected version.
- UNCLEAR: you could not settle it; say what would.

Prefer reproduction over reading. A pathological-input crash counts as reproduced only with the
input size that triggers it.

## Output

Write `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/verified-<area>.md`: one
row per finding with verdict, corrected severity, evidence (command and output or `file:line`),
and a one-line fix direction. Add findings of your own only if you hit them while reproducing,
marked NEW. Final message at most 10 lines: counts per verdict and the confirmed findings,
most severe first, one line each.
