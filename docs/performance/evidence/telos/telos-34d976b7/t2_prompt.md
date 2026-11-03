You work in /home/timo/projects/telos, a Rust compiler for the Telos language. It has two targets:
- Rust: generated crates built by cargo.
- LLVM: IR built by clang and linked with `libtelos_rt.a`.

Read these first:
- `AGENTS.md`;
- `docs/slice.md` section 11, especially 11.3, 11.4, 11.5 and 11.7, which is the frontend contract T1 just landed;
- `docs/llvm.md`.

You are T2: the backends and the substrate for slice 7. That means:
- generics (11.1);
- effectful fn types (11.2), which lower like plain fn types;
- `Store<T>` (11.3);
- system resources (11.4);
- the new builtins (11.5): `List.map`, `Int.parse`, `Byte.to_int`, `Int.to_byte` and `TcpStream.set_idle_timeout`.

T1 left `unimplemented!` placeholders, listed in 11.7. Replace every one of them.

Two other agents work at the same time:
- **P** owns `crates/telos-io/**`, `crates/telos-rt/**` and `crates/telos-llvm/**` for throughput work, and is mid-change in `lower.rs`.
- **T3** owns `std/**`, `examples/todo/**` and `crates/telos/tests/todo.rs`.

Work in two phases.

Phase 1, now:
- **`crates/telos-rustgen/**`:** you own it. Do these:
  - Rust generics with `Clone + Send + Sync + 'static`;
  - effectful fn types;
  - Store method calls;
  - resource creation in every main, before the accept loop;
  - resource names in component properties;
  - the new builtins;
  - goldens.

  Rustgen matches have catch-all arms, so find every slice-7 construct that reaches one and make it real.
- **`crates/telos-io`:** add only a new file, `crates/telos-io/src/store.rs`: `Store<T: Clone + Send>` over `Arc<Mutex<BTreeMap<i64, T>>>`, plus an `AtomicI64`, with the 11.3 methods. Also add `set_idle_timeout`, in a way that does not touch P's hot paths: a new fn in a new file, or a method P agrees to. Before you change `crates/telos-io/src/lib.rs` or `net.rs` (1 to 3 lines: `mod store; pub use …`), send P a message (agent id `af25550e4eb641206`, via SendMessage) naming the exact lines.
- **Test the Rust target end to end:**
  - generic programs, Store use and system resources, with goldens and a real cargo build;
  - then, together with T3, `examples/todo/todo.telos` built with `--target rust`.

  T3 keeps `std/protocol/http.telos` as a draft until the std builds on both targets: every std module is compiled into every program, so a std file that the LLVM backend cannot lower yet would break the terminal example. Agree with T3 how to test with `--std <scratch std dir>` until then.

Phase 2, once P reports done (I will message you; until then, do not edit P's crates):
- **`crates/telos-llvm`:**
  - monomorphization per instance, with mangled names carrying the instance;
  - copy glue per instance;
  - Store glue (`copy_to_heap.<T>`, `copy_from_heap.<T>`, `free_heap.<T>`);
  - system resources in the main module;
  - the new builtins.
- **`crates/telos-rt`:** `telos_rt_store_*` over `telos_io::Store<HeapValue>`, and the idle-timeout adapter.
- Update `docs/llvm.md`.
- Then the todo suite runs on both targets: remove the `#[ignore]` in `crates/telos/tests/todo.rs` together with T3.

Acceptance:
- `task gate` is green.
- The 28 terminal cases pass on both targets.
- `crates/telos/tests/todo.rs` passes on both targets.
- `Store.update` stays atomic.
- The 16×100 POST test gives 1600 distinct ids on both targets.

Rules:
- Use `CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-j` for direct cargo calls. Prefix test runs with `ulimit -c 0;`. Release builds set `CARGO_PROFILE_RELEASE_DEBUG=0`.
- The rest of the rules are in `/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/rules.md`; read and follow them.
- Never call the GitLab API, with `curl` or anything else, using `$GITLAB_PERSONAL_TOKEN` or any other personal token of Timo's.
- A `terminald` on port 2323 belongs to Timo; leave it alone.

Report at the end of phase 1, at most 10 lines, then stop and wait for my message. Report at the end of phase 2, at most 15 lines. Both reports give test counts and anything that deviates from section 11.
