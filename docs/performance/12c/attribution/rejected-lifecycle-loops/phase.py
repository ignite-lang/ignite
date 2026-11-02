from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys
import time

base = Path(__file__).parent
label = sys.argv[1]
source = Path('/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-perf12c')
root = source / 'crates/ignite/src/lib.rs'
before = hashlib.sha256(root.read_bytes()).hexdigest()
root.touch()
env = dict(os.environ, RUSTC_BOOTSTRAP='1', CARGO_TARGET_DIR='/home/timo/.cache/scratch/ignite-perf12c/target-main',
           CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='line-tables-only',
           CARGO_PROFILE_RELEASE_DEBUG='0', CARGO_NET_OFFLINE='true')
command = ['cargo','clippy','-p','ignite','--lib','--','-Ztime-passes','-D','warnings']
logfile = base / (label+'-phases.log')
start = time.monotonic()
with logfile.open('wb') as log:
    result = subprocess.run(command, cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
text = logfile.read_text()
block = text[text.rfind('    Checking ignite v'):]
phases = {}
for line in block.splitlines():
    match = re.match(r'^time:\s*([0-9.]+);.*\)\s+(.+)$', line)
    if match:
        phases[match[2]] = float(match[1])
row = dict(label=label, command=command, exit=result.returncode, seconds_diagnostic_only=time.monotonic()-start,
           phases=phases, source_sha256=hashlib.sha256((source/'crates/ignite/src/run/live.rs').read_bytes()).hexdigest(),
           root_sha256=before, root_unchanged=before==hashlib.sha256(root.read_bytes()).hexdigest(),
           load_after=os.getloadavg(), diagnostic_only=True)
(base/(label+'-phases.json')).write_text(json.dumps(row,indent=2)+'\n')
print(json.dumps(row),flush=True)
assert result.returncode==0 and phases and row['root_unchanged']
