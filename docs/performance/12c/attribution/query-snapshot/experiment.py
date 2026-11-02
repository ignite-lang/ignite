#!/usr/bin/env python3
"""Bounded three-pair source-local scope-map diagnostic; no acceptance claim."""
import hashlib,json,os,shutil,statistics,subprocess,time,resource,signal,threading,re,collections
from pathlib import Path
root=Path('/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-perf12c')
out=Path(__file__).parent
owned=root/'crates/ignite-types/src/lib.rs'
target=out.parent/'target-main'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert owned.read_bytes()==(out/'after-lib.rs').read_bytes()
owned.write_bytes((out/'before-lib.rs').read_bytes())
env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='line-tables-only',CARGO_PROFILE_RELEASE_DEBUG='0',CARGO_NET_OFFLINE='true',CARGO_TERM_COLOR='never')
for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','SCCACHE_RECACHE']:env.pop(k,None)
(out/'environment.json').write_text(json.dumps({k:env.get(k) for k in ['CARGO_TARGET_DIR','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_RELEASE_DEBUG','CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTFLAGS']},indent=2)+'\n')
tracked=subprocess.check_output(['git','ls-files','-z'],cwd=root).decode().split('\0')
def source_manifest():
 return {n:sha(root/n) for n in tracked if n and (n.endswith(('.rs','.toml','.ignite')) or n=='Cargo.lock')}
before=source_manifest()
(out/'source-before.json').write_text(json.dumps(before,indent=2)+'\n')
def build(label):
 with (out/(label+'-build.log')).open('wb') as log:
  subprocess.run(['cargo','build','-q','-p','ignite'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 binary=out/('ignite-'+label)
 shutil.copy2(target/'debug/ignite',binary)
 return {'cli_sha256':sha(binary),'source_sha256':sha(owned)}
meta={}
meta['before']=build('before')
assert source_manifest()==before,'Source changed during baseline build'
owned.write_bytes((out/'after-lib.rs').read_bytes())
meta['after']=build('after')
after=source_manifest()
changed=[n for n in before if before[n]!=after[n]]
assert changed==['crates/ignite-types/src/lib.rs'],changed
(out/'source-after.json').write_text(json.dumps(after,indent=2)+'\n')
(out/'builds.json').write_text(json.dumps(meta,indent=2)+'\n')
rows=[]
def run(label,unit,pair,phase):
 cli=out/('ignite-'+label)
 cmd=[str(cli),unit,'examples/terminal/terminal.design.ignite']
 result=out/(f'{phase}-{pair}-{unit}-{label}.json')
 if unit=='plan':cmd+=['--out',str(result)]
 before_load=os.getloadavg()
 with (out/(f'{phase}-{pair}-{unit}-{label}.stdout')).open('wb') as stdout,(out/(f'{phase}-{pair}-{unit}-{label}.stderr')).open('wb') as stderr:
  usage_before=resource.getrusage(resource.RUSAGE_CHILDREN)
  start=time.perf_counter()
  proc=subprocess.Popen(cmd,cwd=root,env=env,stdout=stdout,stderr=stderr)
  code=proc.wait()
  elapsed=time.perf_counter()-start
  usage_after=resource.getrusage(resource.RUSAGE_CHILDREN)
 row={'phase':phase,'pair':pair,'label':label,'unit':unit,'seconds':elapsed,'exit':code,'load_before':before_load,'load_after':os.getloadavg(),'command':cmd,'cpu_seconds':(usage_after.ru_utime+usage_after.ru_stime)-(usage_before.ru_utime+usage_before.ru_stime)}
 if unit=='plan' and result.exists():row['output_sha256']=sha(result)
 rows.append(row)
 (out/'rows.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert code==0,row
for unit in ['check','plan']:
 for label in ['before','after']:run(label,unit,0,'warmup')
for pair in range(1,4):
 for unit in ['check','plan']:
  order=['before','after'] if (pair+(unit=='plan'))%2 else ['after','before']
  for label in order:run(label,unit,pair,'measured')
assert source_manifest()==after,'Source changed during comparison'
summary={}
for unit in ['check','plan']:
 selected=[r for r in rows if r['phase']=='measured' and r['unit']==unit]
 before_rows=[r for r in selected if r['label']=='before']
 after_rows=[r for r in selected if r['label']=='after']
 summary[unit]={'before':statistics.median(r['seconds'] for r in before_rows),'after':statistics.median(r['seconds'] for r in after_rows),'median_saved_seconds':statistics.median(next(r['seconds'] for r in before_rows if r['pair']==p)-next(r['seconds'] for r in after_rows if r['pair']==p) for p in range(1,4)),'raw_pairs':len(before_rows)}
 if unit=='plan':assert len({r['output_sha256'] for r in rows if r['unit']=='plan'})==1,'Canonical model changed'
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for unit in ['check','plan']:
 for label in ['before','after']:
  summary[unit][label+'_cpu_seconds']=statistics.median(r['cpu_seconds'] for r in rows if r['phase']=='measured' and r['unit']==unit and r['label']==label)
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)

def parse_profile(path):
 names={};current=None;callee=None;edge=False
 own=collections.defaultdict(int);inclusive=collections.defaultdict(int);calls=collections.defaultdict(int)
 for line in path.open():
  if line.startswith('summary:'):total=int(line.split(':')[1])
  if line.startswith(('fn=','cfn=')):
   kind,val=line.rstrip().split('=',1)
   match=re.match(r'\((\d+)\)(?: (.*))?',val)
   assert match,line
   idx=int(match[1])
   if match[2]:names[idx]=match[2]
   if kind=='fn':current=idx
   else:callee=idx
  elif line.startswith('calls='):
   edge=True;calls[callee]+=int(line.split('=',1)[1].split()[0])
  elif current is not None and re.match(r'^[0-9+*\-]',line):
   fields=line.split()
   if len(fields)<2:continue
   cost=int(fields[1]);inclusive[current]+=cost
   if not edge:own[current]+=cost
   edge=False
 result={'total_instructions':total,'selected_functions':[]}
 for idx,name in names.items():
  if any(t in name for t in ['<ignite_types::ModuleTypes as core::clone::Clone>::clone','<ignite_types::ModuleTypes>::query_snapshot','<ignite_types::FnSig as core::clone::Clone>::clone','ignite_types::type_module']):
   result['selected_functions'].append({'name':name,'self':own[idx],'inclusive':inclusive[idx],'calls':calls[idx]})
 return result
profiles={}
for label in ['before','after']:
 cmd=['valgrind','--tool=callgrind','--error-exitcode=97','--callgrind-out-file='+str(out/(label+'.callgrind')),str(out/('ignite-'+label)),'check','examples/terminal/terminal.design.ignite']
 with (out/(label+'-callgrind.stdout')).open('wb') as stdout,(out/(label+'-callgrind.stderr')).open('wb') as stderr:
  start=time.monotonic()
  proc=subprocess.Popen(cmd,cwd=root,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  timer=threading.Timer(45,lambda:os.killpg(proc.pid,signal.SIGKILL) if proc.poll() is None else None)
  timer.start()
  try:code=proc.wait()
  finally:timer.cancel()
  profiles[label]={'command':cmd,'exit':code,'seconds_diagnostic_only':time.monotonic()-start,'cli_sha256':sha(out/('ignite-'+label))}
  assert code==0,profiles[label]
 profiles[label].update(parse_profile(out/(label+'.callgrind')))
 (out/'profiles.json').write_text(json.dumps(profiles,indent=2)+'\n')
assert source_manifest()==after,'Source changed during profiling'
print(json.dumps(profiles,indent=2),flush=True)

