#!/usr/bin/env python3
"""One uninstrumented-source check under Callgrind. Diagnostic, never a timing row."""
import hashlib, json, os, signal, subprocess, threading, time
from pathlib import Path

base = Path('/home/timo/.cache/scratch/ignite-perf12c')
source = base / 'final-bench/sources/candidate-c6b22cf148f1'
cli = base / 'final-bench/builds/candidate-c6b22cf148f1/debug/ignite'
manifest = base / 'final-bench/evidence-v5/source-candidate.json'
out = base / 'design-c6-cpu'
out.mkdir(exist_ok=False)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
expected = json.loads(manifest.read_text())
actual = {name: sha(source / name) for name in expected}
if actual != expected:
    raise RuntimeError('Frozen source does not match candidate manifest')
cmd = ['valgrind', '--tool=callgrind', '--error-exitcode=97',
       '--callgrind-out-file=' + str(out / 'design-check.callgrind'),
       str(cli), 'check', 'examples/terminal/terminal.design.ignite']
metadata = {'command': cmd, 'cwd': str(source), 'cli_sha256': sha(cli),
            'manifest': str(manifest), 'manifest_sha256': sha(manifest),
            'source_files': len(actual), 'diagnostic_only': True,
            'load_before': os.getloadavg()}
(out / 'source.json').write_text(json.dumps(actual, indent=2) + '\n')
(out / 'command.json').write_text(json.dumps(metadata, indent=2) + '\n')
with (out / 'stdout').open('wb') as stdout, (out / 'stderr').open('wb') as stderr:
    start = time.monotonic()
    proc = subprocess.Popen(cmd, cwd=source, stdout=stdout, stderr=stderr, start_new_session=True)
    timer = threading.Timer(45, lambda: os.killpg(proc.pid, signal.SIGKILL) if proc.poll() is None else None)
    timer.start()
    try:
        metadata['exit'] = proc.wait()
    finally:
        timer.cancel()
    metadata['seconds_diagnostic_only'] = time.monotonic() - start
metadata['load_after'] = os.getloadavg()
metadata['cli_sha256_after'] = sha(cli)
(out / 'command.json').write_text(json.dumps(metadata, indent=2) + '\n')
if metadata['exit'] or metadata['cli_sha256'] != metadata['cli_sha256_after']:
    raise RuntimeError('Profile command failed or CLI changed')
for label, args in [('self', ['--inclusive=no']), ('inclusive', ['--inclusive=yes']),
                    ('callers', ['--inclusive=yes', '--tree=both'])]:
    with (out / (label + '.txt')).open('wb') as log:
        subprocess.run(['callgrind_annotate', '--threshold=100', '--auto=no', *args,
                        str(out / 'design-check.callgrind')], stdout=log, stderr=subprocess.STDOUT, check=True)
print(out)
