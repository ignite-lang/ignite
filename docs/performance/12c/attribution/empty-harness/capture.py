import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

phase = sys.argv[1]
repo = Path('/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-perf12c')
root = Path('/home/timo/.cache/scratch/ignite-perf12c/empty-harness-proof')
evidence = root / phase
evidence.mkdir(parents=True, exist_ok=False)
target = Path('/home/timo/.cache/scratch/ignite-perf12c/target-main')
env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE='true', CARGO_INCREMENTAL='0',
           CARGO_PROFILE_DEV_DEBUG='line-tables-only', CARGO_PROFILE_RELEASE_DEBUG='0', CARGO_TERM_COLOR='never')


def run(name, command):
    start = time.monotonic()
    result = subprocess.run(command, cwd=repo, env=env, capture_output=True, text=True)
    (evidence / (name + '.stdout')).write_text(result.stdout)
    (evidence / (name + '.stderr')).write_text(result.stderr)
    record = dict(command=command, exit=result.returncode, seconds=time.monotonic() - start)
    (evidence / (name + '-command.json')).write_text(json.dumps(record, indent=2) + '\n')
    if result.returncode:
        print(result.stderr, file=sys.stderr)
        raise RuntimeError(name + ' failed')
    return result.stdout


metadata = json.loads(run('metadata', ['cargo', 'metadata', '--no-deps', '--format-version', '1']))
packages = {p['id']: p['name'] for p in metadata['packages']}
summary = dict(phase=phase, packages=packages, inventories={})
for name, command in [
    ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--message-format=json', '--', '-D', 'warnings']),
    ('no-run', ['cargo', 'test', '--workspace', '--no-run', '--message-format=json']),
]:
    output = run(name, command)
    artifacts = [json.loads(line) for line in output.splitlines() if line.startswith('{')]
    units = [dict(package=packages[row['package_id']], name=row['target']['name'], kind=row['target']['kind'],
                  profile_test=row['profile']['test'], target_test=row['target']['test'],
                  target_doctest=row['target']['doctest'], fresh=row['fresh'], executable=row['executable'])
             for row in artifacts if row.get('reason') == 'compiler-artifact' and row['package_id'] in packages]
    summary['inventories'][name] = units
    print(phase, name, 'workspace units', len(units), 'test-profile units', sum(row['profile_test'] for row in units), flush=True)
tests = []
for index, unit in enumerate(summary['inventories']['no-run']):
    if not unit['profile_test'] or not unit['executable']:
        continue
    entries = dict(unit)
    for ignored, suffix in [(False, 'all'), (True, 'ignored')]:
        command = [unit['executable'], '--list'] + (['--ignored'] if ignored else [])
        output = run(f'list-{index}-{suffix}', command)
        entries[suffix] = sorted(line[:-6] for line in output.splitlines() if line.endswith(': test'))
    tests.append(entries)
summary['test_executables'] = tests
summary['case_count'] = sum(len(row['all']) for row in tests)
summary['ignored_count'] = sum(len(row['ignored']) for row in tests)
compiler = target / 'debug/ignite'
summary['normal_cli'] = dict(path=str(compiler), exists=compiler.is_file(),
                             sha256=hashlib.sha256(compiler.read_bytes()).hexdigest() if compiler.is_file() else None)
paths = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=repo).decode().split('\0')
source = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest() for name in sorted(set(paths))
          if name and (repo / name).is_file()}
(evidence / 'source-sha256.json').write_text(json.dumps(source, indent=2) + '\n')
(evidence / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(phase, 'test executables', len(tests), 'all cases', summary['case_count'], 'ignored', summary['ignored_count'],
      'normal CLI', summary['normal_cli']['exists'], flush=True)
