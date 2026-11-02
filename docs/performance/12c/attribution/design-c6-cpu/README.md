# Actual design-check profile

One bounded Callgrind invocation of the frozen c6 CLI checked
`examples/terminal/terminal.design.ignite` successfully. Source-manifest and
binary hashes were verified before and after; this is instruction attribution,
not acceptance timing. `profile.py`, commands, summary and the complete compressed
Callgrind record are retained. The annotation heads are for navigation; regenerate
complete annotations from `design-check.callgrind.gz` when needed.

The run recorded 359,394,918 instructions. Local-name lookup accounts for about
14.10 million (3.92%); retained type-metadata cloning accounts for about 10.56
million (2.94%). Line-position work is only 1.58 million (0.44%), and the successful
design needs line tables for only about 10.7 KiB. The larger LLVM emission profile
therefore does not justify a line-table change as this frontend fix.

The private scope HashMap experiment is retained separately in
`../rejected-checker-scopes/`. It produced no reliable improvement to check and
was reverted exactly. The next proposal keeps owned typed-program snapshots but
copies only metadata that their existing public queries read; full exported
signatures and input lifetimes retain their current contracts.
