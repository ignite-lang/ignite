# Full-scope format-check prototype

These are diagnostic results on frozen `d8a9cb74050a3547eabbf99817f802b9d3e5503f`,
before the production format-check wrapper was added. They do not establish 12c acceptance.
The external prototype and original serial command used the same source, environment and
installed formatter. The three timing pairs include Python startup, metadata discovery,
partitioning and every CargoFmt subprocess. Median wall time was 0.5281 s serial and
0.2347 s grouped; the original recovered 11b serial command took 0.4543 s separately.

`target-coverage.json` and the two `*-targets.stdout` files show the same 20 target roots
and editions, exactly once, across all 10 workspace packages. Each grouped command still
uses CargoFmt for target discovery and recursive formatting. Nothing uses a Rust-file allowlist.
`timing-summary.json` retains the commands, loads and raw times. `prototype.py` is the exact
external script measured; its verbose output enabled the root comparison.

`fixture-source.json` preserves an isolated two-package workspace with editions 2021 and
2024, root configuration, and nested library, binary and integration-test modules. Both
serial and grouped checks pass the valid fixture. Introducing `pub fn gen( )->i32{1}` in
the library nested module, or `pub fn value( )->i32{1}` in either other nested module,
fails both checks without changing source bytes. The per-case stdout files and
`nested-module-parity.json` retain the results. Its `accepted` field refers only to this
fixture check. `evidence-sha256.json` lists the copied evidence checksums.

The production implementation preserves this package-selection design, suppresses successful
output, and reports every group's failure diagnostics in stable order. It still needs its
own full validation and frozen performance batch.
