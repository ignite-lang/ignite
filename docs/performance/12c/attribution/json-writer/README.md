# JSON writer diagnostic

The design model contains 1,091 string values/keys (11,733 bytes) and 1,003
indentation calls. `main.rs` compares the previous writer with unchanged-span
copying and bounded indentation chunks using those actual inputs, at the normal
development optimization level. `commands.json` records compilation and input;
`results-pad.txt` retains all five diagnostic pairs, each writing 1,000 copies.
These are microbenchmarks, not CLI acceptance measurements.

The implementation preserves canonical escapes, lowercase control hex, UTF-8,
key ordering and appending to existing output. The existing inverse-parse test
now also checks every C0 control, quotes, backslashes, multibyte text, DEL,
U+2028/U+2029, both keys and values, and indentation across chunk boundaries.
The ten evaluator value tests and eleven CLI plan/golden tests pass.

The first integrated gate failed in an unrelated process probe with ETXTBSY;
its complete failed log is retained as `p4-format-json-gate.log`. The failure
was investigated and reproduced in `../../spawn-etxtbsy/`. This gate cannot be
counted as a passing candidate validation.
