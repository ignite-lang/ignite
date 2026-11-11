# Binary size, v0.15.0 to HEAD aa1defb2 (wave/10-tls): attribution and cuts

Read-only diagnosis of the w10-tls worktree, 2026-11-11. All experiments ran in an exported copy under
`<scratch>/diag/size/`. No timing, benchmark or
load run was made: every statement about speed below is "unmeasured". Pairs are written todo / terminal
unless a table says otherwise.

## 0. Summary

**Cause (verified, sections 2 and 3).** Stripped growth of row 3.9 built here: LLVM +188,928 / +168,448,
Rust +134,608 / +98,424. On LLVM terminal the program objects are byte-identical in both builds; of the
+164,260 loaded bytes ignite_io is +120,947, the parts of std it newly pulls in +16,480
(`available_parallelism` with its cgroup reader, `RandomState`/getrandom, B-tree), linker-made tables
+11,208, ignite_rt +6,373, nix +4,077, corosensei +1,585. By code family (.text): `sched.rs` +40,454,
timer `BTreeMap` +13,251, `wait.rs` +12,321, `available_parallelism` +10,632, reactor `HashMap` +5,811,
`EpollReactor` +5,649, `PollReactor` +5,057, run queue +4,078, thread mode kept beside readiness +3,958,
corosensei +3,085. Todo carries 32 KB more that is not the scheduler (Ignite `std.protocol.http`,
channels and address builtins in ignite_rt). The four dev rows grew by debug info: +2.9 MB on LLVM
(ignite_io's own DWARF +1,288,379, ring's C objects compiled with `-g3` +869,546, strings +488,522),
+1.2 to 1.4 MB on Rust (unoptimised generic instances of the scheduler, their symbols and DWARF).

**The three claims.** Scheduler = 96 % of release growth: consistent, about 90 % (terminal) and 80 % (todo)
at HEAD, not re-measured per commit. Dev LLVM growth = rustls/ring/webpki debug info: only partly; after the
pending `debug = false` it is 53 % ignite_io and 37 % ring C objects, which no profile setting removes.
Wave 9's LTO and opt-level 1: verified; both hide growth (LTO would have taken 120 KB / 83 KB off
v0.15.0 too).

**Ranked changes** (stripped bytes of row 3.9, todo / terminal; details and all twelve rows in section 5):

| # | change | LLVM | Rust | state |
|---|---|---:|---:|---|
| 1 | zlib-compress debug sections in dev links | dev rows only: 11.7 / 11.0 MB to 4.2 / 3.5 MB | 9.3 / 7.8 MB to 4.6 / 3.5 MB | measured, backtraces keep file:line |
| 2 | `-Wl,-z,pack-relative-relocs` (DT_RELR) | -36,792 / -36,792 | -22,104 / -19,480 | measured, needs glibc 2.36+ |
| 3 | five code cuts in ignite-io (`D/cut-prototypes.diff`) | -49,200 / -49,200 | -50,024 / -50,472 | prototypes link, untested |
| 4 | ignite-io `opt-level = "s"`, `codegen-units = 1`; ignite-rt `"s"` | -57,360 / -53,264 | -43,248 / -37,672 | measured, speed unmeasured |
| 5 | `-Wl,--discard-all` on the LLVM release link | 3.2 release cells only: -143,272 / -130,912 | | measured |
| 6 | runtime archive on a std built without `backtrace` | -129,456 / -137,648 | | measured, unstable cargo flag |
| 7 | generated ignite-std at `opt-level = "s"` | | -38,824 / -20,672 | measured, speed unmeasured |

**Result.** After 1 to 5 and 7 (all on stable Rust) ten of the twelve rows are at or under v0.15.0:
rust/terminal 489,216 (518,960), rust/todo 736,448 (755,896), all eight cells of row 3.2 (the two LLVM
release cells are under the v0.15.0 build without change 5 and under the recorded bar only with it).
**The two LLVM rows of 3.9 do not reach v0.15.0 on stable Rust while the scheduler is linked**: the floor
measured is 591,792 for terminal (+29,192, 1490 permille against 1417) and 796,592 for todo (+45,576,
763 against 720). Only change 6 puts them under: 454,144 (1144) and 667,136 (639).

## 1. Setup, references and how to read the numbers

Paths: `S` = `<scratch>`, `D` = `S/diag/size`,
`T` = the w10-tls worktree. Nothing in `T` was written. Every script named below is in `D/scripts/`;
every table file in `D/out/`; every experiment's cargo line and output in `D/log/`.

Toolchain of this machine (`rustc --version`, `clang --version`, `ld --version`, `ld.lld --version`,
`ldd --version`): rustc and cargo 1.99.1, clang 23.1.1, GNU ld 2.47, LLD 23.1.1, glibc 2.44.

Binaries compared: `S/bins/base15/bin/{llvm,rust}/{dev,release}/{todod,terminald}` (v0.15.0) and
`S/bins/head/...` (HEAD aa1defb2 plus the uncommitted `debug = false` of five TLS crates), built by
`S/bins/build.sh` the way `ignite-perf quality` builds row 3.2. Row 3.9's LLVM cells are a different
build (release archive, `tools/ignite-perf/src/bench.rs`); I built them with `D/scripts/export.sh` +
`D/scripts/llvm39.sh` into `D/bin39/base-llvm` and `D/bin39/head-llvm`. Row 3.9's Rust cells are the
stripped 3.2 Rust release binaries.

### 1.1 Three facts about the references (verified)

1. **The 3.2 "v0.15.0" numbers in the task are bars, not one build.** They are the `bars` of
   `quality/baseline.json` at the tag (`git show v0.15.0:quality/baseline.json`): ratcheted minima with a
   5 % tolerance. v0.15.0 itself measured above them (`quality/trend.md` line 14:
   llvm/terminald/release 1,512,792 against the bar 1,468,648). This report gives both references:
   **bar15** (the task's number) and **built15** (v0.15.0 built here, today's toolchain).
2. **Local stripped sizes reproduce the EC2 run of row 3.9 to within one page.**
   `quality/bench.jsonl` (`size_bytes`, run of 4520b8cf): base-llvm 750,968 / 558,456, head-llvm
   939,896 / 726,904, base-rust 753,936 / 517,992, head-rust 886,896 / 614,512 (todo / terminal),
   native-rust 1,043,456 / 397,152. Here (`strip` on a copy, `stat -c %s`): 751,016 / 562,600,
   939,944 / 731,048, 755,896 / 518,960, 890,504 / 617,384. Permille in this report =
   local stripped bytes / native-rust bytes of that run; the v0.15.0 values in local bytes are the targets.
3. **This machine's `~/.cargo/config.toml` changes every dev build**: `[profile.dev] debug =
   "line-tables-only"`, `rustc-wrapper = "sccache"`, and `-C link-arg=-fuse-ld=lld` for
   `x86_64-unknown-linux-gnu`. So all dev numbers here (and the 3.2 bars, which were recorded on this
   machine) are with line tables only, and Rust-target binaries are linked by lld. A machine without that
   file produces larger dev binaries; `debug = "line-tables-only"` overrides passed with `--config` are
   no-ops here (measured: `d-io-rt-lines`, `d-all-lines`, `rd-io-lines` are byte-identical to as-is).

### 1.2 The twelve rows, measured here

| row | bar15 / v0.15.0 EC2 | built15 here | head here | head - built15 |
|---|---:|---:|---:|---:|
| 3.9 llvm/terminal, stripped | 558,456 (1406) | 562,600 | 731,048 (1841) | +168,448 |
| 3.9 llvm/todo, stripped | 750,968 (720) | 751,016 | 939,944 (901) | +188,928 |
| 3.9 rust/terminal, stripped | 517,992 (1304) | 518,960 | 617,384 (1555) | +98,424 |
| 3.9 rust/todo, stripped | 753,936 (723) | 755,896 | 890,504 (853) | +134,608 |
| 3.2 llvm/terminald/dev | 7,563,720 | 8,020,464 | 10,956,904 | +2,936,440 |
| 3.2 llvm/terminald/release | 1,468,648 | 1,527,032 | 1,633,520 | +106,488 |
| 3.2 llvm/todod/dev | 8,230,560 | 8,725,248 | 11,681,616 | +2,956,368 |
| 3.2 llvm/todod/release | 2,042,752 | 2,121,840 | 2,238,048 | +116,208 |
| 3.2 rust/terminald/dev | 6,223,520 | 6,606,088 | 7,780,216 | +1,174,128 |
| 3.2 rust/terminald/release | 681,368 | 706,504 | 764,880 | +58,376 |
| 3.2 rust/todod/dev | 7,405,864 | 7,923,968 | 9,313,888 | +1,389,920 |
| 3.2 rust/todod/release | 1,011,216 | 1,011,360 | 1,079,336 | +67,976 |

Commands: `stat -c %s` on the binaries above; stripped = `cp f x; strip x; stat -c %s x`
(`D/scripts/sections.py`, `D/scripts/attr.py`); permille = stripped / 397,152 (terminal) or / 1,043,456 (todo).
The dev head numbers include the uncommitted `debug = false`; the recorded head bars without it are
15,560,616 and 16,272,232.

## 2. Where the growth is (verified, head - built15)

### 2.1 By section

`readelf -SW` per binary, grouped (`python3 D/scripts/sections.py LABEL=BASE:HEAD ...`,
outputs `D/out/sections-*.md`; full `size -A` tables per pair in `D/out/<pair>.md`).

Row 3.9, LLVM target (release archive):

| | llvm/todo | llvm/terminal |
|---|---:|---:|
| file base / head | 944,680 / 1,204,048 | 733,992 / 967,648 |
| stripped base / head | 751,016 / 939,944 | 562,600 / 731,048 |
| stripped delta | +188,928 | +168,448 |
| .text | +141,248 | +118,704 |
| .eh_frame + .eh_frame_hdr | +20,664 | +17,392 |
| .gcc_except_table | +9,820 | +8,152 |
| .rela.dyn | +8,976 | +8,448 |
| .data.rel.ro + .got + .data | +6,672 | +6,368 |
| .rodata | +5,124 | +5,196 |
| .symtab + .strtab (gone after strip) | +70,435 | +65,209 |

Row 3.2, LLVM target (dev-profile runner archive for dev and release cells):

| | llvm/todod/dev | llvm/terminald/dev | llvm/todod/release | llvm/terminald/release |
|---|---:|---:|---:|---:|
| file delta | +2,956,368 | +2,936,440 | +116,208 | +106,488 |
| stripped base / head | 1,398,184 / 1,529,800 | 947,624 / 1,062,856 | 1,316,264 / 1,443,784 | 935,336 / 1,050,568 |
| stripped delta | +131,616 | +115,232 | +127,520 | +115,232 |
| .debug_* | +2,833,615 | +2,829,789 | 0 | 0 |
| .text | +70,880 | +66,880 | +66,896 | +66,896 |
| .rodata | +29,363 | +23,225 | +29,371 | +23,233 |
| .gcc_except_table | +11,536 | +9,108 | +11,536 | +9,108 |
| .eh_frame + .eh_frame_hdr | +9,376 | +7,440 | +8,944 | +7,440 |
| .data.rel.ro + .got + .data | +3,280 | +5,160 | +3,280 | +5,160 |
| .rela.dyn | +1,080 | +3,696 | +1,080 | +3,696 |
| .symtab + .strtab | -9,032 | -8,747 | -11,316 | -8,747 |

Row 3.2 Rust target (release stripped = row 3.9 Rust):

| | rust/todod/dev | rust/terminald/dev | rust/todod/release | rust/terminald/release |
|---|---:|---:|---:|---:|
| file delta | +1,389,920 | +1,174,128 | +67,976 | +58,376 |
| stripped base / head | 1,258,048 / 1,717,472 | 720,288 / 1,113,552 | 755,896 / 890,504 | 518,960 / 617,384 |
| stripped delta | +459,424 | +393,264 | +134,608 | +98,424 |
| .text | +303,667 | +256,771 | +180,863 | +127,006 |
| .eh_frame + .eh_frame_hdr | +68,964 | +60,032 | -12,740 | -1,964 |
| .rela.dyn | +46,056 | +40,344 | -17,640 | -12,936 |
| .gcc_except_table | +11,404 | +6,932 | -11,084 | -12,584 |
| .data.rel.ro + .got + .data | +21,104 | +18,928 | -6,160 | -3,784 |
| .rodata | +7,864 | +9,888 | +1,126 | +2,470 |
| .debug_* | +557,021 | +463,936 | 0 | 0 |
| .symtab + .strtab | +373,473 | +316,931 | -66,632 | -40,045 |

The negative rows of the Rust release cells are wave 9's `lto = true, codegen-units = 1`; see 3.3.

### 2.2 By crate, loaded sections, from the linker maps (LLVM, row 3.9)

`D/scripts/llvm39.sh` links with `-Wl,-Map`; `python3 D/scripts/mapattr.py BASE.map HEAD.map LABEL out`
sums every input section under the archive member it comes from (`D/out/llvm39-*.map.md`).
The program objects of terminald are identical in base and head: the growth there is 100 % runtime.

| owner | terminald: .text | terminald: all file sections | todod: .text | todod: all file sections |
|---|---:|---:|---:|---:|
| ignite_io | +95,929 | +120,947 | +96,962 | +122,548 |
| std (pulled in by ignite_io) | +13,732 | +16,480 | +13,732 | +16,480 |
| linker-made (.rela.dyn, .eh_frame_hdr, .got) | | +11,208 | | +12,224 |
| ignite_rt | +4,756 | +6,373 | +15,537 | +19,951 |
| program objects (Ignite std library and the example) | 0 | +451 | +10,431 | +12,085 |
| nix (epoll, eventfd) | +835 | +4,077 | +835 | +4,533 |
| alignment fill | +1,812 | +1,817 | +2,111 | +2,128 |
| corosensei | +938 | +1,585 | +938 | +1,585 |
| core, hashbrown, gimli, rustc_demangle | +702 | +1,322 | +702 | +970 |
| **sum** | +118,704 | +164,260 | +141,248 | +192,504 |

ignite_io's +120,947 on terminald is .text +95,929 (39,685 to 135,614), .eh_frame +13,712,
.gcc_except_table +6,732, .data.rel.ro +2,312, .rodata +1,610, .data +492, .tdata +160.

### 2.3 By code family, .text symbols

`nm --size-sort -S --demangle` per binary, diffed by demangled name; the instantiating crate is read
from the v0-mangled name; ordered rules `FEATURES` in `D/scripts/attr.py` name the families
(`python3 D/scripts/attr.py BASE HEAD LABEL out`; per-symbol lists in `D/out/<pair>.symbols.tsv`;
combined with `D/scripts/combine.py`, `D/out/features-text.md`). Bytes of .text, head - built15:

| family | llvm 3.9 terminal | llvm 3.9 todo | rust release terminald | rust release todod | llvm 3.2 release (both) | rust dev terminald | rust dev todod |
|---|---:|---:|---:|---:|---:|---:|---:|
| `sched.rs`: worker loop, spawn, stacks, park | +40,454 | +40,454 | +40,717 | +48,583 | +31,852 | +61,883 | +61,883 |
| timers `BTreeMap<(Instant, u64), Waiter>` | +13,251 | +13,251 | +7,335 | +7,335 | +21,266 | +43,498 | +43,498 |
| `wait.rs`: Waiter, WaitList, Lock, park_in | +12,321 | +12,384 | +7,514 | +8,234 | +11,598 / +14,153 | +18,695 | +23,389 |
| `worker_limit`: `std::thread::available_parallelism` and its cgroup reader | +10,632 | +10,632 | +12,982 | +12,982 | +10,623 | +11,135 | +11,135 |
| `EpollReactor` | +5,649 | +5,649 | +7,327 | +7,327 | +6,070 | +13,749 | +13,749 |
| reactor table `HashMap<u64, Entry>`, RandomState, getrandom | +5,811 | +5,811 | +3,757 | +3,757 | +6,959 | +12,840 | +12,840 |
| `PollReactor` (waits off a worker) | +5,057 | +5,057 | +3,531 | +3,531 | +4,479 | +8,936 | +8,936 |
| run queue `VecDeque<Waiter>` | +4,078 | +4,078 | +2,518 | +2,518 | +6,284 | +10,427 | +10,540 |
| thread mode kept beside readiness (`task::spawn_thread`, `task::work`) | +3,958 | +3,958 | +3,657 | +6,688 | +2,599 | +6,168 | +6,168 |
| corosensei | +3,085 | +3,085 | +3,385 | +3,385 | +3,519 | +12,760 | +12,760 |
| `ignite_rt_tcp_read`, `ignite_io::net` (readiness call sites) | +4,934 | +5,586 | -1,664 | +4,133 | | +9,662 | +10,024 |
| `channel.rs` and `ignite_rt_channel_*` | +639 | +2,851 | | +6,397 | +918 / +16,096 | | +14,763 |
| Ignite `std.protocol.http` (not the scheduler) | | +7,270 | | +30,478 | 0 / +7,270 | | +15,835 |
| LTO inlining into `ignite_io::run`, telnet, todo functions | | | +39,013 | +61,799 | | | |
| **all .text symbols** | +116,892 | +139,137 | +127,408 | +182,247 | +68,589 / +69,221 | +244,583 | +289,618 |

The first ten families exist only with the scheduler: 104,296 of 116,892 B (89 %) of the .text symbol
growth of LLVM terminal, 104,359 of 139,137 B (75 %) of LLVM todo.

Largest single symbols (LLVM 3.9 terminal, head): `ignite_io::sched::work` 15,185;
`std::thread::functions::available_parallelism` 6,282; `ignite_io::sched::spawn` 6,269; B-tree
`Handle<..., (Instant, u64), Waiter>` insert 3,820 + 1,966; `ignite_rt_tcp_read` 4,303 (1,402 before);
`std::sys::thread::unix::cgroups::find_mountpoint` 2,694; `Readiness::wait` 2,472;
`RawTable<(u64, Entry)>::reserve_rehash` 2,263; `PollReactor::wait` 2,110;
`std::sys::random::linux::getrandom` 1,643; `census::HEAP` 4,096 of .bss (not in the file).

### 2.4 Debug info of the dev LLVM cells (llvm/terminald/dev, +2,936,440)

`D/scripts/relink32.sh` relinks the provided objects with `-Wl,-Map` (sizes equal the provided binaries);
`python3 D/scripts/mapattr.py ... --debug` (`D/out/llvm-terminald-dev.map.md`, todod alike):

| owner | loaded | .debug_* | note |
|---|---:|---:|---|
| ignite_io | +58,333 | +1,288,379 | 808,154 to 2,096,533; .debug_info +729,268, .debug_ranges +361,552, .debug_line +203,491 |
| ring, C and assembly objects | 0 | +869,546 | none in base; code fully dropped by `--gc-sections`, DWARF kept |
| merged `.debug_str` | | +488,522 | before merging: ignite_io 364,507 to 1,021,755; ring C 0 to 479,957 |
| ignite_rt | +25,169 | +86,241 | |
| nix | +9,759 | +71,690 | |
| std, corosensei, getrandom, hashbrown | +21,403 | +18,498 | |

Section deltas: .debug_info +965,592, .debug_str +488,522, .debug_loclists +464,849 (ring's
`curve25519.o` alone 366,155), .debug_line +427,786, .debug_ranges +389,984, .debug_macro +33,854,
.debug_rnglists +23,373, .debug_abbrev +21,188, .debug_aranges +7,728.

Absolute, head: 9,307,522 B of the 10,956,904 are debug sections (merged strings 2,975,496, ignite_io
2,096,533, std 1,572,590, ring C 869,546, ignite_rt 819,425, core 361,742, nix 322,928).

Why ring's DWARF cannot be switched off by a profile: `ring-0.17.14/build.rs` line 580 passes `-g3` to
the C compiler unconditionally. Measured: `--config 'profile.dev.package."*".debug=false'`
(`D/log/dev-d-deps-nodebug.log`) leaves ring's 869,546 B in place and takes 384,872 B (nix, getrandom,
corosensei, strings) off llvm/terminald/dev (10,572,176).

### 2.5 The Rust target's dev cells (rust/terminald/dev, +1,174,128)

Generated projects are their own workspace: cargo's default dev profile, opt-level 0, 256 codegen units,
so every generic the scheduler uses is instantiated unoptimised in the binary. .text +256,771
(ignite_io 68,025 to 257,084; std +21,539; core +19,203; nix +5,839; corosensei +4,676), .symtab +
.strtab +316,931 (long v0 names of those instances), .debug_* +463,936, .eh_frame + hdr +60,032,
.rela.dyn +40,344. The timer B-tree alone is 43,498 B of .text at opt-level 0.

## 3. The three claims

### 3.1 "The readiness scheduler (1a8aba78, PR 131) is 96 % of the release growth"

Consistent with the attribution, not re-measured commit by commit (that needs a compiler and an archive
per merge; not done under this machine's load).

- Verified: on LLVM terminal the program objects are byte-identical, so the compiler and the Ignite
  library contribute 0; 89 % of the .text symbol growth is code that exists only with the scheduler
  (table 2.3), and most of the rest sits in the functions that now park on readiness
  (`ignite_rt_tcp_read` +2,901, `ignite_io::net` +2,033). The census is 543 B of .text plus 4,096 B of .bss.
- Verified: on LLVM todo the share is lower at HEAD, because wave 10 added code todo links:
  Ignite `std.protocol.http` +12,085 B in the program objects, channels and socket-address builtins in
  ignite_rt (+19,951 B). Scheduler families are 75 % of its .text symbol growth.
- Inference: 96 % was true at PR 131's merges; at HEAD it is about 90 % for terminal and about 80 % for todo.

### 3.2 "Dev LLVM growth beyond that is debug info of rustls, ring and webpki"

Partly. With the uncommitted `debug = false` for rustls, rustls-webpki, rustls-pki-types, ring and
untrusted, no Rust TLS compile unit is left and no TLS code is linked, and the binary is 10,956,904
(recorded without it: 15,560,616). What remains over built15 (+2,936,440) is **not** mostly TLS:
ignite_io's own DWARF +1,288,379 (plus about 657,000 B of strings before merging), ring's C and assembly
DWARF +869,546 (plus 479,957 B of strings), ignite_rt +86,241, nix +71,690. So after the pending change
about 53 % of the dev growth is ignite_io debug info and about 37 % is ring C objects (inference: the merged string growth split by the pre-merge proportions).

### 3.3 "Wave 9 added fat LTO for Rust-target release and opt-level 1 for ignite-io in dev"

Verified (a693a90f: `RELEASE_PROFILE` in `crates/ignite-rustgen/src/lib.rs`; `[profile.dev.package.ignite-io]
opt-level = 1` in `Cargo.toml`), and both hide growth rather than remove it:

| Rust target, release, stripped (`D/scripts/rustexp.sh`, `D/log/rbatch1.log`) | todod | terminald |
|---|---:|---:|
| v0.15.0 as generated (no LTO) | 755,944 | 518,976 |
| v0.15.0 with `lto = true, codegen-units = 1` | 636,440 | 435,744 |
| head as generated (LTO) | 890,648 | 617,512 |
| head with `lto = false, codegen-units = 16` | 953,632 | 671,040 |

Like for like the Rust target grew by 197,688 B / 152,064 B without LTO and 254,208 B / 181,768 B
with LTO on both sides; LTO takes 63 KB / 54 KB off head but would have taken 120 KB / 83 KB off
v0.15.0.

| LLVM cells of row 3.2 (`D/scripts/devexp.sh`, `D/log/dbatch.log`) | todod/dev | todod/release | terminald/dev | terminald/release |
|---|---:|---:|---:|---:|
| head as is (ignite-io at opt-level 1 in dev) | 11,681,768 | 2,238,120 | 10,957,048 | 1,633,584 |
| `--config profile.dev.package.ignite-io.opt-level=0` | 11,479,344 | 2,951,048 | 10,677,552 | 2,265,344 |

opt-level 1 takes 712,928 B / 631,760 B off the two release cells (they link the dev archive) and adds
202,424 B / 279,496 B of DWARF to the dev cells. It has no effect on row 3.9, whose LLVM cells link the
release archive.

## 4. Cuts, each with its measurement

All experiments ran in copies (`D/exp/head` = `git archive HEAD` + the uncommitted Cargo.toml,
`D/exp/cut` = the same plus the prototypes, `D/exp/base` = v0.15.0), each with its own target directory.
Sizes: file / stripped; "t+d" is `size` (Berkeley) text + data, the page-free measure, since stripped
files move in 4,096 B steps.

### 4.1 Code cuts in ignite-io (prototypes, type-checked and linked, not run)

The prototypes are `D/cut-prototypes.diff` (430 lines, against `crates/ignite-io` of HEAD; written by
`D/scripts/patch_cut.py`). Each sits behind a cargo feature so it can be measured alone. They compile
(`cargo check -p ignite-io --features cut_...`, `D/log/check-cut.log`: exit 0) and link; **no test was
run on them** (no test, timing or load run was in scope), so they are size evidence, not finished code.

| cut | what changes | what is lost | LLVM 3.9 terminald t+d | stripped todod / terminald |
|---|---|---|---:|---:|
| none (HEAD, `cut-none`) | | | 717,629 | 939,944 / 731,048 |
| `cut_parallelism` | `sched::worker_limit` counts the CPUs of `sched_getaffinity(0)` (nix feature `sched`) instead of `std::thread::available_parallelism` | the cgroup CPU quota is no longer read: a container limited by quota, not by cpuset, gets one worker per visible CPU | -13,360 | 927,656 / 710,568 |
| `cut_btree` | worker timers in a `BinaryHeap` with lazy deletion (`fire_timers` already skips an entry whose key is not the slot's armed key) instead of `BTreeMap<(Instant, u64), Waiter>` | a cancelled timer stays in the heap until its deadline | -12,276 | 931,752 / 710,568 |
| `cut_threads` | the IO mode is always readiness: `IGNITE_IO=threads`, `task::spawn_thread` and the thread-mode worker become unreachable | the escape hatch to thread-per-task | -11,216 | 931,712 / 714,624 |
| `cut_hashmap` | `EpollReactor.entries` is a slab (`Vec` of generation + entry, free list; token = generation << 32 \| index + 1) instead of `Mutex<HashMap<u64, Entry>>` | nothing functional; drops hashbrown, `RandomState` and `getrandom` | -8,527 | 935,832 / 714,648 |
| `cut_poll` | a wait off a worker calls `poll(2)` on the descriptor directly (`PollReactor::poll_one`) instead of register / wait / deregister on the `PollReactor` | nothing functional for a single descriptor | -5,692 | 935,848 / 718,760 |
| all five (`cut-all`) | | | -52,147 | 890,744 / 677,752 |

Command per row: `TREE=exp/cut D/scripts/exp.sh cut-<name> --features ignite-io/cut_<name>` =
`cargo build -q --release --locked --offline -p ignite-rt --target-dir target/bench-rt --features ...`,
then `ignite build <example> --target llvm --release --runtime <archive>` (`D/scripts/llvm39.sh`),
`strip` on a copy; log `D/log/batch2.log`; table `D/scripts/tab.sh bin39 "" ...`.

The same five on the Rust target (`SUBSTRATE=exp/cut D/scripts/rustexp2.sh r-cut-all head --features ...`,
`D/log/rbatch2.log`): release stripped 890,648 / 617,512 to 840,144 / 566,192 (-50,504 / -51,320);
dev (`rd-cut-all`, `D/log/rdbatch2.log`): 9,323,944 / 7,790,320 to 9,021,592 / 7,484,816.

Not prototyped, estimated from table 2.3 (inference): run queue `VecDeque<Waiter>` as a plain `Vec`
about -2.5 KB; one boxed thread entry instead of three `spawn_unchecked` instances about -1.4 KB each;
census 0.5 KB. Together under 8 KB.

### 4.2 Profile of the runtime crates (no source change)

LLVM target, row 3.9 (`D/scripts/exp.sh NAME --config ...` on `exp/head`, `D/log/batch1.log`, `batch3.log`, `batch6.log`):

| `--config profile.release...` | todod file / stripped | terminald file / stripped |
|---|---:|---:|
| as is (ignite-rt `codegen-units = 1`, everything opt-level 3) | 1,204,048 / 939,944 | 967,648 / 731,048 |
| `package.ignite-io.opt-level=2` | 1,201,680 / 935,848 | 961,000 / 722,856 |
| `package.ignite-io.opt-level="z"` | 1,305,608 / 927,648 | 1,043,984 / 706,464 |
| `package.ignite-io.codegen-units=1` | 1,147,968 / 919,456 | 909,512 / 706,464 |
| `package.ignite-io.opt-level="s"` | 1,222,752 / 919,456 | 967,968 / 698,272 |
| ignite-io and ignite-rt `opt-level="s"` | 1,262,928 / 907,168 | 984,160 / 690,080 |
| ignite-io `"s"` + `codegen-units=1`, ignite-rt `"s"` (`cut-none-best`) | 1,221,896 / 890,784 | 943,152 / 673,696 |
| `lto=true` | 1,204,032 / 939,944 | 967,608 / 731,048 |

`lto = true` does nothing: ignite-rt is `crate-type = ["staticlib", "rlib"]`, and the sizes are those of
the build without it. `"z"` is worse than `"s"`. The unstripped file grows with `"s"` (more functions stay
out of line, so more symbols) while the stripped file shrinks.

Rust target, release (`D/scripts/rustexp.sh`, `D/log/rbatch1.log`), stripped todod / terminald:
head 890,648 / 617,512; `package.ignite-io.opt-level="s"` 835,408 / 569,152 (-55,240 / -48,360);
whole program `opt-level="s"` 706,848 / 529,104 (changes the user's code, not proposed).

Speed is not measured here (no timing run allowed on this machine): `"s"` on the scheduler's hot path
needs the dedicated-host run before it is taken.

### 4.3 Link flags (no rebuild of anything)

LLVM target, relinks of the same objects with the driver's link line plus the flag
(`D/scripts/linkexp.sh NAME SRC ARCHIVE flags`, `D/scripts/relink32.sh`; a relink without a flag
reproduces the original size exactly):

| flag | 3.9 todod / terminald, stripped | 3.2 release todod / terminald, file | 3.2 dev todod / terminald, file |
|---|---:|---:|---:|
| none | 939,944 / 731,048 | 2,238,048 / 1,633,520 | 11,681,616 / 10,956,904 |
| `-Wl,-z,pack-relative-relocs` (DT_RELR) | 903,152 / 694,256 | 2,156,200 / 1,572,152 | 11,599,768 / 10,895,536 |
| `-Wl,-z,noseparate-code` | 939,944 / 722,856 | | |
| RELR + noseparate-code | 903,152 / 686,064 | 2,152,104 / 1,563,960 | 11,591,576 / 10,891,440 |
| `-Wl,--discard-all` (drop local symbols) | no effect after strip | 2,089,752 / 1,497,464 | |
| RELR + `--discard-all` | | 2,007,904 / 1,436,104 | |
| `-Wl,--compress-debug-sections=zlib` | | | 4,246,904 / 3,518,848 |
| `-Wl,--compress-debug-sections=zstd` | | | 4,191,120 / 3,464,040 |
| `-fuse-ld=lld -Wl,--icf=all` + RELR | 941,784 / 734,152 (larger) | | |
| `-no-pie` | 903,080 / 694,184 (not proposed: no ASLR) | | |

- DT_RELR packs the relative relocations: `.rela.dyn` 39,288 B to 2,520 B plus 416 B of `.relr.dyn` on LLVM terminald (`size -A`). The binary
  then needs glibc 2.36 or newer at run time (it carries the version need `GLIBC_ABI_DT_RELR`; this
  host has 2.44; a tiny RELR binary ran here, `D/out/bt/bt-relr`).
- zlib-compressed debug sections keep `file:line` in a Rust panic backtrace, zstd does not: verified
  with a three-line program (`D/out/bt`: `rustc -C debuginfo=line-tables-only -C link-arg=-Wl,--compress-debug-sections=zlib`,
  `RUST_BACKTRACE=1`: frames carry `at ./main.rs:3:16`; with `zstd` they carry no location).
- `--discard-all` removes the names of local functions from `.symtab`; a release binary has no DWARF
  (`--strip-debug`), so a panic backtrace would show fewer function names. It only matters for the
  unstripped 3.2 release cells.

Rust target (`--config 'target.x86_64-unknown-linux-gnu.rustflags=["-C","link-arg=..."]'`; in the
product this is a `cargo::rustc-link-arg-bins=` line in a generated `build.rs`, since a manifest cannot
carry link arguments): RELR on release head (`r-head-relr`, `D/log/rbatch2.log`) 1,079,512 / 765,016 to 1,057,408 / 745,536, stripped 890,648 / 617,512 to 868,544 / 598,032; `--compress-debug-sections=zlib` on dev
(`rd-z`, `D/log/rdbatch3.log`) 9,317,296 / 7,783,600 to 4,635,888 / 3,499,064.

### 4.4 Dev profile and debug info

LLVM cells of row 3.2 (dev runner archive; `D/scripts/devexp.sh`, `D/log/dbatch.log`, `dbatch2.log`):

| dev archive | todod/dev | todod/release | terminald/dev | terminald/release |
|---|---:|---:|---:|---:|
| as is | 11,681,768 | 2,238,120 | 10,957,048 | 1,633,584 |
| ignite-io `opt-level=0` (before wave 9) | 11,479,344 | 2,951,048 | 10,677,552 | 2,265,344 |
| all dependencies `debug=false` | 11,292,840 | 2,239,808 | 10,572,176 | 1,635,240 |
| five cuts (`d-cut-all`) | 11,465,176 | 2,169,488 | 10,749,224 | 1,569,616 |
| five cuts, ignite-io `opt-level="s"` (`d-cut-all-s`) | 10,882,416 | 2,178,976 | 10,155,832 | 1,572,568 |
| five cuts + RELR | 11,387,424 | 2,091,736 | 10,687,856 | 1,508,248 |
| five cuts + RELR + `--discard-all` | | 1,948,464 | | 1,377,336 |
| five cuts + RELR + zlib debug sections | 4,061,376 | | 3,359,384 | |
| five cuts, `"s"`, RELR + zlib debug sections | 3,969,928 | | 3,261,432 | |

Without compression the dev LLVM cells cannot get near the bars by profile alone: the debug info is
9.3 MB of 11.0 MB, and the parts that grew are ignite_io's own line tables (it is the crate the program
is made of) and ring's `-g3` C objects, which no profile setting reaches (2.4).

Rust-target dev cells (`PROFILE=dev D/scripts/rustexp2.sh`, `D/log/rdbatch*.log`), todod / terminald:

| generated project's dev build | file | stripped |
|---|---:|---:|
| v0.15.0 (`rd-base`) | 7,922,840 / 6,605,736 | 1,258,168 / 720,312 |
| head (`rd-head`) | 9,317,296 / 7,783,600 | 1,717,672 / 1,113,736 |
| `profile.dev.package.ignite-io.opt-level=1` | 9,174,736 / 7,654,928 | 1,362,552 / 803,632 |
| `profile.dev.package."*".opt-level="s"` (ignite-io and its dependencies; not the program, not ignite-std) | 8,583,680 / 7,066,224 | 1,280,872 / 732,768 |
| the same + ignite-io `codegen-units=1` | 8,236,368 / 6,793,336 | 1,236,096 / 700,712 |
| `"*".opt-level="s"` + `"*".debug=false` | 6,803,120 / 5,336,896 | 1,280,864 / 732,792 |
| five cuts | 9,021,592 / 7,484,816 | 1,619,400 / 1,013,552 |
| five cuts + `"*".opt-level="s"` | 8,412,680 / 6,893,576 | 1,236,288 / 687,056 |
| zlib debug sections only | 4,635,888 / 3,499,064 | 1,717,672 / 1,113,736 |
| `"*".opt-level="s"` + zlib | 3,861,656 / 2,788,136 | 1,280,872 / 732,768 |
| five cuts + `"*".opt-level="s"` + zlib + RELR | 3,664,560 / 2,630,520 | 1,142,816 / 635,840 |

`"*"` in the generated manifest covers ignite-io, nix, corosensei, libc and the small crates below them;
the program and ignite-std are workspace members and keep opt-level 0 and their debug info.

### 4.5 The standard library's symbolizer (the largest single block, in v0.15.0 and head alike)

Every binary carries std's backtrace symbolizer (gimli, addr2line, miniz_oxide, rustc_demangle and
`std::backtrace_rs`): 171,262 B of .text symbols in LLVM terminal at v0.15.0, 171,295 B at head, plus
33,494 B of `core::slice::sort` and `OnceCell` instances that are almost only its own
(`D/out/llvm39-terminald.symbols.tsv`, names matching gimli, addr2line, miniz_oxide, rustc_demangle,
backtrace). That is more than the whole scheduler regression, and stable Rust has no switch for it: the
default panic hook references it. `panic = "abort"` does not remove it and is not available anyway
(`report::catch`, `contain::entry` use `catch_unwind`).

Measured with the standard library rebuilt from `rust-src` without its `backtrace` feature
(`D/scripts/bsexp.sh`: `RUSTC_BOOTSTRAP=1 cargo build --release -p ignite-rt -Zbuild-std=std,panic_unwind
-Zbuild-std-features=panic-unwind --target x86_64-unknown-linux-gnu`, `D/log/batch4.log`, `batch5.log`):

| LLVM 3.9, stripped | todod | terminald |
|---|---:|---:|
| head, shipped std | 939,944 | 731,048 |
| std rebuilt, default features (control: the rebuilt std is larger than the shipped one) | 1,021,872 | 808,880 |
| std rebuilt without `backtrace` (`bs-nobt`) | 806,376 | 589,288 |
| the same + RELR | 773,680 | 556,592 |
| std rebuilt without `backtrace`, every crate `opt-level="s"` (`bs-nobt-s`) | 761,328 | 536,048 |
| without `backtrace` + five cuts (`bs-cut-all`) | 748,984 | 527,800 |
| the same + RELR | 716,288 | 499,200 |
| without `backtrace` + five cuts + ignite-io and ignite-rt `"s"` (`bs-cut-all-s`) | 708,024 | 490,936 |
| the same + RELR | 667,136 | 454,144 |

Cost: an unstable cargo flag (`RUSTC_BOOTSTRAP=1` on the stable toolchain, or nightly) and the
`rust-src` component wherever the runtime archive is built; a panic inside the runtime prints its
message and location but an unsymbolized backtrace. It applies to the LLVM target only, whose runtime
the project builds itself; the Rust target builds in the user's cargo, where this is not an option.

## 5. Ranked changes and what each row becomes

Ranked by bytes per unit of risk. Every number is measured unless marked "est.". Rows 3.9 are stripped
bytes with the unstripped file in brackets; the changes are cumulative left to right.

### 5.1 The changes

1. **zlib-compress the debug sections of dev links**: `-Wl,--compress-debug-sections=zlib` in the dev
   link line of `crates/ignite/src/llvm_build.rs` (beside `LINK_FLAGS`), and for the Rust target a
   generated `build.rs` printing `cargo::rustc-link-arg-bins=-Wl,--compress-debug-sections=zlib`.
   Alone it takes the four dev rows to 4,246,904 / 3,518,848 (LLVM todod / terminald) and 4,635,888 /
   3,499,064 (Rust), all far under bar15. Panic backtraces keep `file:line` (4.3). It does not remove
   what grew; it shrinks all debug info, old and new, by about 75 %.
2. **DT_RELR**: `-Wl,-z,pack-relative-relocs` in `LINK_FLAGS` (LLVM) and the generated `build.rs`
   (Rust). Stripped: LLVM -36,792 on both programs, Rust -22,104 / -19,480. Needs glibc 2.36+ at run time;
   the musl `--static` path needs its own check (not measured).
3. **Five code cuts in ignite-io** (4.1): stripped LLVM -49,200 / -53,296, Rust -50,504 / -51,320 (without 2; on top of 2: LLVM -49,200 /
   -49,200, Rust -50,024 / -50,472).
   Prototypes compile and link; they are untested. `cut_parallelism` drops the cgroup quota,
   `cut_threads` drops `IGNITE_IO=threads`: both are product decisions.
4. **Size profile for the runtime crates**: workspace `[profile.release.package.ignite-io] opt-level = "s",
   codegen-units = 1` and `[profile.release.package.ignite-rt] opt-level = "s"`; in the generated Rust
   project `[profile.release.package.ignite-io] opt-level = "s"`. Stripped, on top of 2 and 3: LLVM
   -57,360 / -53,264, Rust -43,248 / -37,672. Speed unmeasured here: needs the dedicated-host run.
5. **`-Wl,--discard-all` on the LLVM release link**: only for the unstripped 3.2 LLVM release cells against
   bar15 (-143,272 / -130,912 on top of 2 and 3). Costs local function names in release backtraces.
   Not needed if built15 (or the 5 % tolerance of bar15) is the reference.
6. **Runtime archive with a standard library built without `backtrace`** (4.5): the only measured way to
   put both LLVM rows of 3.9 under v0.15.0 with the scheduler linked. Stripped, on top of 2, 3 and 4:
   -129,456 / -137,648. Unstable cargo flag; LLVM target only.
7. **Size profile for the generated standard library, Rust target**: `[profile.release.package.ignite-std]
   opt-level = "s"` in the generated manifest (`crates/ignite-rustgen/src/lib.rs`, beside
   `RELEASE_PROFILE`). Stripped, on top of 2, 3 and 4: -38,824 / -20,672. This is what puts rust/todo
   under v0.15.0 (`r-cut-all-io-std-s-relr`). Measured alternative: whole-program `opt-level = 2` with
   ignite-io at `"s"`: 746,672 / 502,048 (`r-cut-all-io-s-o2-relr`), also under. Speed unmeasured.

### 5.2 Row 3.9, stripped bytes [file bytes]

| row | head | + 2 RELR | + 3 five cuts | + 4 size profile | + 6 (LLVM) or 7 (Rust) | v0.15.0 here (EC2) |
|---|---:|---:|---:|---:|---:|---:|
| llvm/todo | 939,944 [1,204,048] | 903,152 [1,167,256] | 853,952 [1,106,008] | 796,592 [1,107,728] | 667,136 [981,072] | 751,016 (750,968) |
| llvm/terminal | 731,048 [967,648] | 694,256 [930,856] | 645,056 [869,552] | 591,792 [841,136] | 454,144 [708,240] | 562,600 (558,456) |
| rust/todo | 890,648 [1,079,512] | 868,544 [1,057,408] | 818,520 [999,056] | 775,272 [960,472] | 736,448 [924,008] | 755,896 (753,936) |
| rust/terminal | 617,512 [765,016] | 598,032 [745,536] | 547,560 [686,808] | 509,888 [652,800] | 489,216 [632,160] | 518,960 (517,992) |

As permille of plain Rust (397,152 terminal, 1,043,456 todo; v0.15.0 on EC2: 1406, 720, 1304, 723):

| row | head | after 2 + 3 + 4 | after 6 (LLVM) or 7 (Rust) | v0.15.0 built here | v0.15.0 on EC2 |
|---|---:|---:|---:|---:|---:|
| llvm/terminal | 1841 | 1490 | 1144 | 1417 | 1406 |
| llvm/todo | 901 | 763 | 639 | 720 | 720 |
| rust/terminal | 1555 | 1284 | 1232 | 1307 | 1304 |
| rust/todo | 854 | 743 | 706 | 724 | 723 |

Local stripped files are up to one page larger than the EC2 ones (1.1), so compare against the
"built here" column; in bytes the comparison is exact.

Commands: LLVM `cut-all-relr`, `cut-all-best-relr`, `bs-cut-all-s-relr` (`D/scripts/exp.sh`, `bsexp.sh`,
`linkexp.sh`; `D/log/batch2.log`, `batch5.log`, `batch6.log`); Rust `r-head-relr`, `r-cut-all-relr`,
`r-cut-all-io-s-relr`, `r-cut-all-io-std-s-relr` (`D/scripts/rustexp2.sh`; `D/log/rbatch2.log`,
`rbatch3.log`, `rbatch4.log`). The Rust "head" here
is the generated project rebuilt in the scratch (890,648 / 617,512; the provided binaries are 890,504 /
617,384).

### 5.3 Row 3.2, file bytes

| row | head | after 1 (dev) or 2 (release) | + 3 five cuts (+ 2) | + 4 `"s"` | + 5 `--discard-all` | bar15 | built15 |
|---|---:|---:|---:|---:|---:|---:|---:|
| llvm/todod/dev | 11,681,616 | 4,246,904 | 4,061,376 | 3,969,928 | | 8,230,560 | 8,725,248 |
| llvm/terminald/dev | 10,956,904 | 3,518,848 | 3,359,384 | 3,261,432 | | 7,563,720 | 8,020,464 |
| llvm/todod/release | 2,238,048 | 2,156,200 | 2,091,736 | 2,097,128 | 1,948,464 | 2,042,752 | 2,121,840 |
| llvm/terminald/release | 1,633,520 | 1,572,152 | 1,508,248 | 1,511,208 | 1,377,336 | 1,468,648 | 1,527,032 |
| rust/todod/dev | 9,317,296 | 4,635,888 | | 3,664,560 | | 7,405,864 | 7,923,968 |
| rust/terminald/dev | 7,783,600 | 3,499,064 | | 2,630,520 | | 6,223,520 | 6,606,088 |
| rust/todod/release | 1,079,512 | 1,057,408 | 999,056 | 960,472 | | 1,011,216 | 1,011,360 |
| rust/terminald/release | 765,016 | 745,536 | 686,808 | 652,800 | | 681,368 | 706,504 |

Notes: the LLVM cells link the dev-profile archive, so "4" there is `[profile.dev.package.ignite-io]
opt-level = "s"` in place of 1 (`d-cut-all-s`): 0.59 MB less DWARF, 3 to 5 KB more symbol table; leaving
it at 1 is equally good once 1 is in. The `--discard-all` column is on the opt-level 1 archive
(`d-cut-all-relr-x`; with `"s"`: 1,955,232 / 1,381,696). The Rust dev column "4" is
`[profile.dev.package."*"] opt-level = "s"` in the generated manifest plus cuts, zlib and RELR
(`rd-cut-all-star-s-z-relr`). With 7 the two Rust release cells are 924,008 / 632,160. Commands: `D/scripts/relink32.sh`, `devexp.sh` (`D/log/dbatch2.log`),
`rustexp2.sh` (`D/log/rdbatch3.log`, `rdbatch4.log`, `rbatch2.log`).

### 5.4 Does the sum reach v0.15.0? Row by row

| row | with 1 to 5 (stable Rust) | reaches | with 6 (LLVM) or 7 (Rust) as well |
|---|---|---|---|
| 3.9 llvm/terminal | 591,792 = 1490 | **no**: +29,192 over 562,600 | 454,144 = 1144: yes |
| 3.9 llvm/todo | 796,592 = 763 | **no**: +45,576 over 751,016 | 667,136 = 639: yes |
| 3.9 rust/terminal | 509,888 = 1284 | yes (-9,072) | 489,216 = 1232 |
| 3.9 rust/todo | 775,272 = 743 | **no**: +19,376 over 755,896 | 736,448 = 706: yes (-19,448) |
| 3.2 llvm/todod/dev | 3,969,928 | yes | |
| 3.2 llvm/terminald/dev | 3,261,432 | yes | |
| 3.2 llvm/todod/release | 2,091,736; 1,948,464 with 5 | under built15 without 5; under bar15 only with 5 | |
| 3.2 llvm/terminald/release | 1,508,248; 1,377,336 with 5 | under built15 without 5; under bar15 only with 5 | |
| 3.2 rust/todod/dev | 3,664,560 | yes | |
| 3.2 rust/terminald/dev | 2,630,520 | yes | |
| 3.2 rust/todod/release | 960,472 | yes | |
| 3.2 rust/terminald/release | 652,800 | yes | |

## 6. What cannot be reached, and the floor

**The two LLVM rows of 3.9 cannot reach v0.15.0 on stable Rust while the scheduler is linked.** With
every change measured here (RELR, the five cuts, the size profile) LLVM terminal stops at 591,792 B
(1490 permille against 1417 for v0.15.0 built here, 1406 on EC2) and LLVM todo at 796,592 B (763 against 720). `-Wl,-z,noseparate-code`
takes one or two more pages (583,600 / 792,496, `cut-all-best-relr-nosep`); the cuts not prototyped are
under 8 KB (est.). The floor on stable is therefore about 575,000 B (1450) for terminal and 785,000 B
(752) for todo (inference from those two numbers).

What is left of the scheduler in LLVM terminal at that point, against v0.15.0
(`D/out/residual-best-terminald.md`, `python3 D/scripts/attr.py bin39/base-llvm/terminald
bin39/cut-all-best-relr/terminald ...`): .text +19,104, .eh_frame + .eh_frame_hdr +15,752,
.gcc_except_table +5,932, .rodata +4,204, .data.rel.ro +3,888, .got +2,104, against .rela.dyn -27,920
from RELR. The unwind tables are now as large as the code: opt-level `"s"` keeps about 340 more functions
out of line and each needs a frame entry. v0.15.0 with RELR alone would be 538,096 B
(`base-relr`), so the scheduler that remains costs about 54 KB there, and no smaller scheduler was found
by removing parts around it; closing the last 29 KB means rewriting `sched.rs` / `wait.rs` for size or
taking change 6.

For LLVM todo 32 KB of the growth is not the scheduler at all (Ignite `std.protocol.http` +12,085,
channels and address builtins in ignite_rt +19,951), so it needs change 6 or a smaller HTTP library even
with a free scheduler.

With change 6 both LLVM rows are far under v0.15.0 (1144 and 639 permille), and even without the size
profile (`bs-cut-all-relr`: 499,200 / 716,288) or without the cuts and the profile for terminal
(`bs-nobt-relr`: 556,592; todo 773,680 is then still over).

**No Rust row needs the scheduler removed.** rust/terminal is under v0.15.0 after 2, 3 and 4 (509,888
against 518,960). rust/todo is not (775,272 against 755,896): what is left over v0.15.0 there is .text
+93,315 (`D/out/residual-rust-todod.md`: `ignite_io::run` +31,588 from LTO inlining, the generated HTTP
library in ignite_std +30,214, the example's own handlers about +34 KB, `sched.rs` +23,460), while every
other loaded section is smaller than in v0.15.0. Change 7 closes it at 736,448 (706 against 724).
Whole-program `opt-level = "s"` on head with no code cut gives 706,848 / 529,104 without RELR
(`r-head-s`), so about 684,700 / 509,600 with it (est.): the Rust rows can be reached by profile and
link flag alone, at an unmeasured cost in speed.

The eight rows of 3.2 all reach built15; the two LLVM release cells reach bar15 only with
`--discard-all` (or stripping the release link), because bar15 is 3 to 4 % under what v0.15.0 itself
measures. The four dev rows are decided by debug-section compression, not by code: without it the LLVM
dev cells stay near 10.1 MB (`d-cut-all-s-relr`: 10,800,576 / 10,094,464) against bars of 8.2 / 7.6 MB,
since ignite_io's and ring's debug info cannot be configured away (2.4).

## 7. Verified and inferred

Verified by measurement here: every size, section, crate and symbol number in sections 1 to 6 with the
command named beside it; the reproduction of the EC2 stripped sizes; the three reference facts of 1.1;
ring's `-g3`; the no-op of `lto = true` on the staticlib; backtraces under zlib and zstd compression;
that the five prototypes compile and link.

Inferred, not measured:
- the 96 % of PR 131 at its own merges (3.1); the 53 % / 37 % split of the dev growth (3.2);
- the floor numbers of section 6 beyond the measured 583,600 / 792,496;
- the bytes of the cuts that were not prototyped (4.1);
- that zlib-compressed sections are acceptable to every tool the project uses on dev binaries (gdb,
  lldb, perf and binutils read them; only the Rust backtrace was tested);
- any effect on speed of changes 3, 4 and 7: no timing was allowed on this machine;
- behaviour of the prototypes: no test was run on them;
- DT_RELR on the musl `--static` link and on targets other than x86_64 glibc.

Not done: per-commit builds between v0.15.0 and HEAD; the driver's ThinLTO path (`ignite build --release`
without `--runtime`), which row 3.9 does not measure because `bench.rs` passes `--runtime`.

## 8. Files

- `D/report.md` (this file), `D/cut-prototypes.diff` (the five cuts against `crates/ignite-io` of HEAD).
- `D/out/`: per pair `<pair>.md` (sections, crates, families, top symbols), `<pair>.symbols.tsv`,
  `<pair>.map.md`, `sections-*.md`, `features-text.md`, `crates-text.md`, `residual-*.md`.
- `D/log/`: one log per experiment with its cargo line; `batch*.log`, `rbatch*.log`, `dbatch*.log`,
  `rdbatch*.log` hold the size lines.
- `D/scripts/`: `export.sh`, `attr.py`, `mapattr.py`, `sections.py`, `combine.py`, `tab.sh`,
  `clangmap.sh`, `llvm39.sh`, `exp.sh`, `bsexp.sh`, `linkexp.sh`, `relink32.sh`, `devexp.sh`,
  `rustexp.sh`, `rustexp2.sh`, `patch_cut.py`.
- `D/bin39/<variant>/`, `D/bin32/<variant>/`: the linked binaries, stripped copies and linker maps of every
  variant. The experiment target directories and the runtime archives were removed.
