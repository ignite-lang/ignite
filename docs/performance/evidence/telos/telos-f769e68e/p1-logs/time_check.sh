#!/bin/zsh
# Usage: time_check.sh <label> <ignite binary> <runs>
# Times `ignite check` of the todo example and the design file, <runs> runs each,
# printing wall seconds per run with the 1-minute load average before and after.
label=$1
bin=$2
runs=${3:-5}
cd /home/timo/projects/telos || exit 1
for file in examples/todo/todo.ignite examples/terminal/terminal.design.ignite; do
  for i in $(seq 1 "$runs"); do
    load_before=$(cut -d' ' -f1 /proc/loadavg)
    start=$(date +%s.%N)
    "$bin" check "$file"
    rc=$?
    end=$(date +%s.%N)
    load_after=$(cut -d' ' -f1 /proc/loadavg)
    printf '%s %s run=%d wall=%.3f exit=%d load=%s/%s\n' "$label" "$file" "$i" "$(echo "$end - $start" | bc)" "$rc" "$load_before" "$load_after"
  done
done
echo "EXIT=0"
