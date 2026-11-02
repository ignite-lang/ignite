#!/bin/sh
cd /home/timo/projects/telos/crates/ignite/tests/fixtures || exit 9
H=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1-harness/debug/s12-p1-harness
for f in e120_validate_refused e121_signal_without_producer e122_unresolvable_ref e123_probe_timing e124_window_too_short e129_missing_entry; do
  out=$("$H" "$f.ignite" 2>&1 >/dev/null)
  printf '%s\n' "$out" > "$f.expected"
  echo "$f: $(wc -l < "$f.expected") lines"
done
echo "EXIT=0"
