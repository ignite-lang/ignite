from pathlib import Path
import hashlib,json,os,shutil,subprocess,time
repo=Path('/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-perf12c')
root=Path('/home/timo/.cache/scratch/ignite-perf12c/inline-final-phase')
root.mkdir(exist_ok=False)
source=root/'source';source.mkdir()
names=subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'],cwd=repo).decode().split('\0')
manifest={}
for name in sorted(set(names)):
    if not name:continue
    p=repo/name
    if not p.is_file():continue
    out=source/name;out.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,out)
    manifest[name]=hashlib.sha256(p.read_bytes()).hexdigest()
(root/'source.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
lib=source/'crates/ignite/src/lib.rs'
text=lib.read_text()+'''
// Disposable diagnostic instrumentation, never copied to the repository.
pub(crate) struct Perf12cPhase(&'static str, std::time::Instant);
impl Drop for Perf12cPhase {
    fn drop(&mut self) { eprintln!("PERF12C {} {:.6}", self.0, self.1.elapsed().as_secs_f64()*1000.0); }
}
pub(crate) fn perf12c_phase(name: &'static str) -> Perf12cPhase { Perf12cPhase(name, std::time::Instant::now()) }
pub(crate) fn perf12c_measure<T>(name: &'static str, f: impl FnOnce()->T)->T { let _phase=perf12c_phase(name); f() }
'''
lib.write_text(text)
def guard(file,function,name):
    p=source/file;s=p.read_text();a=s.index('fn '+function+'(');b=s.index('{',a)+1
    s=s[:b]+'\n    let _perf12c_phase=crate::perf12c_phase("'+name+'");'+s[b:];p.write_text(s)
def replace(file,before,after):
    p=source/file;s=p.read_text();assert s.count(before)==1,(file,before,s.count(before));p.write_text(s.replace(before,after))
for fn,label in [('prepare_tests','prepare-tests'),('prepare_group','prepare-group'),('test_targets','test-total'),('run_executables','native-run'),('build_group','build-group'),('check_runner','check-runner')]:guard('crates/ignite/src/test_build.rs',fn,label)
for fn,label in [('load_std_reachable','load-std'),('type_std','type-std'),('type_parsed_with','type-program'),('cargo_build','cargo-generated')]:guard('crates/ignite/src/lib.rs',fn,label)
for fn,label in [('target','llvm-target'),('link','llvm-link'),('ir_hash','llvm-ir-hash'),('check_runtime_target','runtime-target'),('run_stamped','llvm-run-stamped'),('write_if_changed','llvm-write-if-changed')]:guard('crates/ignite/src/llvm_build.rs',fn,label)
replace('crates/ignite/src/test_build.rs','ignite_rustgen::generate_tests(&inputs, options)','crate::perf12c_measure("rust-test-emit", || ignite_rustgen::generate_tests(&inputs, options))')
replace('crates/ignite/src/test_build.rs','ignite_rustgen::generate_std_for(&std_modules, options, &project)','crate::perf12c_measure("rust-std-emit", || ignite_rustgen::generate_std_for(&std_modules, options, &project))')
replace('crates/ignite/src/test_build.rs','ignite_llvm::generate_tests(&std_inputs, &inputs, &options)','crate::perf12c_measure("llvm-test-emit", || ignite_llvm::generate_tests(&std_inputs, &inputs, &options))')
env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'),CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='line-tables-only',CARGO_PROFILE_RELEASE_DEBUG='0',CARGO_NET_OFFLINE='true',CARGO_TERM_COLOR='never')
for key in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','SCCACHE_RECACHE']:env.pop(key,None)
(root/'environment.json').write_text(json.dumps({k:env.get(k) for k in ['CARGO_TARGET_DIR','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_RELEASE_DEBUG','CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTFLAGS']},indent=2)+'\n')
with (root/'build.log').open('wb') as log:
    result=subprocess.run(['cargo','build','-p','ignite'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
if result.returncode:raise SystemExit(result.returncode)
files=sorted(str(p.relative_to(source)) for folder in ('std','examples') for p in (source/folder).rglob('*.ignite') if not p.name.endswith('.design.ignite'))
command=[str(root/'target/debug/ignite'),'test',*files,'--target','both','--out',str(root/'programs'),'--target-dir',str(root/'generated-target'),'--runtime','/home/timo/.cache/scratch/ignite-perf12c/target-main/ignite-test/rt/debug/libignite_rt.a']
rows=[]
for case,flags in [('warmup',['--list']),('list',['--list']),('full',[])]:
    start=time.monotonic();load=os.getloadavg()[0]
    with (root/f'{case}.log').open('wb') as log:result=subprocess.run(command+flags,cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT)
    text=(root/f'{case}.log').read_text();import re,collections
    phases=collections.defaultdict(list)
    for name,ms in re.findall(r'^PERF12C (\S+) ([0-9.]+)$',text,re.M):phases[name].append(float(ms))
    row=dict(case=case,seconds=time.monotonic()-start,load_before=load,load_after=os.getloadavg()[0],exit=result.returncode,command=command+flags,phases=dict(phases),counts=re.findall(r'test result: ok\. (\d+) passed;',text),listed=len(re.findall(r': test$',text,re.M)))
    rows.append(row);print(json.dumps(row),flush=True)
    (root/'summary.json').write_text(json.dumps(rows,indent=2)+'\n')
    if result.returncode:raise SystemExit(result.returncode)
