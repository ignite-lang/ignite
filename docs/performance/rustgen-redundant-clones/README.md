# Redundant clones in generated Rust (story:rustgen-redundant-clones)

2026-11-06. Before is `1bef40cd` (`main`); step 1 is `37927d6b`, step 2 is `5a6f9e1a` on
`perf/rustgen-redundant-clones`. Program: ember, the RESP2 server of the examples repository
(`ember/ember.ignite`, sha256 `6ca3ed91…af100de`, the working tree of 2026-11-06 on top of
`bcbe5a3`), `ignite build --target rust --release` with each tree's own compiler, `std/` and
`ignite-io`. Load generator: emberload of the same repository, one binary for every run (sha256
`7763a2e0…7a23138`).

## Method

`dhat.sh TREE EMBER_SOURCE EMBERLOAD OUT` (this directory): `valgrind --tool=dhat` (3.25.1,
rustc 1.99.1) over the ember tournament's allocation workload, the one its `measure.sh` runs
under memcheck: `4 2000 10` (8,000 `INCR counter`), `2 1000 10 set` (2,000 `SET` of 1,000
bytes), `FLUSHDB`, then `SIGTERM`. `sites.jq` groups dhat's program points by their first three
program frames. Two runs of the "before" binary gave the same totals to the byte and block, so
each step was run once.

## Totals

| tree | total | at t-gmax |
|---|---|---|
| before (`1bef40cd`) | 24,099,729 B in 311,307 blocks | 4,904,187 B in 12,397 blocks |
| step 1, byte string literals | 24,035,702 B in 279,293 blocks | 4,904,194 B in 12,401 blocks |
| step 2, Store results read in place | 21,294,885 B in 249,287 blocks | 4,904,194 B in 12,401 blocks |

249,287 blocks is 0.80 of the "before" count and 52 times the handwritten Rust baseline's 4,797
allocations (memcheck, same workload, examples `ember/tournament`).

## Step 1: a byte string literal built once (`37927d6b`)

`crates/ignite-rustgen/src/lib.rs`: `Generator::byte_literal` in `ref_arg` and the `Bytes.of`
arm of `static_call`. `parse_array` passes `b"\r\n"` to `bytes_find_from` once per command and
once per argument:

```rust
// before: two allocations per evaluation
bytes_find_from(data, &(vec![13, 10]).iter().map(|x| *x as u8).collect::<Vec<u8>>(), ..)
// after: none; a value position is one `vec![13u8, 10u8]`
bytes_find_from(data, { static BYTES: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![13u8, 10u8]); &*BYTES }, ..)
```

| site | before | after |
|---|---|---|
| `ember::parse_array` | 64,028 blocks, 3,132,234 B | 32,014 blocks, 3,068,206 B |
| `ember::encode` (`b"$-1\r\n"`, `b"\r\n"`) | 20,015 blocks | 20,013 blocks |
| total | 311,307 blocks | 279,293 blocks |

## Step 2: `Store.update`/`upsert` results read in place (`5a6f9e1a`)

`crates/ignite-rustgen/src/lib.rs`: `Generator::store_call_read` from the `_ = ..` and
expression-statement arms of `stmt` and the field arm of `bare`;
`crates/ignite-io/src/store.rs`: `update_read`, `upsert_read` (over `replace_with` and
`upsert_with`). The read only copies a field or nothing and never calls a store, so the
non-reentrant lock is safe.

```rust
// increment (INCR), before: a copy of the whole bucket, then its field
match db.upsert(hash(key), |found: Option<Bucket>| -> Bucket { .. }).outcome {
// after: the field, copied under the lock
match db.upsert_read(hash(key), |found: Option<Bucket>| -> Bucket { .. }, |__v| __v.outcome.clone()) {
// write (SET), before / after
let _ = db.upsert(hash(&entry.key), |bucket: Option<Bucket>| -> Bucket { .. });
let _ = db.upsert_read(hash(&entry.key), |bucket: Option<Bucket>| -> Bucket { .. }, |_| ());
```

| site | before | after |
|---|---|---|
| `Store::upsert_with` (result copies of `increment` and `write`, and the copies `f` gets) | 54,336 blocks, 3,581,723 B | 24,330 blocks, 840,906 B |
| `Store::upsert` / `upsert_read` in `write` (the copy `f` gets) | 6,008 blocks | 6,008 blocks |
| total | 279,293 blocks | 249,287 blocks |

Of the 24,330 blocks left under `upsert_with`, 24,000 are the copy of the stored bucket that
`upsert` hands `increment`'s `f` (Ignite's copy semantics: the store keeps the old value when `f`
fails) and 330 are the store's map nodes for new ids in `write`.

## Sites not changed

- `match db.get(id)` as `read` (read-only arms): ember's `lookup` (GET) is 6 blocks, 147 B under
  this load; not measurable, so not done. The arm would run under the store's lock, which needs
  an analysis that it calls no store and no effect.
- `for x in db.values()` as `read_values`: the one loop the load runs is `flush`, whose body
  calls `db.remove`; under `read_values` it would deadlock on the non-reentrant lock, so it keeps
  the copy (6,004 blocks, 2,253,908 B). `count` (DBSIZE) is not in the load.
- `Int.to_text(n).to_bytes()`: already `n.clone().to_string().into_bytes()` since PR 23 (a fresh
  text is consumed); its 8,000 blocks are the text itself.
- Pattern bindings copied at a branch tail, `held`'s `Some(found) => found.clone()` (24,000
  blocks, 598,890 B): story:moves-for-pattern-bindings.
