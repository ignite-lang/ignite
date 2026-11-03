#!/usr/bin/env bash
set -u
cd /home/timo/projects/telos
export CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
W=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/bench
mkdir -p "$W"
EX=examples/terminal/terminal.telos
T=./target/debug/telos
TIMEFORMAT='%R'

stamp() {
  local label=$1; shift
  local start end
  start=$(date +%s.%N)
  "$@" > "$W/$label.log" 2>&1
  local rc=$?
  end=$(date +%s.%N)
  printf '%s\t%s\texit=%s\n' "$label" "$(echo "$end - $start" | bc)" "$rc"
}

echo "== build times (s)"
stamp rust-debug-cold   $T build $EX --out "$W/rust-debug"   --target-dir "$W/rust-debug-target"
touch $EX
stamp rust-debug-warm   $T build $EX --out "$W/rust-debug"   --target-dir "$W/rust-debug-target"
stamp rust-release-cold $T build $EX --out "$W/rust-release" --target-dir "$W/rust-release-target" --release
stamp rt-debug-cold     env CARGO_TARGET_DIR="$W/rt-target" cargo build -p telos-rt
stamp rt-release-cold   env CARGO_TARGET_DIR="$W/rt-target" cargo build -p telos-rt --release
stamp llvm-debug        $T build $EX --target llvm --out "$W/llvm-debug"   --runtime "$W/rt-target/debug/libtelos_rt.a"
stamp llvm-debug-again  $T build $EX --target llvm --out "$W/llvm-debug"   --runtime "$W/rt-target/debug/libtelos_rt.a"
stamp llvm-release      $T build $EX --target llvm --out "$W/llvm-release" --runtime "$W/rt-target/release/libtelos_rt.a" --release

echo "== binaries"
declare -A BIN=(
  [rust-debug]="$W/rust-debug-target/debug"
  [rust-release]="$W/rust-release-target/release"
  [llvm-debug]="$W/llvm-debug/bin"
  [llvm-release]="$W/llvm-release/bin"
)
for k in rust-debug rust-release llvm-debug llvm-release; do
  d=${BIN[$k]}
  for exe in terminald tn; do
    f="$d/$exe"
    [ -f "$f" ] || { echo "$k $exe missing"; continue; }
    strip -o "$W/$k-$exe.stripped" "$f"
    printf '%s\t%s\tsize=%s\tstripped=%s\tshared=%s\n' "$k" "$exe" \
      "$(stat -c %s "$f")" "$(stat -c %s "$W/$k-$exe.stripped")" \
      "$(ldd "$f" | awk '{print $1}' | grep -v vdso | xargs | tr ' ' ',')"
  done
done

echo "== startup: 200 runs of terminald --port nope (exit 2), seconds total"
for k in rust-debug rust-release llvm-debug llvm-release; do
  f="${BIN[$k]}/terminald"
  start=$(date +%s.%N)
  for _ in $(seq 200); do "$f" --port nope > /dev/null 2>&1; done
  end=$(date +%s.%N)
  printf '%s\t%s\n' "$k" "$(echo "$end - $start" | bc)"
done

echo "== memory: terminald VmRSS kB idle, after 20 sessions; threads idle"
PORT=24100
for k in rust-debug rust-release llvm-debug llvm-release; do
  PORT=$((PORT + 1))
  "${BIN[$k]}/terminald" --port $PORT > /dev/null 2>&1 &
  pid=$!
  for _ in $(seq 50); do
    if (exec 3<>/dev/tcp/127.0.0.1/$PORT) 2>/dev/null; then break; fi
    timeout 0.1 tail -f /dev/null
  done
  idle=$(awk '/VmRSS/{print $2}' /proc/$pid/status)
  thr=$(awk '/Threads/{print $2}' /proc/$pid/status)
  for _ in $(seq 20); do
    printf 'echo hello\nstatus\nquit\n' | timeout 10 "${BIN[$k]}/tn" --port $PORT > /dev/null 2>&1
  done
  after=$(awk '/VmRSS/{print $2}' /proc/$pid/status)
  printf '%s\tidle=%s\tafter20=%s\tthreads=%s\n' "$k" "$idle" "$after" "$thr"
  kill $pid
  wait $pid 2>/dev/null
done
echo "== disk"
du -sh "$W/rust-debug-target" "$W/rust-release-target" "$W/rt-target" "$W/llvm-debug" "$W/llvm-release"
echo EXIT=0
