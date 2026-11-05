//! The performance budget harness: reproduce the frozen 11b comparison without editing a
//! checkout (the former `scripts/performance.py`, schema `ignite-performance/1`).
//!
//! Source exports, build caches and evidence have separate roots. Source exports are the
//! committed trees, not linked worktrees; evidence stays outside disposable targets. Records,
//! `run.json`, `summary.json` and the per-run files keep the Python harness's fields and bytes
//! (`crate::json`), so evidence of both harnesses compares line by line.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::cli::{RecoverArgs, RunArgs};
use crate::json::{Json, obj};

pub type R<T> = Result<T, String>;

pub const SCHEMA: &str = "ignite-performance/1";

/// The gate's units and the serial commands that own them.
pub fn gate() -> Vec<(&'static str, Vec<String>)> {
    let words = |text: &str| text.split(' ').map(String::from).collect::<Vec<_>>();
    vec![
        ("fmt", words("cargo fmt --all -- --check")),
        (
            "clippy",
            words("cargo clippy --workspace --all-targets -- -D warnings"),
        ),
        ("workspace", words("cargo test --workspace --no-fail-fast")),
        ("inline", words("task test-ignite")),
    ]
}

pub fn ensure(condition: bool, message: impl Into<String>) -> R<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub fn io_err(what: impl std::fmt::Display) -> impl FnOnce(std::io::Error) -> String {
    move |error| format!("{what}: {error}")
}

pub fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `Path.resolve()`: absolute, symlinks of the existing prefix resolved, the rest appended.
pub fn resolve(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut out = PathBuf::from("/");
    for component in absolute.components() {
        match component {
            Component::Normal(name) => {
                out.push(name);
                if let Ok(real) = out.canonicalize() {
                    out = real;
                }
            }
            Component::ParentDir => {
                out.pop();
            }
            _ => {}
        }
    }
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn digest(path: &Path) -> R<String> {
    Ok(sha256(&fs::read(path).map_err(io_err(path.display()))?))
}

pub fn read_json(path: &Path) -> R<Json> {
    let text = fs::read_to_string(path).map_err(io_err(path.display()))?;
    Json::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// `json.dumps(value, indent=2, sort_keys=True)` plus a newline, parents created.
pub fn dump(path: &Path, value: &Json) -> R<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err(parent.display()))?;
    }
    fs::write(path, value.dumps_indent(true, 2) + "\n").map_err(io_err(path.display()))
}

/// Python's `subprocess` return code: negative for a signal.
pub fn exit_code(status: ExitStatus) -> i64 {
    match status.code() {
        Some(code) => i64::from(code),
        None => -i64::from(status.signal().unwrap_or(0)),
    }
}

/// `os.getloadavg()[0]`.
pub fn loadavg() -> R<f64> {
    nix::sys::sysinfo::sysinfo()
        .map(|info| info.load_average().0)
        .map_err(|e| format!("getloadavg: {e}"))
}

fn py_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    format!("[{}]", quoted.join(", "))
}

/// `subprocess.check_output(["git", "-C", repo, *args], input=data, env=env)`.
pub fn git(repo: &Path, args: &[&str], data: Option<Vec<u8>>, env: &[(&str, &Path)]) -> R<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    command
        .stdin(if data.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped());
    let mut child = command.spawn().map_err(io_err("git"))?;
    let writer = data.map(|data| {
        let mut stdin = child.stdin.take().expect("piped stdin");
        std::thread::spawn(move || stdin.write_all(&data))
    });
    let output = child.wait_with_output().map_err(io_err("git"))?;
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| "git stdin writer panicked".to_string())?
            .map_err(io_err("git stdin"))?;
    }
    if !output.status.success() {
        let mut full = vec!["git".to_string(), "-C".into(), path_str(repo)];
        full.extend(args.iter().map(|a| a.to_string()));
        return Err(format!(
            "Command '{}' returned non-zero exit status {}.",
            py_list(&full),
            exit_code(output.status)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn verify_manifest(root: &Path, manifest: &Json) -> R<()> {
    let mut changed = Vec::new();
    for (name, expected) in manifest.as_obj() {
        let path = root.join(name);
        let same = path.is_file() && digest(&path).ok().as_deref() == expected.as_str();
        if !same {
            changed.push(name.clone());
        }
    }
    ensure(
        changed.is_empty(),
        format!("Source manifest mismatch: {}", changed.join(", ")),
    )
}

/// Every file under `root` (following file symlinks, not directory symlinks) and its SHA256.
pub fn source_manifest(root: &Path) -> R<Json> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> R<()> {
        for entry in fs::read_dir(dir).map_err(io_err(dir.display()))? {
            let path = entry.map_err(io_err(dir.display()))?.path();
            let link = fs::symlink_metadata(&path).map_err(io_err(path.display()))?;
            if link.is_dir() {
                walk(root, &path, out)?;
            } else if path.is_file() {
                let name = path.strip_prefix(root).expect("walked under root");
                out.insert(path_str(name), digest(&path)?);
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files)?;
    Ok(Json::Obj(
        files.into_iter().map(|(k, v)| (k, Json::Str(v))).collect(),
    ))
}

/// The tree of the manifest's files of `source`, written through the private index `index`.
fn private_tree(repo: &Path, source: &Path, manifest: &Json, index: &Path) -> R<String> {
    let env = [("GIT_INDEX_FILE", index)];
    git(repo, &["read-tree", "--empty"], None, &env)?;
    let mut names: Vec<&String> = manifest.as_obj().iter().map(|(k, _)| k).collect();
    names.sort();
    let mut entries = Vec::new();
    for name in names {
        let path = source.join(name);
        let meta = fs::symlink_metadata(&path).map_err(io_err(path.display()))?;
        ensure(
            !meta.file_type().is_symlink(),
            format!("Recovery needs an explicit symlink manifest: {name}"),
        )?;
        let file = path_str(&path);
        let blob = git(
            repo,
            &["hash-object", "-w", "--no-filters", &file],
            None,
            &[],
        )?;
        let mode = if meta.permissions().mode() & 0o111 != 0 {
            "100755"
        } else {
            "100644"
        };
        entries.extend(format!("{mode} {}\t{name}\0", blob.trim()).into_bytes());
    }
    git(
        repo,
        &["update-index", "-z", "--index-info"],
        Some(entries),
        &env,
    )?;
    Ok(git(repo, &["write-tree"], None, &env)?.trim().to_string())
}

/// Write only Git objects and the ref through a private index, never the checkout index.
pub fn recover(args: &RecoverArgs) -> R<()> {
    let source = resolve(&args.source);
    let evidence = resolve(&args.evidence);
    let manifest_path = resolve(&args.manifest);
    let repo = &args.repo;
    let manifest = read_json(&manifest_path)?;
    verify_manifest(&source, &manifest)?;
    for (name, _) in manifest.as_obj() {
        let path = Path::new(name);
        ensure(
            !path.is_absolute() && !path.components().any(|c| c == Component::ParentDir),
            "Manifest paths must be relative source paths",
        )?;
    }
    fs::create_dir_all(&evidence).map_err(io_err(evidence.display()))?;
    let parent_rev = format!("{}^{{commit}}", args.parent);
    let parent = git(repo, &["rev-parse", &parent_rev], None, &[])?
        .trim()
        .to_string();
    let temp = evidence.join(format!("recover-index-{}-{}", std::process::id(), now_ns()));
    fs::create_dir(&temp).map_err(io_err(temp.display()))?;
    let tree = private_tree(repo, &source, &manifest, &temp.join("index"));
    let _ = fs::remove_dir_all(&temp);
    let tree = tree?;
    let existing = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", &args.r#ref])
        .stdin(Stdio::null())
        .output()
        .map_err(io_err("git"))?;
    let commit = if existing.status.success() {
        let commit = String::from_utf8_lossy(&existing.stdout).trim().to_string();
        let current = git(
            repo,
            &["rev-parse", &format!("{commit}^{{tree}}")],
            None,
            &[],
        )?;
        ensure(
            current.trim() == tree,
            "Existing benchmark ref has a different tree; refusing replacement",
        )?;
        commit
    } else {
        let message = format!(
            "Recover the exact 11b benchmark snapshot\n\n\
             Recovered now from the retained pre-slice-12 source export; this is\n\
             not a historical commit from the implementation session.\n\
             Recorded predecessor: {parent}\n\
             Verified source files: {}\n\
             Manifest SHA256: {}\n\
             The source tree is unchanged; provenance is retained in the main\n\
             branch under docs/performance/baselines.\n",
            manifest.as_obj().len(),
            digest(&manifest_path)?
        );
        let commit = git(
            repo,
            &["commit-tree", &tree, "-p", &parent],
            Some(message.into_bytes()),
            &[],
        )?
        .trim()
        .to_string();
        let zero = "0".repeat(commit.len());
        git(
            repo,
            &["update-ref", &args.r#ref, &commit, &zero],
            None,
            &[],
        )?;
        commit
    };
    let meta = obj(vec![
        ("schema", SCHEMA.into()),
        ("ref", args.r#ref.as_str().into()),
        ("commit", commit.into()),
        ("tree", tree.into()),
        ("parent", parent.into()),
        (
            "provenance",
            "Recovered snapshot, committed at recovery time, not historical".into(),
        ),
        ("source_files", manifest.as_obj().len().into()),
        ("source_manifest_sha256", digest(&manifest_path)?.into()),
    ]);
    let copy = evidence.join("11b-source.json");
    if manifest_path != copy {
        fs::copy(&manifest_path, &copy).map_err(io_err(copy.display()))?;
    }
    dump(&evidence.join("11b.json"), &meta)?;
    if args.publish {
        let refspec = format!("{0}:{0}", args.r#ref);
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["push", "origin", &refspec])
            .status()
            .map_err(io_err("git push"))?;
        ensure(
            status.success(),
            format!("git push returned {}", exit_code(status)),
        )?;
    }
    println!("{}", meta.dumps_indent(false, 2));
    Ok(())
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// `statistics.median`: the middle element itself for an odd count, else the mean of the two.
fn median(mut values: Vec<&Json>) -> Json {
    values.sort_by(|a, b| {
        a.as_f64()
            .unwrap_or(f64::NAN)
            .total_cmp(&b.as_f64().unwrap_or(f64::NAN))
    });
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2].clone()
    } else {
        let a = values[n / 2 - 1].as_f64().unwrap_or(f64::NAN);
        let b = values[n / 2].as_f64().unwrap_or(f64::NAN);
        Json::Float((a + b) / 2.0)
    }
}

fn num(row: &Json, key: &str) -> f64 {
    row.field(key).as_f64().unwrap_or(f64::NAN)
}

/// A `(unit, pair)` key of the records.
type Pair = (String, String);

/// Pair rows by `(unit, pair)`, exclude a pair whose command failed or whose start loads differ
/// by more than `load_delta`, and report medians of the kept pairs and of their ratios.
pub fn compare_rows(rows: &[Json], baseline: &str, candidate: &str, load_delta: f64) -> Vec<Json> {
    let key = |row: &Json, name: &str| row.field(name).as_str().unwrap_or("").to_string();
    let mut paired: Vec<(Pair, HashMap<String, &Json>)> = Vec::new();
    let mut units: Vec<String> = Vec::new();
    for row in rows {
        let id = (key(row, "unit"), key(row, "pair"));
        if !units.contains(&id.0) {
            units.push(id.0.clone());
        }
        let slot = match paired.iter().position(|(k, _)| *k == id) {
            Some(n) => n,
            None => {
                paired.push((id, HashMap::new()));
                paired.len() - 1
            }
        };
        paired[slot].1.insert(key(row, "label"), row);
    }
    let mut result = Vec::new();
    for unit in units {
        let mut good: Vec<(&Json, &Json)> = Vec::new();
        let mut excluded = Vec::new();
        for ((name, pair), values) in &paired {
            if *name != unit {
                continue;
            }
            let (Some(before), Some(after)) = (values.get(baseline), values.get(candidate)) else {
                continue;
            };
            let reason = if before.field("exit").truthy() || after.field("exit").truthy() {
                Some("command_failed")
            } else if (num(before, "load_before") - num(after, "load_before")).abs() > load_delta {
                Some("load_mismatch")
            } else {
                None
            };
            match reason {
                Some(reason) => excluded.push(obj(vec![
                    ("pair", pair.as_str().into()),
                    ("reason", reason.into()),
                    (
                        "exits",
                        Json::Arr(vec![
                            before.field("exit").clone(),
                            after.field("exit").clone(),
                        ]),
                    ),
                    (
                        "loads",
                        Json::Arr(vec![
                            before.field("load_before").clone(),
                            after.field("load_before").clone(),
                        ]),
                    ),
                ])),
                None => good.push((before, after)),
            }
        }
        let mut record = obj(vec![
            ("unit", unit.as_str().into()),
            ("baseline", baseline.into()),
            ("valid_pairs", good.len().into()),
            ("excluded", Json::Arr(excluded)),
        ]);
        if !good.is_empty() {
            let ratios: Vec<Json> = good
                .iter()
                .map(|(b, a)| Json::Float(num(a, "seconds") / num(b, "seconds")))
                .collect();
            let loads: Vec<&Json> = good
                .iter()
                .flat_map(|(b, a)| [b.field("load_before"), a.field("load_before")])
                .collect();
            let lowest = loads
                .iter()
                .copied()
                .reduce(|x, y| if num_of(y) < num_of(x) { y } else { x });
            let highest = loads
                .iter()
                .copied()
                .reduce(|x, y| if num_of(y) > num_of(x) { y } else { x });
            record.set(
                "before",
                median(good.iter().map(|(b, _)| b.field("seconds")).collect()),
            );
            record.set(
                "candidate",
                median(good.iter().map(|(_, a)| a.field("seconds")).collect()),
            );
            record.set("ratio", median(ratios.iter().collect()));
            record.set(
                "loads",
                Json::Arr(vec![
                    lowest.cloned().unwrap_or(Json::Null),
                    highest.cloned().unwrap_or(Json::Null),
                ]),
            );
        }
        result.push(record);
    }
    result
}

fn num_of(value: &Json) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

/// Step 1 requires the generator tests to recover their full matched 11b time.
pub fn within_budget(unit: &str, ratio: f64) -> bool {
    ratio <= if unit == "file-rustgen" { 1.0 } else { 1.10 }
}

/// The candidate's format unit runs the frozen source's own parallel entry point when that
/// source still carries `scripts/check-format.py` (sources before the Rust port); every other
/// unit, label and source runs the serial owner.
pub fn gate_command(label: &str, source: &Path, unit: &str, command: &[String]) -> Vec<String> {
    if label == "candidate" && unit == "fmt" && source.join("scripts/check-format.py").is_file() {
        return vec!["python3".into(), "scripts/check-format.py".into()];
    }
    command.to_vec()
}

/// Loose controls are useful diagnostics, never performance acceptance.
pub fn closure_eligible(
    controls: &Json,
    frozen_baseline_matches: bool,
    comparison_present: bool,
) -> bool {
    let value = |key: &str| controls.field(key).as_f64().unwrap_or(f64::NAN);
    frozen_baseline_matches
        && comparison_present
        && 0.0 < value("load_delta")
        && value("load_delta") <= 5.0
        && 0.0 < value("absolute_max_load")
        && value("absolute_max_load") <= 9.0
        && (0.0..=2.0).contains(&value("extra_warm_passes"))
        && (0.0..=1.0).contains(&value("cold_retries"))
        && value("warm_pairs") >= 3.0
        && value("check_pairs") >= 15.0
}

/// Export `commit`'s tree into a new directory, as `git archive` and a `data`-filtered extract
/// did: regular files with modes 0644 or 0755 and the commit time as mtime; a symlink, a path
/// leaving the tree, or a `.gitattributes` with an `export-ignore` or `export-subst` rule (which
/// this does not apply) is refused. A `.gitattributes` without export rules is exported as a
/// file, as `git archive` does.
pub fn safe_export(repo: &Path, commit: &str, destination: &Path) -> R<()> {
    let listing = git(
        repo,
        &["ls-tree", "-r", "-z", "--full-tree", commit],
        None,
        &[],
    )?;
    let mut blobs = Vec::new();
    let mut directories = Vec::new();
    for entry in listing.split('\0').filter(|e| !e.is_empty()) {
        let (head, name) = entry
            .split_once('\t')
            .ok_or_else(|| format!("Unexpected ls-tree entry: {entry}"))?;
        let mut parts = head.split(' ');
        let (mode, kind, object) = (parts.next(), parts.next(), parts.next());
        let path = Path::new(name);
        let unsafe_path = path.is_absolute()
            || path.components().any(|c| c == Component::ParentDir)
            || mode == Some("120000");
        ensure(
            !unsafe_path,
            format!("Unsupported source archive member: {name}"),
        )?;
        if path.file_name() == Some(".gitattributes".as_ref()) {
            let object = object.unwrap_or_default();
            let rules = git(repo, &["cat-file", "blob", object], None, &[])?;
            ensure(
                !rules.contains("export-ignore") && !rules.contains("export-subst"),
                format!("Unsupported source archive member: {name} (export attributes)"),
            )?;
        }
        match (kind, object) {
            (Some("blob"), Some(object)) => {
                blobs.push((name.to_string(), object.to_string(), mode == Some("100755")))
            }
            (Some("commit"), _) => directories.push(name.to_string()),
            _ => return Err(format!("Unsupported source archive member: {name}")),
        }
    }
    let stamp: u64 = git(repo, &["log", "-1", "--format=%ct", commit], None, &[])?
        .trim()
        .parse()
        .map_err(|e| format!("commit time of {commit}: {e}"))?;
    let mtime = UNIX_EPOCH + Duration::from_secs(stamp);
    fs::create_dir_all(destination.parent().unwrap_or(Path::new("/")))
        .map_err(io_err(destination.display()))?;
    fs::create_dir(destination).map_err(io_err(destination.display()))?;
    for name in directories {
        fs::create_dir_all(destination.join(&name)).map_err(io_err(&name))?;
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(io_err("git cat-file"))?;
    let requests: String = blobs.iter().map(|(_, o, _)| format!("{o}\n")).collect();
    let mut stdin = child.stdin.take().expect("piped stdin");
    let writer = std::thread::spawn(move || stdin.write_all(requests.as_bytes()));
    let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
    for (name, object, executable) in &blobs {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(io_err("git cat-file"))?;
        let size: usize = header
            .trim_end()
            .strip_prefix(&format!("{object} blob "))
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("Unexpected cat-file header for {name}: {header}"))?;
        let mut content = vec![0; size + 1];
        reader
            .read_exact(&mut content)
            .map_err(io_err("git cat-file"))?;
        content.pop();
        let path = destination.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent.display()))?;
        }
        let file = fs::File::create(&path).map_err(io_err(path.display()))?;
        (&file)
            .write_all(&content)
            .map_err(io_err(path.display()))?;
        let mode = if *executable { 0o755 } else { 0o644 };
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(io_err(path.display()))?;
        file.set_modified(mtime).map_err(io_err(path.display()))?;
    }
    writer
        .join()
        .map_err(|_| "cat-file writer panicked".to_string())?
        .map_err(io_err("git cat-file"))?;
    drop(reader);
    let status = child.wait().map_err(io_err("git cat-file"))?;
    ensure(status.success(), "git cat-file --batch failed")
}

/// Test counts, inline counts and executable paths read from a command's log.
#[derive(Debug, PartialEq)]
pub struct LogCounts {
    pub passed: i64,
    pub failed: i64,
    pub ignored: i64,
    pub inline: Vec<i64>,
    pub executables: Vec<String>,
}

fn leading_number(text: &str) -> Option<(i64, &str)> {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    Some((text[..end].parse().ok()?, &text[end..]))
}

fn counted<'a>(text: &'a str, word: &str) -> Option<(i64, &'a str)> {
    let (n, rest) = leading_number(text)?;
    Some((n, rest.strip_prefix(word)?))
}

/// The harness's three patterns: `test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored`,
/// `test result: ok\. (\d+) passed; 0 failed; 0 filtered out` and `Executable [^\n]*\(([^\n]+)\)`.
pub fn log_counts(text: &str) -> LogCounts {
    let mut counts = LogCounts {
        passed: 0,
        failed: 0,
        ignored: 0,
        inline: Vec::new(),
        executables: Vec::new(),
    };
    const RESULT: &str = "test result: ok. ";
    for (at, _) in text.match_indices(RESULT) {
        let rest = &text[at + RESULT.len()..];
        let cargo = counted(rest, " passed; ").and_then(|(p, rest)| {
            let (f, rest) = counted(rest, " failed; ")?;
            let (i, _) = counted(rest, " ignored")?;
            Some((p, f, i))
        });
        if let Some((p, f, i)) = cargo {
            counts.passed += p;
            counts.failed += f;
            counts.ignored += i;
        }
        if let Some((p, _)) = counted(rest, " passed; 0 failed; 0 filtered out") {
            counts.inline.push(p);
        }
    }
    for line in text.split('\n') {
        let Some(at) = line.find("Executable ") else {
            continue;
        };
        let rest = &line[at + "Executable ".len()..];
        let Some(close) = rest.rfind(')') else {
            continue;
        };
        if let Some(open) = rest[..close].rfind('(')
            && open + 1 < close
        {
            counts.executables.push(rest[open + 1..close].to_string());
        }
    }
    counts
}

/// One measured command, before the capture fields are added.
pub struct Measured<'a> {
    pub label: &'a str,
    pub commit: &'a str,
    pub unit: &'a str,
    pub pair: &'a str,
    pub phase: &'a str,
    pub recache: bool,
    pub command: &'a [String],
    pub seconds: f64,
    pub exit: i64,
    pub load_before: f64,
    pub load_after: f64,
    pub log: &'a str,
    pub text: &'a str,
}

/// The record of one command, with the fields of the Python harness.
pub fn command_row(m: &Measured) -> Json {
    let counts = log_counts(m.text);
    let mut row = obj(vec![
        ("schema", SCHEMA.into()),
        ("label", m.label.into()),
        ("commit", m.commit.into()),
        ("unit", m.unit.into()),
        ("pair", m.pair.into()),
        ("phase", m.phase.into()),
        (
            "cache_policy",
            if m.recache { "recache" } else { "normal" }.into(),
        ),
        ("command", m.command.to_vec().into()),
        ("seconds", m.seconds.into()),
        ("exit", m.exit.into()),
        ("load_before", m.load_before.into()),
        ("load_after", m.load_after.into()),
        ("log", m.log.into()),
        (
            "tests",
            obj(vec![
                ("passed", counts.passed.into()),
                ("failed", counts.failed.into()),
                ("ignored", counts.ignored.into()),
            ]),
        ),
    ]);
    if !counts.inline.is_empty() {
        row.set("inline_counts", counts.inline.clone().into());
    }
    if !counts.executables.is_empty() {
        let bytes: u64 = counts
            .executables
            .iter()
            .filter_map(|name| fs::metadata(name).ok())
            .map(|meta| meta.len())
            .sum();
        row.set(
            "executables",
            obj(vec![
                ("count", counts.executables.len().into()),
                ("bytes", Json::Int(bytes as i64)),
            ]),
        );
    }
    row
}

/// The one-line progress record printed after each appended row, in the Python key order.
pub fn progress_line(row: &Json) -> String {
    let keys = ["label", "unit", "pair", "seconds", "exit", "load_before"];
    Json::Obj(
        keys.iter()
            .map(|k| (k.to_string(), row.field(k).clone()))
            .collect(),
    )
    .dumps(false)
}

pub fn failed_candidate(rows: &[Json]) -> bool {
    rows.iter()
        .any(|r| r.field("label").as_str() == Some("candidate") && r.field("exit").truthy())
}

/// One configured source: label, ref, commit, export and owned build directory.
#[derive(Clone, Debug)]
pub struct Side {
    pub label: String,
    pub reference: String,
    pub commit: String,
    pub path: PathBuf,
    pub target: PathBuf,
}

pub const PREPARED_ARTIFACTS: [&str; 3] = [
    "debug/ignite",
    "ignite-test/rt/debug/libignite_rt.a",
    "ignite-test/rt/release/libignite_rt.a",
];

/// Reuse only a complete, successful preparation: for every side the latest record of each
/// preparation unit exited 0 for its commit in one pair and one capture, no later `clean`, and
/// the compiler and both runner archives present and nonempty inside its target.
pub fn is_prepared(rows: &[Json], sides: &[Side]) -> bool {
    let mut units: BTreeSet<String> = gate()
        .iter()
        .map(|(name, _)| format!("prepare-{name}"))
        .collect();
    units.insert("gate-prepare".into());
    units.insert("runtime-release".into());
    for side in sides {
        let mut latest: BTreeMap<String, (usize, &Json)> = BTreeMap::new();
        let mut last_clean: i64 = -1;
        for (index, row) in rows.iter().enumerate() {
            if row.field("label").as_str() == Some(side.label.as_str()) {
                let unit = row.field("unit").as_str().unwrap_or("");
                if units.contains(unit) {
                    latest.insert(unit.to_string(), (index, row));
                }
                if unit == "clean" {
                    last_clean = index as i64;
                }
            }
        }
        if latest.len() != units.len() {
            return false;
        }
        let first = latest.values().map(|(i, _)| *i).min().unwrap_or(0) as i64;
        let pairs: BTreeSet<String> = latest
            .values()
            .map(|(_, r)| r.field("pair").dumps(true))
            .collect();
        let captures: BTreeSet<String> = latest
            .values()
            .map(|(_, r)| r.field("capture").dumps(true))
            .collect();
        let bad = latest.values().any(|(_, r)| {
            r.field("exit").truthy()
                || r.field("commit").as_str() != Some(side.commit.as_str())
                || r.field("phase").as_str() != Some("prepare")
        });
        if bad
            || pairs.len() != 1
            || captures.len() != 1
            || captures.contains("null")
            || last_clean > first
        {
            return false;
        }
        let root = side
            .target
            .canonicalize()
            .unwrap_or_else(|_| resolve(&side.target));
        for name in PREPARED_ARTIFACTS {
            let path = side.target.join(name);
            let ok = fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() > 0)
                && path.canonicalize().is_ok_and(|p| p.starts_with(&root));
            if !ok {
                return false;
            }
        }
    }
    true
}

/// Whether `warm` must run the full preparation; a candidate failure blocks the batch.
pub fn needs_prepare(rows: &[Json], sides: &[Side]) -> R<bool> {
    ensure(
        !failed_candidate(rows),
        "Candidate failure in this batch; fix and freeze a new candidate",
    )?;
    Ok(!is_prepared(rows, sides))
}

/// Inputs of `summarize` besides the records.
pub struct SummaryInputs<'a> {
    pub commits: Json,
    pub has_comparison: bool,
    pub controls: &'a Json,
    pub warm_pairs: i64,
    pub check_pairs: i64,
    pub absolute_max_load: f64,
    pub load_delta: f64,
    pub frozen_baseline_matches: bool,
}

/// The units whose comparison acceptance requires.
pub fn required_units() -> BTreeSet<String> {
    let mut units = BTreeSet::new();
    for (name, _) in gate() {
        units.insert(format!("cold-{name}"));
        units.insert(format!("warm-{name}"));
    }
    for unit in [
        "gate-cold",
        "gate-warm",
        "no-run-warm",
        "no-run-touch",
        "file-ignite_test",
        "file-rustgen",
        "check",
        "plan",
    ] {
        units.insert(unit.into());
    }
    for target in ["rust", "llvm"] {
        for profile in ["dev", "release"] {
            units.insert(format!("todod-{target}-{profile}"));
        }
    }
    units
}

/// `summary.json` and `summary.md` from all records: per-unit comparisons against the frozen
/// baseline (and the published comparison), missing and over-budget units, the absolute goals,
/// coverage and the acceptance verdict.
pub fn summarize_rows(rows: &[Json], inputs: &SummaryInputs) -> (Json, String) {
    let measured: Vec<Json> = rows
        .iter()
        .filter(|r| r.field("phase").as_str() == Some("measured"))
        .cloned()
        .collect();
    let mut results = compare_rows(&measured, "baseline", "candidate", inputs.load_delta);
    if inputs.has_comparison {
        results.extend(compare_rows(
            &measured,
            "comparison",
            "candidate",
            inputs.load_delta,
        ));
    }
    let find = |baseline: &str, unit: &str| {
        results.iter().find(|r| {
            r.field("baseline").as_str() == Some(baseline) && r.field("unit").as_str() == Some(unit)
        })
    };
    let needed = |unit: &str| -> i64 {
        if unit.starts_with("cold-") || unit == "gate-cold" {
            1
        } else if unit == "check" || unit == "plan" {
            inputs.check_pairs.max(15)
        } else {
            inputs.warm_pairs.max(3)
        }
    };
    let valid = |row: &Json| row.field("valid_pairs").as_i64().unwrap_or(0);
    let get_f = |row: &Json, key: &str| row.field(key).as_f64().unwrap_or(f64::INFINITY);
    let required = required_units();
    let (mut missing, mut over) = (Vec::new(), Vec::new());
    for unit in &required {
        match find("baseline", unit) {
            Some(row) if valid(row) >= needed(unit) => {
                if !within_budget(unit, get_f(row, "ratio")) {
                    over.push(unit.clone());
                }
            }
            _ => missing.push(unit.clone()),
        }
    }
    let workspace = find("baseline", "warm-workspace");
    let absolute = workspace.is_some_and(|w| get_f(w, "candidate") <= 10.8);
    let low_load = workspace.is_some_and(|w| {
        w.field("loads").item(1).as_f64().unwrap_or(f64::INFINITY) <= inputs.absolute_max_load
    });
    let file_ok =
        find("baseline", "file-ignite_test").is_some_and(|f| get_f(f, "candidate") <= 4.6);
    let rustgen_ok = find("baseline", "file-rustgen").is_some_and(|r| {
        valid(r) >= inputs.warm_pairs.max(3) && within_budget("file-rustgen", get_f(r, "ratio"))
    });
    let mut comparison_missing = Vec::new();
    if inputs.has_comparison {
        for unit in &required {
            if !find("comparison", unit).is_some_and(|row| valid(row) >= needed(unit)) {
                comparison_missing.push(unit.clone());
            }
        }
    } else {
        comparison_missing.push("published comparison not configured".to_string());
    }
    let candidate_rows = |suffix: &str| -> Vec<&Json> {
        measured
            .iter()
            .filter(|r| {
                r.field("label").as_str() == Some("candidate")
                    && r.field("unit")
                        .as_str()
                        .is_some_and(|u| u.ends_with(suffix))
            })
            .collect()
    };
    let workspaces = candidate_rows("workspace");
    let inlines = candidate_rows("inline");
    let coverage = !workspaces.is_empty()
        && !inlines.is_empty()
        && workspaces.iter().all(|r| {
            let tests = r.field("tests");
            tests.field("passed").as_f64().unwrap_or(0.0) >= 647.0
                && tests.field("ignored").as_f64() == Some(5.0)
        })
        && inlines.iter().all(|r| {
            let counts = r.field("inline_counts").as_arr();
            counts.len() == 2
                && counts
                    .iter()
                    .map(|c| c.as_f64().unwrap_or(0.0))
                    .fold(f64::INFINITY, f64::min)
                    >= 148.0
        });
    let failed = rows.iter().any(|r| r.field("exit").truthy());
    let candidate_failed = failed_candidate(rows);
    let controls_pass = closure_eligible(
        inputs.controls,
        inputs.frozen_baseline_matches,
        inputs.has_comparison,
    );
    let accepted = controls_pass
        && rustgen_ok
        && missing.is_empty()
        && comparison_missing.is_empty()
        && over.is_empty()
        && absolute
        && low_load
        && file_ok
        && coverage
        && !candidate_failed;
    let mut lines = vec![
        "# Performance comparison".to_string(),
        String::new(),
        "unit | baseline | before s | candidate s | ratio | valid/excluded".into(),
        "---|---|---:|---:|---:|---:".into(),
    ];
    for row in &results {
        if row.get("ratio").is_some() {
            lines.push(format!(
                "{} | {} | {:.6} | {:.6} | {:.4} | {}/{}",
                row.field("unit").as_str().unwrap_or(""),
                row.field("baseline").as_str().unwrap_or(""),
                get_f(row, "before"),
                get_f(row, "candidate"),
                get_f(row, "ratio"),
                valid(row),
                row.field("excluded").as_arr().len()
            ));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "Accepted: {}",
        if accepted { "True" } else { "False" }
    ));
    lines.push(format!("Missing evidence: {}", missing.join(", ")));
    lines.push(format!("Over budget: {}", over.join(", ")));
    let summary = obj(vec![
        ("schema", SCHEMA.into()),
        ("commits", inputs.commits.clone()),
        ("results", Json::Arr(results.clone())),
        ("missing", missing.into()),
        ("over_budget", over.into()),
        ("warm_workspace_absolute_pass", absolute.into()),
        ("comparable_low_load", low_load.into()),
        ("ignite_test_absolute_pass", file_ok.into()),
        ("rustgen_11b_pass", rustgen_ok.into()),
        ("candidate_failed", candidate_failed.into()),
        ("any_command_failed", failed.into()),
        (
            "frozen_baseline_matches",
            inputs.frozen_baseline_matches.into(),
        ),
        ("closure_controls_pass", controls_pass.into()),
        ("coverage_pass", coverage.into()),
        ("comparison_missing", comparison_missing.into()),
        ("accepted", accepted.into()),
    ]);
    (summary, lines.join("\n") + "\n")
}

/// Python `str.splitlines()` line count.
pub fn line_count(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars().peekable();
    let mut open = false;
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                count += 1;
                open = false;
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => {
                count += 1;
                open = false;
            }
            _ => open = true,
        }
    }
    count + usize::from(open)
}

/// The text of `check_output([...], stderr=STDOUT, text=True).strip()`, or the Python error
/// text when the tool is missing or fails, as `tools.json` records it.
fn tool_version(command: &[&str]) -> R<String> {
    let (reader, writer) = std::io::pipe().map_err(io_err("pipe"))?;
    let spawned = {
        let mut process = Command::new(command[0]);
        process.args(&command[1..]).stdin(Stdio::null());
        let clone = writer.try_clone().map_err(io_err("pipe"))?;
        process.stdout(writer).stderr(clone);
        process.spawn()
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(format!(
                "[Errno 2] No such file or directory: '{}'",
                command[0]
            ));
        }
        Err(error) => return Err(format!("{}: {error}", command[0])),
    };
    let mut text = Vec::new();
    let mut reader = reader;
    reader.read_to_end(&mut text).map_err(io_err(command[0]))?;
    let status = child.wait().map_err(io_err(command[0]))?;
    if !status.success() {
        let words: Vec<String> = command.iter().map(|w| w.to_string()).collect();
        return Ok(format!(
            "Command '{}' returned non-zero exit status {}.",
            py_list(&words),
            exit_code(status)
        ));
    }
    Ok(String::from_utf8_lossy(&text).trim().to_string())
}

pub struct Harness {
    pub args: RunArgs,
    pub repo: PathBuf,
    pub sources: PathBuf,
    pub builds: PathBuf,
    pub evidence: PathBuf,
    pub sides: Vec<Side>,
    pub frozen_baseline_matches: bool,
    pub env_base: BTreeMap<OsString, OsString>,
    pub records_path: PathBuf,
    pub rows: Vec<Json>,
    pub controls: Json,
    pub capture_id: String,
    pub harness_sha256: String,
}

const ENVIRONMENT: [&str; 12] = [
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_RELEASE_DEBUG",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTUP_TOOLCHAIN",
    "CARGO_BUILD_JOBS",
    "CARGO_BUILD_TARGET",
    "CARGO_NET_OFFLINE",
    "SCCACHE_DISABLE",
];

impl Harness {
    pub fn new(args: RunArgs) -> R<Harness> {
        let repo = resolve(&args.repo);
        let (sources, builds, evidence) = (
            resolve(&args.sources),
            resolve(&args.builds),
            resolve(&args.evidence),
        );
        let roots = [&sources, &builds, &evidence];
        ensure(
            sources != builds && builds != evidence && sources != evidence,
            "Source/build/evidence roots must be distinct",
        )?;
        for a in roots {
            for b in roots {
                ensure(
                    a == b || !(b.starts_with(a) || a.starts_with(b)),
                    "Source/build/evidence roots must be separate, non-nested directories",
                )?;
            }
        }
        for root in roots {
            fs::create_dir_all(root).map_err(io_err(root.display()))?;
        }
        let mut refs = vec![
            ("baseline", args.baseline.clone()),
            ("candidate", args.candidate.clone()),
        ];
        if !args.comparison.is_empty() {
            refs.push(("comparison", args.comparison.clone()));
        }
        let mut sides = Vec::new();
        for (label, reference) in refs {
            let commit = git(
                &repo,
                &["rev-parse", &format!("{reference}^{{commit}}")],
                None,
                &[],
            )?
            .trim()
            .to_string();
            let short = &commit[..commit.len().min(12)];
            sides.push(Side {
                label: label.into(),
                reference,
                path: sources.join(format!("{label}-{short}")),
                target: builds.join(format!("{label}-{short}")),
                commit,
            });
        }
        let baseline_metadata = repo.join("docs/performance/baselines/11b.json");
        let expected_tree = if baseline_metadata.exists() {
            read_json(&baseline_metadata)?
                .field("tree")
                .as_str()
                .map(String::from)
        } else {
            None
        };
        let actual_tree = git(
            &repo,
            &["rev-parse", &format!("{}^{{tree}}", sides[0].commit)],
            None,
            &[],
        )?
        .trim()
        .to_string();
        let frozen_baseline_matches = expected_tree.as_deref() == Some(actual_tree.as_str());
        let mut env_base: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
        for (key, value) in [
            ("CARGO_INCREMENTAL", "0"),
            ("CARGO_PROFILE_DEV_DEBUG", "line-tables-only"),
            ("CARGO_PROFILE_RELEASE_DEBUG", "0"),
            ("CARGO_TERM_COLOR", "never"),
        ] {
            env_base.insert(key.into(), value.into());
        }
        env_base.remove(&OsString::from("CARGO_TARGET_DIR"));
        env_base.remove(&OsString::from("SCCACHE_RECACHE"));
        let records_path = evidence.join("records.jsonl");
        let mut rows = Vec::new();
        if records_path.exists() {
            let text = fs::read_to_string(&records_path).map_err(io_err(records_path.display()))?;
            for line in text.lines() {
                rows.push(Json::parse(line).map_err(|e| format!("records.jsonl: {e}"))?);
            }
        }
        let controls = args.controls();
        let by_label = |f: &dyn Fn(&Side) -> Json| {
            Json::Obj(sides.iter().map(|s| (s.label.clone(), f(s))).collect())
        };
        let environment = Json::Obj(
            ENVIRONMENT
                .iter()
                .map(|key| {
                    let value = env_base
                        .get(&OsString::from(key))
                        .map(|v| Json::Str(v.to_string_lossy().into_owned()));
                    (key.to_string(), value.unwrap_or(Json::Null))
                })
                .collect(),
        );
        let config = obj(vec![
            ("schema", SCHEMA.into()),
            ("commits", by_label(&|s| s.commit.as_str().into())),
            ("refs", by_label(&|s| s.reference.as_str().into())),
            ("controls", controls.clone()),
            ("sources", path_str(&sources).into()),
            ("builds", path_str(&builds).into()),
            ("load_delta", args.load_delta_json()),
            ("environment", environment),
        ]);
        let metadata = evidence.join("run.json");
        if metadata.exists() {
            ensure(
                read_json(&metadata)? == config,
                "Evidence belongs to another configuration; use a new evidence directory",
            )?;
        }
        dump(&metadata, &config)?;
        for side in &sides {
            let manifest_path = evidence.join(format!("source-{}.json", side.label));
            if !side.path.exists() {
                safe_export(&repo, &side.commit, &side.path)?;
                dump(&manifest_path, &source_manifest(&side.path)?)?;
            } else {
                ensure(
                    manifest_path.exists(),
                    format!("Unowned source directory: {}", side.path.display()),
                )?;
            }
            verify_manifest(&side.path, &read_json(&manifest_path)?)?;
            let owner = owner_path(&side.target);
            let expected = obj(vec![
                ("schema", SCHEMA.into()),
                ("commit", side.commit.as_str().into()),
                ("source", path_str(&side.path).into()),
            ]);
            ensure(
                !side.target.exists() || owner.exists(),
                format!("Unowned build directory: {}", side.target.display()),
            )?;
            if owner.exists() {
                ensure(
                    read_json(&owner)? == expected,
                    format!("Build ownership mismatch: {}", owner.display()),
                )?;
            }
            dump(&owner, &expected)?;
        }
        let mut tools = Vec::new();
        for (name, command) in [
            ("rustc", &["rustc", "-Vv"][..]),
            ("cargo", &["cargo", "-V"]),
            ("clang", &["clang", "--version"]),
            ("task", &["task", "--version"]),
            ("sccache", &["sccache", "--version"]),
        ] {
            tools.push((name.to_string(), Json::Str(tool_version(command)?)));
        }
        let tools = Json::Obj(tools);
        let tool_path = evidence.join("tools.json");
        if tool_path.exists() {
            ensure(
                read_json(&tool_path)? == tools,
                "Tool versions changed; do not mix toolchains in one comparison",
            )?;
        }
        dump(&tool_path, &tools)?;
        let bundle = crate::harness_bundle();
        let harness_sha256 = sha256(bundle.as_bytes());
        let harness_source = evidence
            .join("harness")
            .join(format!("{harness_sha256}.rs"));
        fs::create_dir_all(evidence.join("harness")).map_err(io_err("harness"))?;
        if !harness_source.exists() {
            fs::write(&harness_source, bundle).map_err(io_err(harness_source.display()))?;
        }
        Ok(Harness {
            args,
            repo,
            sources,
            builds,
            evidence,
            sides,
            frozen_baseline_matches,
            env_base,
            records_path,
            rows,
            controls,
            capture_id: now_ns().to_string(),
            harness_sha256,
        })
    }

    pub(crate) fn side(&self, label: &str) -> &Side {
        self.sides
            .iter()
            .find(|s| s.label == label)
            .expect("configured label")
    }

    pub(crate) fn labels(&self) -> Vec<String> {
        self.sides.iter().map(|s| s.label.clone()).collect()
    }

    fn has_comparison(&self) -> bool {
        self.sides.iter().any(|s| s.label == "comparison")
    }

    pub(crate) fn guard(&self) -> R<()> {
        let stat = nix::sys::statvfs::statvfs(&self.builds).map_err(|e| format!("statvfs: {e}"))?;
        let free =
            stat.blocks_available() as f64 * stat.fragment_size() as f64 / (1u64 << 30) as f64;
        let limit = self.args.min_free_gib();
        let shown = self.args.min_free_gib.map_or(Json::Int(20), Json::Float);
        ensure(
            free >= limit,
            format!(
                "Disk guard: {free:.1} GiB available, requires {}",
                shown.dumps(false)
            ),
        )
    }

    pub(crate) fn append(&mut self, mut row: Json) -> R<Json> {
        row.set("capture", self.capture_id.as_str().into());
        row.set("harness_sha256", self.harness_sha256.as_str().into());
        let mut stream = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.records_path)
            .map_err(io_err(self.records_path.display()))?;
        writeln!(stream, "{}", row.dumps(true)).map_err(io_err(self.records_path.display()))?;
        println!("{}", progress_line(&row));
        self.rows.push(row.clone());
        Ok(row)
    }

    pub(crate) fn command(
        &mut self,
        label: &str,
        unit: &str,
        pair: &str,
        command: Vec<String>,
        recache: bool,
        phase: &str,
    ) -> R<Json> {
        self.guard()?;
        let logs = self.evidence.join("logs");
        fs::create_dir_all(&logs).map_err(io_err(logs.display()))?;
        let log = logs.join(format!("{}-{pair}-{label}-{unit}.log", self.capture_id));
        let side = self.side(label).clone();
        let before = loadavg()?;
        let start = Instant::now();
        let stream = fs::File::create(&log).map_err(io_err(log.display()))?;
        let errors = stream.try_clone().map_err(io_err(log.display()))?;
        let mut env = self.env_base.clone();
        env.insert("CARGO_TARGET_DIR".into(), side.target.clone().into());
        if recache {
            env.insert("SCCACHE_RECACHE".into(), "1".into());
        }
        let status = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&side.path)
            .env_clear()
            .envs(&env)
            .stdin(Stdio::inherit())
            .stdout(stream)
            .stderr(errors)
            .status()
            .map_err(io_err(&command[0]))?;
        let seconds = start.elapsed().as_secs_f64();
        let bytes = fs::read(&log).map_err(io_err(log.display()))?;
        let text = String::from_utf8_lossy(&bytes);
        let relative = path_str(
            log.strip_prefix(&self.evidence)
                .expect("log under evidence"),
        );
        let row = command_row(&Measured {
            label,
            commit: &side.commit,
            unit,
            pair,
            phase,
            recache,
            command: &command,
            seconds,
            exit: exit_code(status),
            load_before: before,
            load_after: loadavg()?,
            log: &relative,
            text: &text,
        });
        self.append(row)
    }

    fn comparisons(&self) -> Vec<Json> {
        let delta = self.args.load_delta();
        let mut values = compare_rows(&self.rows, "baseline", "candidate", delta);
        if self.has_comparison() {
            values.extend(compare_rows(&self.rows, "comparison", "candidate", delta));
        }
        values
    }

    fn failed_candidate(&self) -> bool {
        failed_candidate(&self.rows)
    }

    fn gate(&mut self, label: &str, pair: &str, mode: &str, recache: bool, phase: &str) -> R<Json> {
        let before = loadavg()?;
        let start = Instant::now();
        let mut failures = 0i64;
        let side = self.side(label).clone();
        for (name, command) in gate() {
            let command = gate_command(label, &side.path, name, &command);
            let row = self.command(
                label,
                &format!("{mode}-{name}"),
                pair,
                command,
                recache,
                phase,
            )?;
            failures += i64::from(row.field("exit").truthy());
            if label == "candidate" && name == "workspace" {
                let journal = side.target.join("tmp/run-state-item2-real/journal.jsonl");
                if journal.exists() {
                    let destination = self
                        .evidence
                        .join(format!("restart-{}-{pair}.jsonl", self.capture_id));
                    fs::copy(&journal, &destination).map_err(io_err(destination.display()))?;
                }
            }
        }
        let row = obj(vec![
            ("schema", SCHEMA.into()),
            ("label", label.into()),
            ("commit", side.commit.as_str().into()),
            ("unit", format!("gate-{mode}").into()),
            ("pair", pair.into()),
            ("phase", phase.into()),
            ("seconds", start.elapsed().as_secs_f64().into()),
            ("exit", failures.into()),
            ("load_before", before.into()),
            ("load_after", loadavg()?.into()),
        ]);
        self.append(row)
    }

    pub fn ensure_prepared(&mut self) -> R<()> {
        if needs_prepare(&self.rows, &self.sides)? {
            self.prepare()?;
        }
        Ok(())
    }

    pub fn prepare(&mut self) -> R<()> {
        let pair = format!("prepare-{}", self.capture_id);
        for label in self.labels() {
            self.gate(&label, &pair, "prepare", false, "prepare")?;
            let rt = path_str(&self.side(&label).target.join("ignite-test/rt"));
            let command = words(&[
                "cargo",
                "build",
                "-q",
                "--release",
                "-p",
                "ignite-rt",
                "--features",
                "runner",
                "--target-dir",
                &rt,
            ]);
            self.command(&label, "runtime-release", &pair, command, false, "prepare")?;
        }
        ensure(
            !self.failed_candidate(),
            "Candidate failure during preparation; fix and freeze a new candidate",
        )
    }

    pub fn smoke(&mut self) -> R<()> {
        for label in self.labels() {
            let build = words(&["cargo", "build", "-q", "-p", "ignite"]);
            self.command(&label, "smoke-compiler", "smoke", build, false, "smoke")?;
            let compiler = path_str(&self.side(&label).target.join("debug/ignite"));
            let outputs: Vec<PathBuf> = [1, 2]
                .iter()
                .map(|n| self.evidence.join(format!("smoke-{label}-{n}.json")))
                .collect();
            for (number, output) in outputs.iter().enumerate() {
                let command = words(&[
                    &compiler,
                    "plan",
                    "examples/todo/todo.ignite",
                    "--out",
                    &path_str(output),
                ]);
                self.command(
                    &label,
                    "smoke-plan",
                    &format!("smoke-{number}"),
                    command,
                    false,
                    "smoke",
                )?;
            }
            let same = match (fs::read(&outputs[0]), fs::read(&outputs[1])) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            };
            ensure(
                same,
                format!("Non-deterministic or failed smoke plan: {label}"),
            )?;
            let output = self
                .builds
                .join("smoke")
                .join(&self.capture_id)
                .join(&label);
            let command = words(&[
                &compiler,
                "build",
                "examples/todo/todo.ignite",
                "--target",
                "rust",
                "--out",
                &path_str(&output.join("program")),
                "--target-dir",
                &path_str(&output.join("cargo")),
            ]);
            self.command(&label, "smoke-build", "smoke", command, false, "smoke")?;
        }
        Ok(())
    }

    pub fn cold(&mut self) -> R<()> {
        for attempt in 0..=self.args.cold_retries {
            let pair = format!("{}-cold-{}", self.capture_id, attempt + 1);
            for label in self.labels() {
                let target = self.side(&label).target.clone();
                if target.exists() {
                    let command = words(&["cargo", "clean", "--target-dir", &path_str(&target)]);
                    let cleaned =
                        self.command(&label, "clean", &pair, command, false, "prepare")?;
                    ensure(
                        !cleaned.field("exit").truthy(),
                        "Cold preparation failed; no cold claim is valid",
                    )?;
                }
                self.gate(&label, &pair, "cold", true, "measured")?;
                ensure(
                    !self.failed_candidate(),
                    "Candidate cold failure; freeze corrected source before restarting",
                )?;
            }
            let comparisons = self.comparisons();
            let gates: Vec<&Json> = comparisons
                .iter()
                .filter(|r| {
                    let unit = r.field("unit").as_str().unwrap_or("");
                    unit == "gate-cold" || unit.starts_with("cold-")
                })
                .collect();
            if !gates.is_empty() && gates.iter().all(|g| g.field("valid_pairs").truthy()) {
                return Ok(());
            }
            let baseline_failed = self.rows.iter().any(|r| {
                r.field("label").as_str() == Some("baseline")
                    && r.field("pair").as_str() == Some(pair.as_str())
                    && r.field("exit").truthy()
            });
            if baseline_failed {
                return Ok(());
            }
        }
        Ok(())
    }

    fn enough(&self, units: impl Fn(&str) -> bool, count: i64) -> bool {
        self.comparisons()
            .iter()
            .filter(|r| units(r.field("unit").as_str().unwrap_or("")))
            .all(|r| r.field("valid_pairs").as_i64().unwrap_or(0) >= count)
    }

    pub fn warm(&mut self) -> R<()> {
        self.ensure_prepared()?;
        let passes = self.args.warm_pairs + self.args.extra_warm_passes;
        for number in 1..=passes {
            let pair = format!("{}-warm-{number}", self.capture_id);
            for label in self.labels() {
                let lib = self
                    .side(&label)
                    .path
                    .join("crates/ignite-check/src/lib.rs");
                touch(&lib)?;
                self.gate(&label, &pair, "warm", false, "measured")?;
                ensure(
                    !self.failed_candidate(),
                    "Candidate warm failure; do not collect greens around it",
                )?;
                let no_run = words(&["cargo", "test", "--workspace", "--no-run"]);
                self.command(
                    &label,
                    "no-run-warm",
                    &pair,
                    no_run.clone(),
                    false,
                    "measured",
                )?;
                touch(&lib)?;
                self.command(&label, "no-run-touch", &pair, no_run, false, "measured")?;
                let file = words(&[
                    "cargo",
                    "test",
                    "-p",
                    "ignite",
                    "--test",
                    "main",
                    "ignite_test::",
                ]);
                self.command(&label, "file-ignite_test", &pair, file, false, "measured")?;
                let rustgen = words(&["cargo", "test", "-p", "ignite-rustgen", "--test", "main"]);
                self.command(&label, "file-rustgen", &pair, rustgen, false, "measured")?;
            }
            let needed = |unit: &str| {
                unit.starts_with("warm-")
                    || matches!(
                        unit,
                        "gate-warm"
                            | "no-run-warm"
                            | "no-run-touch"
                            | "file-ignite_test"
                            | "file-rustgen"
                    )
            };
            if number >= self.args.warm_pairs && self.enough(needed, self.args.warm_pairs) {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn programs(&mut self) -> R<()> {
        let passes = self.args.warm_pairs + self.args.extra_warm_passes;
        for pair in 1..=passes {
            let pair_name = format!("{}-programs-{pair}", self.capture_id);
            for profile in ["dev", "release"] {
                for target in ["rust", "llvm"] {
                    for label in self.labels() {
                        let side = self.side(&label).clone();
                        let compiler = path_str(&side.target.join("debug/ignite"));
                        let rt = side.target.join("ignite-test/rt");
                        if target == "llvm" {
                            let mut command = words(&[
                                "cargo",
                                "build",
                                "-q",
                                "-p",
                                "ignite-rt",
                                "--features",
                                "runner",
                                "--target-dir",
                                &path_str(&rt),
                            ]);
                            if profile == "release" {
                                command.push("--release".into());
                            }
                            self.command(
                                &label,
                                &format!("runtime-{profile}"),
                                &pair_name,
                                command,
                                false,
                                "prepare",
                            )?;
                        }
                        let output = self
                            .builds
                            .join("programs")
                            .join(&self.capture_id)
                            .join(format!("{pair}-{label}-{target}-{profile}"));
                        let mut command = words(&[
                            &compiler,
                            "build",
                            "examples/todo/todo.ignite",
                            "--out",
                            &path_str(&output),
                        ]);
                        if profile == "release" {
                            command.push("--release".into());
                        }
                        if target == "rust" {
                            command.extend(words(&["--target", "rust", "--target-dir"]));
                            command.push(format!("{}-cargo", path_str(&output)));
                        } else {
                            let dir = if profile == "release" {
                                "release"
                            } else {
                                "debug"
                            };
                            command.extend(words(&[
                                "--target",
                                "llvm",
                                "--runtime",
                                &path_str(&rt.join(dir).join("libignite_rt.a")),
                            ]));
                        }
                        let unit = format!("todod-{target}-{profile}");
                        self.command(&label, &unit, &pair_name, command, true, "measured")?;
                        let mut sizes = Vec::new();
                        for name in ["std/src/lib.rs", "std.ll", "program.ll"] {
                            let path = output.join(name);
                            if path.exists() {
                                let text =
                                    fs::read_to_string(&path).map_err(io_err(path.display()))?;
                                sizes.push((name.to_string(), Json::from(line_count(&text))));
                            }
                        }
                        let file = self
                            .evidence
                            .join(format!("program-{pair}-{label}-{target}-{profile}.json"));
                        dump(&file, &Json::Obj(sizes))?;
                    }
                }
            }
            if pair >= self.args.warm_pairs
                && self.enough(|u| u.starts_with("todod-"), self.args.warm_pairs)
            {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn cli(&mut self) -> R<()> {
        let passes = self.args.check_pairs + self.args.extra_warm_passes;
        for number in 1..=passes {
            for unit in ["check", "plan"] {
                for label in self.labels() {
                    let compiler = path_str(&self.side(&label).target.join("debug/ignite"));
                    let mut command =
                        words(&[&compiler, unit, "examples/terminal/terminal.design.ignite"]);
                    if unit == "plan" {
                        command.push("--out".into());
                        command.push(path_str(
                            &self.evidence.join(format!("plan-{number}-{label}.json")),
                        ));
                    }
                    let pair = format!("{}-cli-{number}", self.capture_id);
                    self.command(&label, unit, &pair, command, false, "measured")?;
                }
            }
            if number >= self.args.check_pairs
                && self.enough(|u| u == "check" || u == "plan", self.args.check_pairs)
            {
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn summarize(&self) -> R<Json> {
        let commits = Json::Obj(
            self.sides
                .iter()
                .map(|s| (s.label.clone(), Json::Str(s.commit.clone())))
                .collect(),
        );
        let (summary, markdown) = summarize_rows(
            &self.rows,
            &SummaryInputs {
                commits,
                has_comparison: self.has_comparison(),
                controls: &self.controls,
                warm_pairs: self.args.warm_pairs,
                check_pairs: self.args.check_pairs,
                absolute_max_load: self.args.absolute_max_load(),
                load_delta: self.args.load_delta(),
                frozen_baseline_matches: self.frozen_baseline_matches,
            },
        );
        dump(&self.evidence.join("summary.json"), &summary)?;
        let path = self.evidence.join("summary.md");
        fs::write(&path, markdown).map_err(io_err(path.display()))?;
        Ok(summary)
    }

    pub fn verify_sources(&self) -> R<()> {
        for side in &self.sides {
            let manifest = read_json(&self.evidence.join(format!("source-{}.json", side.label)))?;
            verify_manifest(&side.path, &manifest)?;
        }
        Ok(())
    }
}

pub fn owner_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".owner.json");
    target.with_file_name(name)
}

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn touch(path: &Path) -> R<()> {
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_err(path.display()))?;
    file.set_modified(SystemTime::now())
        .map_err(io_err(path.display()))
}

/// Run one measuring mode; the exit status of the Python harness: 1 when a command or the
/// mode failed, 2 when `all` or `summarize` lacks accepted evidence, else 0.
pub fn run_mode(mode: &str, args: RunArgs) -> R<i32> {
    args.validate()?;
    let mut harness = Harness::new(args)?;
    let outcome = match mode {
        "all" => harness
            .smoke()
            .and_then(|_| harness.cold())
            .and_then(|_| harness.warm())
            .and_then(|_| harness.programs())
            .and_then(|_| harness.cli()),
        "smoke" => harness.smoke(),
        "prepare" => harness.prepare(),
        "cold" => harness.cold(),
        "warm" => harness.warm(),
        "programs" => harness.programs(),
        "cli" => harness.cli(),
        "apply-cycle" => harness.apply_cycle(),
        _ => Ok(()),
    };
    if let Err(failure) = &outcome {
        eprintln!("{failure}");
    }
    harness.verify_sources()?;
    let summary = harness.summarize()?;
    let accepted = summary.field("accepted").truthy();
    Ok(if outcome.is_err() {
        1
    } else if mode == "summarize" {
        if accepted { 0 } else { 2 }
    } else if harness.rows.iter().any(|r| r.field("exit").truthy()) {
        1
    } else if mode == "all" && !accepted {
        2
    } else {
        0
    })
}

fn fixture_row(label: &str, pair: &str, seconds: i64, load: i64, exit: i64) -> Json {
    obj(vec![
        ("unit", "warm-workspace".into()),
        ("label", label.into()),
        ("pair", pair.into()),
        ("seconds", seconds.into()),
        ("load_before", load.into()),
        ("exit", exit.into()),
    ])
}

/// The Python harness's self-check: paired ratios, exclusions, candidate failures, strict
/// closure controls and preparation reuse, on fixture records in a scratch directory.
pub fn self_check(scratch: &Path) -> R<()> {
    let rows = vec![
        fixture_row("baseline", "1", 10, 1, 0),
        fixture_row("candidate", "1", 11, 1, 0),
        fixture_row("baseline", "2", 100, 1, 0),
        fixture_row("candidate", "2", 50, 1, 0),
        fixture_row("baseline", "3", 10, 1, 0),
        fixture_row("candidate", "3", 100, 20, 0),
        fixture_row("baseline", "4", 10, 1, 0),
        fixture_row("candidate", "4", 1, 1, 1),
    ];
    let value = compare_rows(&rows, "baseline", "candidate", 5.0).remove(0);
    ensure(
        value.field("valid_pairs") == &Json::Int(2) && value.field("ratio") == &Json::Float(0.8),
        "paired ratio",
    )?;
    let reasons: Vec<&str> = value
        .field("excluded")
        .as_arr()
        .iter()
        .filter_map(|e| e.field("reason").as_str())
        .collect();
    ensure(reasons == ["load_mismatch", "command_failed"], "exclusions")?;
    ensure(
        (11.0 + 50.0) / 2.0 / ((10.0 + 100.0) / 2.0) != 0.8,
        "ratio of medians is not the median of ratios",
    )?;
    ensure(
        failed_candidate(&[
            fixture_row("candidate", "1", 10, 1, 1),
            fixture_row("candidate", "2", 1, 1, 0),
        ]),
        "Later green samples cannot hide a candidate failure",
    )?;
    ensure(
        !failed_candidate(&[
            fixture_row("baseline", "1", 10, 1, 1),
            fixture_row("candidate", "2", 1, 1, 0),
        ]),
        "Unchanged-baseline failure is tracked separately",
    )?;
    ensure(within_budget("file-rustgen", 1.0), "rustgen at 1.0")?;
    ensure(
        !within_budget("file-rustgen", 1.01),
        "Generator tests must recover matched 11b time",
    )?;
    ensure(
        within_budget("warm-workspace", 1.10),
        "1.10 is within budget",
    )?;
    ensure(
        !within_budget("warm-workspace", 1.11),
        "1.11 is over budget",
    )?;
    let unique = format!("ignite-perf-self-check-{}-{}", std::process::id(), now_ns());
    let root = scratch.join(unique);
    let result = self_check_files(&root);
    let _ = fs::remove_dir_all(&root);
    result?;
    crate::cycle::self_check()?;
    println!(
        "Self-check passed: paired ratios, exclusions, candidate failures, strict closure controls, preparation reuse, and apply-cycle units, edits and medians"
    );
    Ok(())
}

fn self_check_files(root: &Path) -> R<()> {
    let source = root.join("format");
    fs::create_dir_all(&source).map_err(io_err(source.display()))?;
    let steps = gate();
    let serial = &steps[0].1;
    ensure(
        gate_command("candidate", &source, "fmt", serial) == *serial,
        "serial fmt",
    )?;
    fs::create_dir_all(source.join("scripts")).map_err(io_err("scripts"))?;
    fs::write(
        source.join("scripts/check-format.py"),
        "# frozen format-check entry point\n",
    )
    .map_err(io_err("check-format.py"))?;
    ensure(
        gate_command("candidate", &source, "fmt", serial) == ["python3", "scripts/check-format.py"],
        "frozen candidate fmt entry point",
    )?;
    ensure(
        gate_command("baseline", &source, "fmt", serial) == *serial,
        "baseline fmt",
    )?;
    ensure(
        gate_command("comparison", &source, "fmt", serial) == *serial,
        "comparison fmt",
    )?;
    ensure(
        gate_command("candidate", &source, "clippy", &steps[1].1) == steps[1].1,
        "candidate clippy",
    )?;
    let controls = obj(vec![
        ("warm_pairs", Json::Int(3)),
        ("check_pairs", Json::Int(15)),
        ("extra_warm_passes", Json::Int(2)),
        ("cold_retries", Json::Int(1)),
        ("load_delta", Json::Int(5)),
        ("absolute_max_load", Json::Int(9)),
    ]);
    ensure(closure_eligible(&controls, true, true), "strict controls")?;
    ensure(
        !closure_eligible(&controls, false, true),
        "Another baseline cannot redefine the budget",
    )?;
    ensure(
        !closure_eligible(&controls, true, false),
        "Published checkpoint comparison is required",
    )?;
    for (key, value) in [
        ("load_delta", 6),
        ("absolute_max_load", 10),
        ("cold_retries", 2),
        ("extra_warm_passes", 3),
        ("warm_pairs", 2),
        ("check_pairs", 14),
    ] {
        let mut loose = controls.clone();
        loose.set(key, Json::Int(value));
        ensure(!closure_eligible(&loose, true, true), key)?;
    }
    let target = root.join("prepare");
    let candidate = Side {
        label: "candidate".into(),
        reference: "frozen".into(),
        commit: "frozen-commit".into(),
        path: root.join("source"),
        target: target.clone(),
    };
    let mut units: Vec<String> = gate().iter().map(|(n, _)| format!("prepare-{n}")).collect();
    units.push("gate-prepare".into());
    units.push("runtime-release".into());
    let prepared: Vec<Json> = units
        .iter()
        .map(|unit| {
            obj(vec![
                ("label", "candidate".into()),
                ("unit", unit.as_str().into()),
                ("exit", Json::Int(0)),
                ("commit", "frozen-commit".into()),
                ("phase", "prepare".into()),
                ("pair", "prepare-1".into()),
                ("capture", "1".into()),
            ])
        })
        .collect();
    let artifacts: Vec<PathBuf> = PREPARED_ARTIFACTS.iter().map(|n| target.join(n)).collect();
    for path in &artifacts {
        fs::create_dir_all(path.parent().expect("artifact parent")).map_err(io_err("artifact"))?;
        fs::write(path, b"prepared").map_err(io_err(path.display()))?;
    }
    let sides = vec![candidate.clone()];
    ensure(is_prepared(&prepared, &sides), "complete preparation")?;
    ensure(
        !needs_prepare(&prepared, &sides)?,
        "Matching complete preparation must be reusable",
    )?;
    let mut both = sides.clone();
    both.push(Side {
        label: "baseline".into(),
        reference: "frozen-baseline".into(),
        ..candidate.clone()
    });
    ensure(
        !is_prepared(&prepared, &both),
        "Every configured source needs preparation",
    )?;
    for (key, value) in [
        ("exit", Json::Int(1)),
        ("commit", "other-commit".into()),
        ("pair", "prepare-2".into()),
        ("capture", "2".into()),
        ("phase", "measured".into()),
    ] {
        let mut rows = prepared.clone();
        let mut changed = prepared[0].clone();
        changed.set(key, value);
        rows.push(changed);
        ensure(!is_prepared(&rows, &sides), format!("changed {key}"))?;
    }
    ensure(
        !is_prepared(&prepared[..prepared.len() - 1], &sides),
        "A runtime build record is required",
    )?;
    let mut cleaned = prepared.clone();
    let mut clean = prepared[0].clone();
    clean.set("unit", "clean".into());
    cleaned.push(clean);
    ensure(
        !is_prepared(&cleaned, &sides),
        "Cleaning invalidates prior preparation",
    )?;
    let last = artifacts.last().expect("artifacts");
    fs::write(last, b"").map_err(io_err(last.display()))?;
    ensure(
        !is_prepared(&prepared, &sides),
        "Empty artifacts cannot prove preparation",
    )?;
    fs::remove_file(last).map_err(io_err(last.display()))?;
    ensure(
        needs_prepare(&prepared, &sides)?,
        "Missing outputs must execute full preparation",
    )
}
