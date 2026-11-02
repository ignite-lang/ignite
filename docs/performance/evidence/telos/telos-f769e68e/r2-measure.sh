#!/bin/zsh
# usage: r2-measure.sh <after-tree> <after-target-dir> <tag>
# Interleaved pairs (docs/slice.md 15.1): todod on the Rust target, dev and release, fresh
# --out and --target-dir, SCCACHE_RECACHE=1, 3 pairs; before = s12-p4-after2. Records the
# generated ignite-std line count per run. Summary lines are STEP lines for pairs.py.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
AFTER_TREE=$1
AFTER_TARGET=$2
TAG=$3
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
LOG=$S/r2-measure-$TAG.log
SUM=$LOG.summary
BUILDS=$S/r2-measure-$TAG-builds
mkdir -p $BUILDS
: >$LOG; : >$SUM
load1() { cut -d' ' -f1 /proc/loadavg; }
free_gb() { df -k / | awk 'NR==2 {print int($4 / 1048576)}'; }
echo "START $(date -Is) load=$(cat /proc/loadavg) free=$(free_gb)G after=$AFTER_TREE" | tee -a $SUM
TODO=examples/todo/todo.ignite
for pair in 1 2 3; do
  for profile in dev release; do
    rel=(); [[ $profile == release ]] && rel=(--release)
    for tree in before after; do
      if [[ $(free_gb) -lt 20 ]]; then echo "DISK under 20G free, stopping" | tee -a $SUM; echo "EXIT=95" >>$SUM; exit 95; fi
      if [[ $tree == before ]]; then
        cd $S/s12-p4-after2 || exit 97
        IGNITE=$S/target-s12-p4-after2/debug/ignite
      else
        cd $AFTER_TREE || exit 97
        IGNITE=$AFTER_TARGET/debug/ignite
      fi
      out=$BUILDS/$tree-rust-$profile-r$pair
      if [[ -e $out ]]; then echo "$out exists, refusing" | tee -a $SUM; echo "EXIT=96" >>$SUM; exit 96; fi
      lb=$(load1)
      t0=$(date +%s.%N)
      SCCACHE_RECACHE=1 $IGNITE build $TODO $rel --out $out --target-dir $out-target >>$LOG 2>&1
      rc=$?
      t1=$(date +%s.%N)
      la=$(load1)
      dt=$(echo "$t1 - $t0" | bc)
      lines=$(wc -l <$out/std/src/lib.rs 2>/dev/null)
      printf 'STEP tree=%s label=todod-rust-%s pair=%s secs=%.2f load_before=%s load_after=%s exit=%s std_lines=%s\n' "$tree" "$profile" "$pair" "$dt" "$lb" "$la" "$rc" "$lines" | tee -a $SUM
    done
  done
done
echo "EXIT=0 $(date -Is)" | tee -a $SUM
