#!/bin/sh
# The steps of `task gate` as separate build-pool jobs, for a gate that runs longer than the
# pool's job cap as one job. check, the tests of the other crates, the library tests of ignite,
# then the test binary of crates/ignite in $SHARDS shards by top-level module, each shard its
# own `cargo test` (so its own job), and a check that every test of the binary's `--list`
# passed or was ignored; then test-ignite, path-cost and ux as one `slot` job each. Every step
# runs; the last line is `gate exit 0` or `gate exit 1`, and so is the exit code.
# `task gate-sharded [LOG=dir]` runs it and sets VALGRIND_MODULES, IGNITE_GATE_VALGRIND and SHARDS.
# Usage: tools/gate-sharded.sh [LOG_DIR]
set -u

: "${VALGRIND_MODULES:?run it as task gate-sharded}"
SHARDS=${SHARDS:-8}
LOG=${1:-${CARGO_TARGET_DIR:-target}/gate-sharded}
mkdir -p "$LOG" || exit 1
rm -f "$LOG"/shard-*.log "$LOG"/shard-*.rc "$LOG"/main-*.txt

failed=0

step() {
  name=$1
  shift
  echo "== $name $(date -u +%H:%M:%SZ)"
  "$@"
  rc=$?
  echo "$name exit $rc"
  [ "$rc" -eq 0 ] || failed=1
}

pooled() {
  if command -v slot > /dev/null 2>&1; then
    slot --timeout 30m "$@"
  else
    "$@"
  fi
}

logged() {
  file=$1
  shift
  { "$@"; echo $? > "$file.rc"; } 2>&1 | tee "$file"
  return "$(cat "$file.rc")"
}

skips=""
if [ "${IGNITE_GATE_VALGRIND:-}" = 0 ]; then
  for m in $VALGRIND_MODULES; do
    skips="$skips --skip $m::"
  done
fi

step check pooled task check
step test-other pooled cargo test --workspace --exclude ignite --no-fail-fast
step test-ignite-lib pooled cargo test -p ignite --lib
step build-main pooled cargo test -p ignite --test main --no-run

# shellcheck disable=SC2086
pooled cargo test -q -p ignite --test main -- --list $skips 2> /dev/null |
  sed -n 's/: test$//p' | sort > "$LOG/main-list.txt"
sed 's/::.*//' "$LOG/main-list.txt" | sort | uniq -c | sort -rn |
  awk -v n="$SHARDS" '{ best = 1; for (k = 2; k <= n; k++) if (load[k] < load[best]) best = k; load[best] += $1; print best, $2 }' \
    > "$LOG/main-shards.txt"
echo "main: $(wc -l < "$LOG/main-list.txt") tests, $(wc -l < "$LOG/main-shards.txt") modules in $SHARDS shards"

k=1
while [ "$k" -le "$SHARDS" ]; do
  filters=$(awk -v k="$k" '$1 == k { printf " %s::", $2 }' "$LOG/main-shards.txt")
  if [ -n "$filters" ]; then
    # shellcheck disable=SC2086
    step "test-main-$k" logged "$LOG/shard-$k.log" pooled cargo test -p ignite --test main -- $filters $skips
  fi
  k=$((k + 1))
done

cat "$LOG"/shard-*.log 2> /dev/null | grep -E '^test [^ ]* \.\.\. (ok|ignored.*)$' |
  awk '{ print $2 }' | sort -u > "$LOG/main-passed.txt"
comm -23 "$LOG/main-list.txt" "$LOG/main-passed.txt" > "$LOG/main-missing.txt"
missing=$(wc -l < "$LOG/main-missing.txt")
echo "main tests listed and neither passed nor ignored: $missing"
head -20 "$LOG/main-missing.txt"
if [ "$missing" -ne 0 ] || [ ! -s "$LOG/main-list.txt" ]; then
  failed=1
fi

step test-ignite pooled task test-ignite
if [ "${IGNITE_GATE_VALGRIND:-}" = 0 ]; then
  echo "== path-cost skipped (IGNITE_GATE_VALGRIND=0)"
else
  step path-cost pooled task path-cost
fi
step ux pooled task ux

echo "gate exit $failed"
exit "$failed"
