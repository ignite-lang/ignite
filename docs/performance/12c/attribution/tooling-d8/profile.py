"""Package-unit attribution only; no performance acceptance claims."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import time

base = Path(__file__).parent
plan = json.loads((base / 'plan.json').read_text())
rows = []
for item in plan:
    label, unit = item['label'], item['unit']
    source = Path(item['touch'])
    before_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    before_mtime = source.stat().st_mtime_ns
    source.touch()
    env = dict(os.environ, CARGO_TARGET_DIR=item['target'], CARGO_INCREMENTAL='0',
               CARGO_PROFILE_DEV_DEBUG='line-tables-only', CARGO_PROFILE_RELEASE_DEBUG='0',
               CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='never')
    env.pop('SCCACHE_RECACHE', None)
    logfile = base / f'{label}-{unit}.log'
    before_load = os.getloadavg()
    start = time.monotonic()
    with logfile.open('wb') as log:
        result = subprocess.run(item['command'], cwd=item['cwd'], env=env,
                                stdout=log, stderr=subprocess.STDOUT)
    elapsed = time.monotonic() - start
    timing = Path(item['target']) / 'cargo-timings/cargo-timing.html'
    units = []
    if timing.exists():
        destination = base / f'{label}-{unit}.html'
        shutil.copy2(timing, destination)
        text = destination.read_text()
        marker = 'const UNIT_DATA = '
        if marker not in text:
            marker = 'const UNIT_DATA ='
        units, _ = json.JSONDecoder().raw_decode(text.split(marker, 1)[1].lstrip())
        (base / f'{label}-{unit}-units.json').write_text(json.dumps(units, indent=2) + '\n')
    row = dict(item, exit=result.returncode, seconds=elapsed, load_before=before_load,
               load_after=os.getloadavg(), checker_hash_before=before_hash,
               checker_hash_after=hashlib.sha256(source.read_bytes()).hexdigest(),
               checker_mtime_before=before_mtime, checker_mtime_after=source.stat().st_mtime_ns,
               unit_count=len(units), diagnostic_only=True)
    rows.append(row)
    (base / 'records.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    assert row['checker_hash_after'] == before_hash
    assert result.returncode == 0
