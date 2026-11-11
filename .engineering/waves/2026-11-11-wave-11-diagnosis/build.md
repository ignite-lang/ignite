# Diagnosis: rows 6.2 llvm, 6.2 rust, 6.1

Written by the coordinator from the agent's final message (the agent could not write this file).
Evidence in this directory: `scripts/`, `out/` (per-variant logs, callgrind outputs, section
tables), patches `01` to `06`, `prototype-tls-free-archive.patch`, source copies `exp*`.

## Method

No wall-clock. Evidence: bytes and counts (`stat`, `readelf -SW`, `ar t`, linker `-Map`), executed
commands (`strace -f -e execve`), instructions per process (`valgrind --tool=callgrind
--trace-children=yes`, "Ir"), one execution per tracer. Ir does not see kernel time, waiting or
parallelism.
The workstation's cargo config sets dev `debug = "line-tables-only"`, sccache and lld. The
dedicated host (`tools/ignite-perf/src/remote-user-data.sh`) has no cargo config, so its dev
profile is `debug = 2`. All archives and Rust builds below ran with `RUSTC_WRAPPER= RUSTFLAGS=
CARGO_PROFILE_DEV_DEBUG=2`. Local `line-tables-only` archives are 38,273,784 (base) and
51,733,968 B (head): a workstation build does not show the host's link input.
Toolchain here: clang 23.1.1, GNU ld 2.47, LLD 23.1.1.

## 6.2 llvm (+44.8 % on the host)

Steps, identical at base and head: driver in process writes four `.ll`; one clang probe; four
`clang -c -O0 -g`; then per main (`todod`, `health`) one `clang -o` whose linker is GNU ld
(`/usr/bin/ld ... --build-id ... --gc-sections`). The archive is linked twice per build. The link
command: `crates/ignite/src/llvm_build.rs` `fn link` (line 484), loop over `generated.mains`
(545-584); no `-fuse-ld` on dev builds.

Base -> head at the host profile: archive 56,633,500 B / 764 members -> 130,882,800 / 784; rustc
compilations for the archive 9 -> 29; loaded section bytes for `todod` 47,482,477 -> 119,165,119;
debug incl. relocations 29,913,244 (63.0 %) -> 96,137,944 (80.7 %); debug relocations 556,988 ->
1,915,714; `todod` 16,402,064 -> 44,256,840 B; compile units 444 -> 1,212 (429 TLS).
Instructions: driver 2,433.7 M -> 2,779.4 M; clang processes 1,074.9 M -> 1,140.2 M; ld `todod`
1,436.6 M -> 3,324.1 M; ld `health` 1,377.5 M -> 3,257.2 M; total 6,322.7 M -> 10,500.8 M
(+66.1 %). The two ld runs are 90 % of the added work. Inside GNU ld `sha1_process_block`
(`--build-id`) is 22.6 % at base and 27.2 % at head.
Whose bytes (loaded, all / debug): TLS crates 0 -> 49.3 MB / 45.0 MB; ring C and assembly 0 ->
2.7 / 2.2 MB; ignite_io 13.1 / 10.3 -> 27.5 / 24.9 MB; ignite_rt 12.6 / 9.6 -> 16.6 / 13.2 MB;
nix 3.7 / 2.5 -> 4.9 / 3.3 MB; std, core, alloc unchanged 15.9 / 7.1 MB.
The `tls` feature accounts for 59,426,447 B (49.9 %) of the head link input, 53,928,380 B of it
debug info. With the uncommitted five `debug = false` overrides: archive 85,124,054 B, `todod`
24,844,216 B, build 8,088.3 M Ir (127.9 % of base): not enough. Without TLS the growth is in
ignite_io: loaded debug 10.33 MB (base) -> 15.58 (66a3d5b9) -> 19.76 (head without TLS).

Variants (`todod` link, GNU ld Ir / lld Ir; base 1,436.6 M / 433.6 M):
- head 3,324.1 M / 1,029.4 M; head with `--build-id=none` 2,408.8 M
- h2 `"*"` debug=false: 2,017.6 M / 643.2 M
- h3 = h2 + `split-debuginfo="unpacked"`: 1,121.0 M / 309.0 M (debug info then points at files
  in the build directory)
- h4 no `tls` feature: 1,720.4 M; h5 = h4 + `"*"` debug=false: 1,611.7 M / 500.0 M
- h7 = patch 01: archive 50,943,314 B, loaded 39.97 MB, 343,525 relocations, `todod` 11.31 MB,
  1,208.1 M / 331.4 M; with `--build-id=none` 987.3 M
Whole build against base 6,322.7 M: patch 01 6,269.7 M (99.2 %); 01 + 04 6,175.7 M (97.7 %);
01 + 02 + 04, one run: 4,473.7 M (70.8 %), `ld.lld` executed twice, `todod` 11,573,152 B.

## 6.2 rust (+20.1 % on the host)

Generated project, fresh dir, no lock: crate compilations 12 -> 18 plus one probe; build-script
runs 2 -> 4; `Cargo.lock` packages 8 -> 22; index requests per fresh build 6 -> 20; ignite-io
source lines 8,898 -> 13,066. New: autocfg, scopeguard, corosensei and its build script
(scheduler), memoffset and its build script (nix `socket`). nix features `signal, dir, poll,
resource` -> plus `event, feature, socket`. No rustls unit is compiled (`substrate_features()` in
`crates/ignite-rustgen/src/lib.rs`). All units opt-level 0, dependencies `debuginfo=2`, path
crates incremental.
Instructions: rustc ignite_io 7,026 M -> 10,174 M; nix 5,826 M -> 7,844 M; ignite_std 3,035 M ->
3,304 M; driver 1,847 M -> 2,221 M; new units about 1,310 M; total 27,257 M -> 34,892 M. Serial
chain 23,683 M -> 29,427 M (+24.3 %).
Candidates (chain against base): dependencies `debug = false` (patch 06) 107.6 %; nix without
`socket` (patch 05) 118.4 % alone; `incremental = false` 107.0 % alone; 05 + 06 +
`incremental = false` 87.3 %; lock seeded (patch 03): `connect()` 18 -> 0, not visible in Ir.
`incremental = false` cuts codegen units from 256 to 16; its wall-clock effect on the 96-vCPU
host is open. Patch 05 adds four `unsafe` libc calls (row 3.6).

## 6.1 (+3 %)

`check` 864.1 M -> 896.8 M Ir, `plan` 869.9 M -> 902.5 M. The same 15 std modules are opened.
Growth: `module_dependencies` +15.5 M (`builtin_dependencies` scans each module's text for every
builtin name; `import_order` runs it a second time per module), `import_order` +7.6 M,
`check_module` +13.7 M. Patch 04 (`Parsed` caches its dependencies in a `OnceLock`): check
848.1 M, plan 853.9 M (98.2 % of base), `plan.json` identical.

## Patches (each applies alone to aa1defb2)

- `01-runtime-debug-profile.patch`: workspace `Cargo.toml` `"*"` debug=false, ignite-rt and
  ignite-io `line-tables-only`. Also applies over the uncommitted `Cargo.toml`.
- `02-dev-link-lld.patch`: `llvm_build.rs` `dev_lld()` and `-fuse-ld=lld` in `link()`,
  `docs/llvm.md`; `--test main -- llvm_build:: build_cache::` 12 passed, 2 ignored.
- `03-rust-seed-lock.patch`: `lib.rs` `seed_lock()`; traced build 0 `connect()`.
- `04-module-dependencies-once.patch`: lib tests 94 passed.
- `05-nix-without-socket.patch`: `net.rs` through libc; `cargo test -p ignite-io --lib -- net::`
  10 passed.
- `06-rustgen-deps-without-debug.patch`: generated manifest `[profile.dev.package."*"] debug =
  false`, unit test, `docs/slice.md`.

## Inferred, to be settled on the host

- Whether `ld.lld` sits beside the resolved clang on the host; otherwise 02 does nothing and the
  llvm margin is 01 + 04 alone (97.7 %).
- Whether the host's clang passes `--build-id` by default.
- The wall-clock effect of `incremental = false` and of the removed index resolution.
- 05 + 06 without `incremental = false`: chain about 102 % (a subtraction, not a run).
