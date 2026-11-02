from pathlib import Path
import hashlib
import json
import os
import statistics
import subprocess
import time

base = Path(__file__).parent
source = Path('/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-perf12c')
live = source / 'crates/ignite/src/run/live.rs'
checker = source / 'crates/ignite-check/src/lib.rs'
(base/'after-live.rs').write_bytes(live.read_bytes())
versions = {'before': (base/'before-live.rs').read_bytes(), 'after': live.read_bytes()}
env = dict(os.environ, CARGO_TARGET_DIR='/home/timo/.cache/scratch/ignite-perf12c/target-main',
           CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='line-tables-only',
           CARGO_PROFILE_RELEASE_DEBUG='0', CARGO_NET_OFFLINE='true')
env.pop('RUSTC_BOOTSTRAP', None)
env.pop('SCCACHE_RECACHE', None)
rows = []

def run(side, phase, pair, command):
    logfile = base/f'{phase}-{pair}-{side}.log'
    load = os.getloadavg()
    start = time.monotonic()
    with logfile.open('wb') as out:
        result = subprocess.run(command, cwd=source, env=env, stdout=out, stderr=subprocess.STDOUT)
    row = dict(side=side, phase=phase, pair=pair, command=command, exit=result.returncode,
               seconds=time.monotonic()-start, load_before=load, load_after=os.getloadavg(),
               source_sha256=hashlib.sha256(live.read_bytes()).hexdigest(), diagnostic_only=True)
    rows.append(row)
    (base/'paired-rows.json').write_text(json.dumps(rows,indent=2)+'\n')
    print(json.dumps(row),flush=True)
    assert result.returncode==0

# Fill both codegen cache variants before checker-touch comparisons.
for side in versions:
    live.write_bytes(versions[side])
    run(side, 'prepare', -1, ['cargo','test','--workspace','--no-run'])
for pair in range(3):
    for side in (['before','after'] if pair%2==0 else ['after','before']):
        live.write_bytes(versions[side])
        for phase,command in [('clippy',['cargo','clippy','--workspace','--all-targets','--','-D','warnings']),
                              ('no-run',['cargo','test','--workspace','--no-run'])]:
            checker.touch()
            run(side, phase, pair, command)
live.write_bytes(versions['after'])
summary = {}
for phase in ['clippy','no-run']:
    before = [r['seconds'] for r in rows if r['phase']==phase and r['side']=='before']
    after = [r['seconds'] for r in rows if r['phase']==phase and r['side']=='after']
    summary[phase] = dict(before=statistics.median(before),after=statistics.median(after),
                          paired_ratio=statistics.median(b/a for a,b in zip(before,after)))
(base/'paired-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary),flush=True)
