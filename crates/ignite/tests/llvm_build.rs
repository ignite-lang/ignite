//! The LLVM driver's build steps (`docs/llvm.md` section 1): executables
//! are linked with unused sections dropped, an unchanged rebuild runs no
//! clang, an edit recompiles only the module it changed, a failed step runs
//! again, the default runtime lookup, and the target every module is
//! pinned to.
//! `bench` (ignored, `task bench`) prints sizes and build times of both
//! targets.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn terminal_source() -> PathBuf {
    workspace().join("examples/terminal/terminal.ignite")
}

fn which(tool: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(tool))
                .find(|path| path.is_file())
        })
        .unwrap_or_else(|| panic!("{tool} not on PATH"))
}

fn cargo() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

fn run(command: &mut Command) -> Output {
    let output = command.output().expect("start the command");
    assert!(
        output.status.success(),
        "{command:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// `ignite build --target llvm` of `source` into `out`.
fn llvm_build(source: &Path, out: &Path, clang: &Path, release: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .args(["build", "--target", "llvm"])
        .arg(source)
        .arg("--out")
        .arg(out)
        .arg("--clang")
        .arg(clang)
        .arg("--runtime")
        .arg(common::runtime_archive());
    if release {
        command.arg("--release");
    }
    command
}

/// A clang wrapper that appends each call's arguments to `log`, one line.
fn logging_clang(dir: &Path, log: &Path) -> PathBuf {
    let wrapper = dir.join("clang");
    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
        log.display(),
        which("clang").display()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    wrapper
}

/// The clang calls logged since the last call: (compiled `.ll` file names,
/// number of links), without the target probe (`-emit-llvm`). Empties the
/// log.
fn calls(log: &Path) -> (Vec<String>, usize) {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let _ = std::fs::remove_file(log);
    let mut compiled = Vec::new();
    let mut links = 0;
    for line in text.lines() {
        if line.split(' ').any(|arg| arg == "-emit-llvm") {
            continue;
        }
        if line.split(' ').any(|arg| arg == "-c") {
            let ll = line
                .split(' ')
                .find(|arg| arg.ends_with(".ll"))
                .expect("a .ll input");
            compiled.push(Path::new(ll).file_name().unwrap().to_string_lossy().into());
        } else {
            assert!(line.contains("-Wl,--gc-sections"), "link line: {line}");
            links += 1;
        }
    }
    compiled.sort();
    (compiled, links)
}

/// `docs/llvm.md` section 1, object cache: an unchanged rebuild calls clang
/// zero times; an edit to the program recompiles `program.ll` only and
/// relinks both executables.
#[test]
fn rebuild_compiles_only_what_changed() {
    let dir = tmp("llvm-cache");
    let source = dir.join("terminal.ignite");
    std::fs::create_dir_all(&dir).unwrap();
    for sibling in [
        "terminal.ignite",
        "commands.ignite",
        "command_schema.ignite",
    ] {
        std::fs::copy(terminal_source().with_file_name(sibling), dir.join(sibling)).unwrap();
    }
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let out = dir.join("out");

    run(&mut llvm_build(&source, &out, &clang, false));
    let (compiled, links) = calls(&log);
    assert_eq!(
        compiled,
        ["main_terminald.ll", "main_tn.ll", "program.ll", "std.ll"]
    );
    assert_eq!(links, 2);

    let output = run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(calls(&log), (vec![], 0), "an unchanged rebuild ran clang");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "built {}\nbuilt {}\n",
            out.join("bin/terminald").display(),
            out.join("bin/tn").display()
        )
    );

    let commands = dir.join("commands.ignite");
    let text = std::fs::read_to_string(&commands).unwrap();
    let edited = text.replace("lines = [\"ready\"]", "lines = [\"ready now\"]");
    assert_ne!(text, edited, "the edit did not apply");
    std::fs::write(&commands, edited).unwrap();
    run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(
        calls(&log),
        (vec!["program.ll".to_string()], 2),
        "a sibling module is part of program.ll"
    );

    run(&mut llvm_build(&source, &out, &clang, true));
    let (compiled, links) = calls(&log);
    assert_eq!(compiled.len(), 4, "--release recompiles every module");
    assert_eq!(links, 2);
}

/// A failed compile leaves no stamp: the next build compiles again.
#[test]
fn failed_compile_is_retried() {
    let dir = tmp("llvm-cache-failure");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let out = dir.join("out");
    run(&mut llvm_build(&source, &out, &clang, false));
    calls(&log);
    let failing_clang = dir.join("failing-clang");
    std::fs::write(
        &failing_clang,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do [ \"$arg\" = -c ] && exit 1; done\nexec '{}' \"$@\"\n",
            which("clang").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&failing_clang, std::fs::Permissions::from_mode(0o755)).unwrap();
    let failing = llvm_build(&source, &out, &failing_clang, false)
        .output()
        .unwrap();
    assert_eq!(failing.status.code(), Some(2));
    assert!(!out.join("llvm/std.o.stamp").exists());
    run(&mut llvm_build(&source, &out, &clang, false));
    assert_eq!(calls(&log), (vec!["std.ll".to_string()], 0));
}

/// `docs/llvm.md` section 1, target: every emitted module starts with
/// clang's default triple and its data layout, and clang compiles and
/// links for that triple.
#[test]
fn ir_is_pinned_to_the_target() {
    let dir = tmp("llvm-target");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let out = dir.join("out");
    run(&mut llvm_build(&source, &out, &clang, false));
    let printed = run(Command::new(which("clang")).arg("-print-target-triple"));
    let triple = String::from_utf8_lossy(&printed.stdout).trim().to_string();
    for unit in ["std", "program", "main_probe", "main_crash"] {
        let ir = std::fs::read_to_string(out.join(format!("llvm/{unit}.ll"))).unwrap();
        let mut lines = ir.lines();
        assert_eq!(
            lines.next(),
            Some(format!("target triple = \"{triple}\"").as_str()),
            "{unit}.ll"
        );
        assert!(
            lines
                .next()
                .is_some_and(|line| line.starts_with("target datalayout = \"e-")),
            "{unit}.ll"
        );
    }
    let text = std::fs::read_to_string(&log).unwrap();
    let steps: Vec<&str> = text
        .lines()
        .filter(|line| !line.contains("-emit-llvm"))
        .collect();
    assert_eq!(steps.len(), 4 + 2, "{text}");
    for line in steps {
        assert!(line.contains(&format!("--target={triple}")), "{line}");
    }
}

/// The major LLVM version of a `--version` text: `LLVM version: X.Y.Z`
/// (rustc `-vV`) or `clang version X.Y.Z`.
fn llvm_major(text: &str) -> Option<u32> {
    let rest = text
        .split("LLVM version: ")
        .nth(1)
        .or_else(|| text.split("clang version ").nth(1))?;
    rest.split('.').next()?.trim().parse().ok()
}

/// Whether this host can link a release build with LTO (`docs/llvm.md`
/// section 1): rustc and clang share an LLVM major version and `ld.lld`
/// is on `PATH`.
fn lto_possible() -> bool {
    let version = |tool: &str, flag: &str| {
        let output = Command::new(tool).arg(flag).output().ok()?;
        llvm_major(&String::from_utf8_lossy(&output.stdout))
    };
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc = version(&rustc, "-vV");
    rustc.is_some()
        && rustc == version("clang", "--version")
        && std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join("ld.lld").is_file())
        })
}

/// `ignite build --target llvm --release` of the probe with no `--runtime`,
/// the runtime cache under `dir/cache`.
fn release_build_of_the_probe(dir: &Path, clang: &Path) -> Output {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    run(Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["build", "--target", "llvm", "--release"])
        .arg(&source)
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--clang")
        .arg(clang)
        .env("XDG_CACHE_HOME", dir.join("cache")))
}

/// Exit code, standard output and standard error of the probe's two
/// executables in `out/bin`.
fn probe_results(out: &Path) -> Vec<(Option<i32>, String, String)> {
    ["probe", "crash"]
        .iter()
        .map(|name| {
            let output = Command::new(out.join("bin").join(name)).output().unwrap();
            (
                output.status.code(),
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        })
        .collect()
}

/// The probe built without `--release` and `--runtime`: what every release
/// build must print.
fn dev_probe_results(dir: &Path) -> Vec<(Option<i32>, String, String)> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    let out = dir.join("dev");
    run(&mut llvm_build(&source, &out, &which("clang"), false));
    probe_results(&out)
}

/// Story `llvm-lto-runtime`, `docs/llvm.md` section 1: with `--release`
/// and no `--runtime`, every unit is compiled to bitcode (`-flto=thin`)
/// and each executable is linked by lld together with the bitcode runtime
/// the driver carries, written to `<cache>/ignite/runtime/lto-<hash>/`.
/// The executables behave as the dev build's.
#[test]
fn release_links_std_program_and_runtime_with_lto() {
    if !lto_possible() {
        eprintln!("skipped: rustc and clang differ in LLVM version, or no ld.lld");
        return;
    }
    let dir = tmp("llvm-lto");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    release_build_of_the_probe(&dir, &clang);
    let text = std::fs::read_to_string(&log).unwrap();
    let steps: Vec<&str> = text
        .lines()
        .filter(|line| !line.contains("-emit-llvm"))
        .collect();
    assert_eq!(steps.len(), 4 + 2, "{text}");
    let lto_runtime = dir.join("cache/ignite/runtime");
    for line in steps {
        let args: Vec<&str> = line.split(' ').collect();
        assert!(args.contains(&"-flto=thin"), "{line}");
        if !args.contains(&"-c") {
            assert!(args.contains(&"-fuse-ld=lld"), "{line}");
            assert!(args.contains(&"-O2"), "{line}");
            let runtime = args
                .iter()
                .find(|arg| arg.ends_with(".a"))
                .unwrap_or_else(|| panic!("no archive in {line}"));
            assert!(
                runtime.starts_with(&*lto_runtime.join("lto-").to_string_lossy()),
                "{line}"
            );
        }
    }
    assert_eq!(probe_results(&dir.join("out")), dev_probe_results(&dir));
}

/// Story `llvm-lto-runtime`, the fallback: a clang of another LLVM version
/// than the rustc that built the carried runtime cannot read its bitcode,
/// so the release build compiles native objects and links the native
/// runtime without LTO, as does a release build with `--runtime`.
#[test]
fn release_with_another_llvm_links_the_native_runtime_without_lto() {
    let dir = tmp("llvm-lto-fallback");
    let log = dir.join("clang.log");
    let wrapper = dir.join("clang");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\ncase \" $* \" in *' -emit-llvm '*) '{clang}' \"$@\" | sed 's/clang version [0-9]*/clang version 1/'; exit ;; esac\nexec '{clang}' \"$@\"\n",
            log.display(),
            clang = which("clang").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    release_build_of_the_probe(&dir, &wrapper);
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("-flto"), "{text}");
    assert!(
        !dir.join("cache/ignite/runtime").exists() || {
            std::fs::read_dir(dir.join("cache/ignite/runtime"))
                .unwrap()
                .all(|entry| {
                    !entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with("lto-")
                })
        }
    );
    let expected = dev_probe_results(&dir);
    assert_eq!(probe_results(&dir.join("out")), expected);

    let _ = std::fs::remove_file(&log);
    let explicit = dir.join("explicit");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    run(&mut llvm_build(
        &source,
        &explicit,
        &logging_clang(&dir, &log),
        true,
    ));
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("-flto"), "{text}");
    assert_eq!(probe_results(&explicit), expected);
}

/// Story `llvm-lto-runtime`: `ignite test --release` with no `--runtime`
/// probes the target with the native runtime it links, so its IR and its
/// objects agree on one triple and the tests pass, without LTO.
#[test]
fn release_tests_link_the_native_runtime() {
    let dir = tmp("llvm-lto-tests");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["test", "--target", "llvm", "--release"])
        .arg(workspace().join("std/option.ignite"))
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--clang")
        .arg(&clang)
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("-flto"), "{text}");
}

/// `docs/llvm.md` section 1, target: a runtime archive built for another
/// architecture than `--triple` fails with a clear error (exit 3) before
/// clang compiles anything.
#[test]
fn runtime_for_another_target_is_refused() {
    let dir = tmp("llvm-target-mismatch");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    let log = dir.join("clang.log");
    let clang = logging_clang(&dir, &log);
    let (other, needs) = if cfg!(target_arch = "aarch64") {
        ("x86_64-unknown-linux-gnu", 62)
    } else {
        ("aarch64-unknown-linux-gnu", 183)
    };
    let output = llvm_build(&source, &dir.join("out"), &clang, false)
        .args(["--triple", other])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    for needle in [
        format!("the runtime archive is not built for the target {other}: its objects are ELF64"),
        format!("the target needs machine {needs} (build ignite-rt for {other}"),
    ] {
        assert!(stderr.contains(&needle), "{stderr}");
    }
    assert_eq!(calls(&log), (vec![], 0), "clang ran before the check");
}

/// `docs/llvm.md` section 1, runtime lookup: `libignite_rt.a` next to the
/// driver, then in its parent directory, as for a driver in
/// `target/<profile>/deps`. Both come before the runtime the driver
/// carries; with neither and a cache directory that cannot be written the
/// build is exit 3, naming the archive next to the driver and `--runtime`.
#[test]
fn default_runtime_is_next_to_the_driver_or_in_its_parent() {
    let dir = tmp("llvm-runtime-lookup");
    let deps = dir.join("deps");
    std::fs::create_dir_all(&deps).unwrap();
    let driver = deps.join("ignite");
    std::fs::copy(env!("CARGO_BIN_EXE_ignite"), &driver).unwrap();
    assert_eq!(
        ignite::runtime_candidates(&driver),
        [deps.join("libignite_rt.a"), dir.join("libignite_rt.a")]
    );
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.ignite");
    let unwritable = dir.join("cache-is-a-file");
    std::fs::write(&unwritable, "").unwrap();
    let out = dir.join("out");
    let build = || {
        run_copied(
            &driver,
            &[
                "build".as_ref(),
                "--target".as_ref(),
                "llvm".as_ref(),
                source.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            &unwritable,
            &unwritable,
        )
    };

    let missing = build();
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert_eq!(missing.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains(&*deps.join("libignite_rt.a").to_string_lossy()),
        "{stderr}"
    );
    assert!(stderr.contains("--runtime"), "{stderr}");

    std::fs::copy(common::runtime_archive(), dir.join("libignite_rt.a")).unwrap();
    let parent = build();
    assert!(
        parent.status.success(),
        "{}",
        String::from_utf8_lossy(&parent.stderr)
    );

    std::fs::write(deps.join("libignite_rt.a"), "not an archive").unwrap();
    let next_to = build();
    assert_eq!(
        next_to.status.code(),
        Some(2),
        "the archive next to the driver comes first"
    );
}

#[test]
fn default_std_is_share_ignite_std_beside_the_driver_when_it_exists() {
    let dir = tmp("std-lookup");
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let driver = bin.join("ignite");
    std::fs::copy(env!("CARGO_BIN_EXE_ignite"), &driver).unwrap();
    let source = dir.join("main.ignite");
    std::fs::write(
        &source,
        "package main\n\nimport std.text.split\n\nfn words(line: Text) -> List<Text> = line.split(\" \")\n",
    )
    .unwrap();
    let check = || run_copied(&driver, &["check".as_ref(), source.as_os_str()], &dir, &dir);
    assert_eq!(ignite::installed_std_path(&driver), None);
    let repository = check();
    assert!(
        repository.status.success(),
        "without share/ignite/std the repository's std is used: {}",
        String::from_utf8_lossy(&repository.stderr)
    );

    let installed = dir.join("share/ignite/std");
    std::fs::create_dir_all(&installed).unwrap();
    assert_eq!(
        ignite::installed_std_path(&driver),
        Some(installed.canonicalize().unwrap())
    );
    let empty = check();
    assert_eq!(
        empty.status.code(),
        Some(1),
        "an empty share/ignite/std has no std.text: {}",
        String::from_utf8_lossy(&empty.stderr)
    );
}

/// Run `driver` with `args`, the runtime cache under `cache` and `HOME`
/// under `home`, retrying an exec that fails with ETXTBSY (a copied
/// driver's write descriptor held for an instant by another test thread's
/// fork).
fn run_copied(driver: &Path, args: &[&std::ffi::OsStr], cache: &Path, home: &Path) -> Output {
    let mut attempts = 0;
    loop {
        let output = Command::new(driver)
            .args(args)
            .env("XDG_CACHE_HOME", cache)
            .env("HOME", home)
            .output();
        match output {
            Err(error) if error.raw_os_error() == Some(26) && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => return other.unwrap(),
        }
    }
}

/// The archives in the runtime cache under `cache`, each with its inode
/// and modification time.
fn cached_runtimes(cache: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    use std::os::unix::fs::MetadataExt;
    let root = cache.join("ignite/runtime");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut found: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("libignite_rt.a"))
        .filter_map(|path| {
            let metadata = std::fs::metadata(&path).ok()?;
            Some((path, metadata.ino(), metadata.modified().ok()?))
        })
        .collect();
    found.sort();
    found
}

/// Story `llvm-runtime-without-flags`: an `ignite` copied alone into a
/// fresh directory, with no `libignite_rt.a` beside it, no `--runtime` and
/// an empty cache, builds the todo example on the default target by
/// writing the runtime it carries to `$XDG_CACHE_HOME/ignite/runtime/`; a
/// second build links the cached archive without writing it again.
#[test]
fn a_driver_alone_links_the_runtime_it_carries() {
    let dir = tmp("llvm-runtime-embedded");
    let bin = dir.join("bin");
    let cache = dir.join("cache");
    let home = dir.join("home");
    for path in [&bin, &cache, &home] {
        std::fs::create_dir_all(path).unwrap();
    }
    let driver = bin.join("ignite");
    std::fs::copy(env!("CARGO_BIN_EXE_ignite"), &driver).unwrap();
    let source = workspace().join("examples/todo/todo.ignite");
    let build = |out: &Path| {
        run_copied(
            &driver,
            &[
                "build".as_ref(),
                source.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            &cache,
            &home,
        )
    };

    let first_out = dir.join("out1");
    let first = build(&first_out);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first_out.join("bin/todod").is_file());
    assert!(
        first_out.join("llvm/program.ll").is_file(),
        "the default target is LLVM"
    );
    assert!(!first_out.join("Cargo.toml").exists());
    let cached = cached_runtimes(&cache);
    assert_eq!(cached.len(), 1, "{cached:?}");
    assert!(!bin.join("libignite_rt.a").exists());
    assert!(!home.join(".cache").exists(), "XDG_CACHE_HOME comes first");

    let second_out = dir.join("out2");
    let second = build(&second_out);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(second_out.join("bin/todod").is_file());
    assert_eq!(
        cached_runtimes(&cache),
        cached,
        "the cached archive is reused"
    );

    let tests = workspace().join("std/option.ignite");
    let test_out = dir.join("test-out");
    let tested = run_copied(
        &driver,
        &[
            "test".as_ref(),
            tests.as_os_str(),
            "--out".as_ref(),
            test_out.as_os_str(),
        ],
        &cache,
        &home,
    );
    assert!(
        tested.status.success(),
        "the carried archive has the test runner: {}{}",
        String::from_utf8_lossy(&tested.stdout),
        String::from_utf8_lossy(&tested.stderr)
    );
    assert_eq!(cached_runtimes(&cache), cached);
}

/// Story `llvm-runtime-without-flags`, end to end: `cargo install --root`
/// into a fresh directory, then the installed `ignite` builds the todo
/// example with no flags and an empty cache. Ignored: it compiles the
/// workspace in release once more.
#[test]
#[ignore = "cargo install of the workspace; run with --ignored"]
fn cargo_install_builds_without_a_runtime_flag() {
    let dir = tmp("llvm-runtime-install");
    let root = dir.join("root");
    let cache = dir.join("cache");
    let home = dir.join("home");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    run(Command::new(cargo())
        .args(["install", "--locked", "--path"])
        .arg(workspace().join("crates/ignite"))
        .arg("--root")
        .arg(&root)
        .arg("--target-dir")
        .arg(dir.join("target")));
    let driver = root.join("bin/ignite");
    assert!(!root.join("bin/libignite_rt.a").exists());
    let source = workspace().join("examples/todo/todo.ignite");
    let out = dir.join("out");
    let built = run_copied(
        &driver,
        &[
            "build".as_ref(),
            source.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        &cache,
        &home,
    );
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert!(out.join("bin/todod").is_file());
    assert!(out.join("llvm/program.ll").is_file());
    assert_eq!(cached_runtimes(&cache).len(), 1);
}

/// Seconds `f` takes.
fn time(f: impl FnOnce()) -> f64 {
    let start = Instant::now();
    f();
    start.elapsed().as_secs_f64()
}

fn size(file: &Path) -> u64 {
    std::fs::metadata(file).unwrap().len()
}

fn stripped_size(file: &Path, scratch: &Path) -> u64 {
    let stripped = scratch.join("stripped");
    run(Command::new("strip").arg("-o").arg(&stripped).arg(file));
    size(&stripped)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / 1e6)
}

/// A command with sccache off, so that a cold build is cold.
fn uncached(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("CARGO_INCREMENTAL", "0");
    command
}

/// `task bench`: sizes of `terminald` for both targets and profiles, build
/// times (runtime archive and program cold, unchanged rebuild; no sccache)
/// and the time of 200 runs of `terminald --port nope`.
#[test]
#[ignore = "benchmark; run with task bench"]
fn bench() {
    let root = tmp("bench");
    let ignite = env!("CARGO_BIN_EXE_ignite");
    let source = terminal_source();
    let clang = which("clang");
    let profiles = ["debug", "release"];
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let mut sizes = Vec::new();
    let mut runtime_cold = Vec::new();
    let mut cold = Vec::new();
    let mut rebuild = Vec::new();
    let mut startup = Vec::new();
    let mut binaries = Vec::new();

    for profile in profiles {
        let release = profile == "release";
        let rt_target = root.join("runtime");
        let mut command = uncached(cargo());
        command
            .args(["build", "-q", "-p", "ignite-rt", "--manifest-path"])
            .arg(workspace().join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&rt_target);
        if release {
            command.arg("--release");
        }
        runtime_cold.push(time(|| {
            run(&mut command);
        }));
        let runtime = rt_target.join(profile).join("libignite_rt.a");
        let out = root.join(format!("llvm-{profile}"));
        let mut build = Command::new(ignite);
        build
            .args(["build", "--target", "llvm"])
            .arg(&source)
            .arg("--out")
            .arg(&out)
            .arg("--clang")
            .arg(&clang)
            .arg("--runtime")
            .arg(&runtime);
        if release {
            build.arg("--release");
        }
        cold.push(time(|| {
            run(&mut build);
        }));
        rebuild.push(time(|| {
            run(&mut build);
        }));
        binaries.push(out.join("bin/terminald"));
    }
    for profile in profiles {
        let out = root.join(format!("rust-{profile}"));
        let target_dir = root.join(format!("rust-{profile}-target"));
        let mut build = uncached(ignite);
        build
            .args(["build", "--target", "rust"])
            .arg(&source)
            .arg("--out")
            .arg(&out)
            .arg("--target-dir")
            .arg(&target_dir);
        if profile == "release" {
            build.arg("--release");
        }
        cold.push(time(|| {
            run(&mut build);
        }));
        rebuild.push(time(|| {
            run(&mut build);
        }));
        binaries.push(target_dir.join(profile).join("terminald"));
    }
    for binary in &binaries {
        sizes.push((size(binary), stripped_size(binary, &root)));
        startup.push(time(|| {
            for _ in 0..200 {
                let status = Command::new(binary)
                    .args(["--port", "nope"])
                    .output()
                    .unwrap()
                    .status;
                assert_eq!(status.code(), Some(2));
            }
        }));
    }

    let seconds =
        |values: &[f64]| -> Vec<String> { values.iter().map(|s| format!("{s:.2} s")).collect() };
    rows.push((
        "terminald size".into(),
        sizes.iter().map(|(full, _)| megabytes(*full)).collect(),
    ));
    rows.push((
        "terminald stripped".into(),
        sizes
            .iter()
            .map(|(_, stripped)| megabytes(*stripped))
            .collect(),
    ));
    rows.push((
        "runtime archive cold".into(),
        seconds(&runtime_cold)
            .into_iter()
            .chain(["-".into(), "-".into()])
            .collect(),
    ));
    rows.push(("program build cold".into(), seconds(&cold)));
    rows.push(("rebuild unchanged".into(), seconds(&rebuild)));
    rows.push(("200 runs --port nope".into(), seconds(&startup)));
    println!(
        "\n{:<22}{:>14}{:>14}{:>14}{:>14}",
        "", "llvm debug", "llvm release", "rust debug", "rust release"
    );
    for (label, values) in rows {
        print!("{label:<22}");
        for value in values {
            print!("{value:>14}");
        }
        println!();
    }
}
