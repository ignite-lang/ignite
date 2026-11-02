"""Run only after the shared Cargo/CPU slot is granted. Diagnostic flags only."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import time

base = Path(__file__).parent
rows = []
for item in json.loads((base / 'plan.json').read_text()):
    env = dict(os.environ, **item['environment'])
    env.pop('SCCACHE_RECACHE', None)
    source = Path(item['cwd'])
    important = ['crates/ignite/Cargo.toml', 'crates/ignite/src/lib.rs', 'crates/ignite/tests/main.rs']
    hashes = {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in important}
    name = item['label'] + '-' + item['phase']
    logfile = base / (name + '.log')
    load = os.getloadavg()
    start = time.monotonic()
    with logfile.open('wb') as log:
        result = subprocess.run(item['command'], cwd=source, env=env, stdout=log, stderr=subprocess.STDOUT)
    phases = []
    for line in logfile.read_text(errors='replace').splitlines():
        match = re.match(r'^time:\s*([0-9.]+);.*\)\s+(.+)$', line)
        if match:
            phases.append(dict(seconds=float(match[1]), phase=match[2], raw=line))
    row = dict(item, exit=result.returncode, seconds=time.monotonic() - start,
               load_before=load, load_after=os.getloadavg(), source_hashes=hashes,
               phase_timings=phases, regular_profiles_changed=False)
    rows.append(row)
    (base / 'records.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps({key: row[key] for key in ['label', 'phase', 'exit', 'seconds']}), flush=True)
    assert result.returncode == 0
    assert phases, 'retain raw log and inspect the compiler timing format'
