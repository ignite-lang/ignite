//! Journal integrity and the host's working directory (`docs/slice.md` 22.7,
//! 22.4; rows 23 and 33 of 22.0, and rows 20 to 22 on a state that hosts no
//! process). Every host here runs in a working directory under the test's
//! own scratch directory.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common::apply::{BUILD as BUILD_TIMEOUT, LIVE as LIVE_TIMEOUT};

/// The text `inspect` gives as the next action on a damaged journal.
const DAMAGED: &str = "restore host-journal.jsonl or move the state aside; the host does not start on a damaged journal";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("read child exit") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// One test's directory: `state` is the host state, `a` and `b` two working
/// directories, everything else is CLI output.
struct Case {
    root: PathBuf,
    state: PathBuf,
    a: PathBuf,
    b: PathBuf,
    counter: std::cell::Cell<u64>,
}

impl Case {
    fn new(label: &str) -> Case {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "apply-integrity-{label}-{}-{stamp}",
            std::process::id()
        ));
        let (a, b) = (root.join("a"), root.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        Case {
            state: fs::canonicalize(&state).unwrap(),
            a: fs::canonicalize(&a).unwrap(),
            b: fs::canonicalize(&b).unwrap(),
            root,
            counter: std::cell::Cell::new(0),
        }
    }

    fn state_arg(&self) -> &str {
        self.state.to_str().unwrap()
    }

    /// Run the CLI in `cwd`, require `expected` as its exit code, return
    /// stdout and stderr.
    fn cli(&self, label: &str, cwd: &Path, args: &[&str], expected: i32) -> (String, String) {
        let sequence = self.counter.get();
        self.counter.set(sequence + 1);
        let stdout = self.root.join(format!("{sequence}-{label}.stdout"));
        let stderr = self.root.join(format!("{sequence}-{label}.stderr"));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(cwd)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start CLI");
        let limit = if args.first() == Some(&"plan") {
            BUILD_TIMEOUT
        } else {
            LIVE_TIMEOUT
        };
        let Some(status) = wait(&mut child, limit) else {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{label} exceeded {limit:?}\nstderr: {}", read(&stderr));
        };
        assert_eq!(
            status.code(),
            Some(expected),
            "{label}\nstdout: {}\nstderr: {}",
            read(&stdout),
            read(&stderr)
        );
        (read(&stdout), read(&stderr))
    }

    /// `ignite host` that must refuse to start: its stderr.
    fn refused(&self, label: &str, cwd: &Path, expected: i32) -> String {
        self.cli(label, cwd, &["host", "--state", self.state_arg()], expected)
            .1
    }

    fn inspect(&self, label: &str) -> Json {
        let (stdout, _) = self.cli(
            label,
            &self.root,
            &["inspect", "--state", self.state_arg(), "--json"],
            0,
        );
        Json::parse(&stdout).unwrap_or_else(|reason| panic!("{label}: {reason}\n{stdout}"))
    }

    /// Every file under the state with its bytes.
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(path: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    walk(&entry.path(), found);
                } else if kind.is_file() {
                    found.insert(entry.path(), fs::read(entry.path()).unwrap());
                }
            }
        }
        let mut found = BTreeMap::new();
        walk(&self.state, &mut found);
        found
    }

    fn journal(&self) -> PathBuf {
        self.state.join("host-journal.jsonl")
    }
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    /// Start a host in `cwd` and wait until it answers on its socket.
    fn start(case: &Case, label: &str, cwd: &Path) -> Host {
        let stderr = case.root.join(format!("{label}.stderr"));
        let child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["host", "--state", case.state_arg()])
            .current_dir(cwd)
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::piped())
            .stdout(File::create(case.root.join(format!("{label}.stdout"))).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start host");
        let mut host = Host {
            child: Some(child),
            stderr,
        };
        let deadline = Instant::now() + LIVE_TIMEOUT;
        loop {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "host exited: {}",
                read(&host.stderr)
            );
            if ignite::apply::inspect(&case.state, None)
                .is_ok_and(|status| status.get("online").and_then(Json::as_bool) == Some(true))
            {
                return host;
            }
            assert!(
                Instant::now() < deadline,
                "host socket deadline: {}",
                read(&host.stderr)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stop(&mut self) {
        let mut child = self.child.take().expect("host is running");
        let stdin = child.stdin.as_mut().expect("host control stdin");
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(&mut child, LIVE_TIMEOUT);
        assert!(
            status.is_some_and(|status| status.success()),
            "host stop failed: {status:?}\n{}",
            read(&self.stderr)
        );
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = writeln!(stdin, "stop");
                let _ = stdin.flush();
            }
            if wait(&mut child, Duration::from_secs(10)).is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("no {key} in {}", value.compact()))
    })
}

/// The `journal` field offline `inspect` gives for a damage.
fn damaged(record: i64, reason: &str, file: Option<&str>) -> Json {
    let mut fields: BTreeMap<String, Json> = [
        ("intact".to_string(), Json::Bool(false)),
        ("record".to_string(), Json::Int(record)),
        ("reason".to_string(), Json::Str(reason.into())),
    ]
    .into_iter()
    .collect();
    if let Some(file) = file {
        fields.insert("file".into(), Json::Str(file.into()));
    }
    Json::Object(fields)
}

/// Offline `inspect` of a damaged state: exit 0, offline, the damage and
/// the next action of row 21.
fn assert_reported(case: &Case, label: &str, journal: Json) {
    let status = case.inspect(label);
    assert_eq!(field(&status, &["online"]), &Json::Bool(false));
    assert_eq!(
        field(&status, &["journal"]),
        &journal,
        "{}",
        status.compact()
    );
    assert_eq!(
        field(&status, &["nextAction"]).as_str(),
        Some(DAMAGED),
        "{}",
        status.compact()
    );
}

/// Row 33: the host records its canonical working directory at its first
/// start and refuses another; a state without the record gets one.
#[test]
fn a_host_started_elsewhere_is_refused() {
    let case = Case::new("workdir");
    let mut host = Host::start(&case, "first", &case.a);
    host.stop();
    let workdir = case.state.join("workdir");
    assert_eq!(read(&workdir), format!("{}\n", case.a.display()));

    let before = case.files();
    let stderr = case.refused("elsewhere", &case.b, 2);
    assert!(
        stderr.contains(&format!(
            "error: state {} is hosted from {}; start ignite host there",
            case.state.display(),
            case.a.display()
        )),
        "{stderr}"
    );
    assert_eq!(case.files(), before, "a refused host changes no file");
    assert!(!case.state.join("control.sock").exists());

    let mut host = Host::start(&case, "again", &case.a);
    host.stop();

    fs::remove_file(&workdir).unwrap();
    let mut host = Host::start(&case, "unrecorded", &case.b);
    host.stop();
    assert_eq!(read(&workdir), format!("{}\n", case.b.display()));
    case.refused("back", &case.a, 2);
}

/// Rows 20 to 22 on a state that hosts no process: a torn final line is
/// repaired and listed, an altered record and a cut before the anchored
/// record refuse the host and are reported offline, every file unchanged.
#[test]
fn a_damaged_host_journal_refuses_the_host_and_inspect_reports_it() {
    let case = Case::new("journal");
    let mut host = Host::start(&case, "first", &case.a);
    host.stop();
    let intact = fs::read(case.journal()).unwrap();
    let status = Json::parse(&read(&case.state.join("status.json"))).unwrap();
    let lines: Vec<&[u8]> = intact.split_inclusive(|byte| *byte == b'\n').collect();
    let last = Json::parse(std::str::from_utf8(lines[lines.len() - 1]).unwrap()).unwrap();
    assert_eq!(
        field(&status, &["journalAnchor", "record"]).as_i64(),
        Some(lines.len() as i64),
        "status.json anchors the last state record"
    );
    assert_eq!(
        field(&status, &["journalAnchor", "digest"]),
        field(&last, &["digest"])
    );

    // Row 20: a torn final record.
    let torn = b"{\"kind\":\"sta";
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(case.journal())
        .unwrap();
    file.write_all(torn).unwrap();
    drop(file);
    let status = case.inspect("torn-offline");
    assert_eq!(
        field(&status, &["journal"]).compact(),
        "{\"intact\":true,\"pendingRepair\":{\"droppedBytes\":12}}"
    );
    let mut host = Host::start(&case, "repair", &case.a);
    let status = case.inspect("repaired-online");
    assert_eq!(field(&status, &["journal"]).compact(), "{\"intact\":true}");
    let repairs = field(&status, &["journalRepairs"]).as_array().unwrap();
    assert_eq!(repairs.len(), 1, "{}", status.compact());
    assert_eq!(field(&repairs[0], &["droppedBytes"]).as_i64(), Some(12));
    assert_eq!(
        field(&repairs[0], &["droppedDigest"]).as_str(),
        Some(ignite::apply::digest::bytes(torn).as_str())
    );
    host.stop();
    assert!(
        read(&host.stderr).contains("ignite host: dropped a torn final journal record of 12 bytes"),
        "{}",
        read(&host.stderr)
    );
    let repaired = read(&case.journal());
    let first_new = repaired.lines().nth(lines.len()).unwrap();
    let record = Json::parse(first_new).unwrap();
    assert_eq!(
        field(&record, &["kind"]).as_str(),
        Some("journalRepaired"),
        "the first new record is the repair"
    );

    // Row 21: one byte inside the last record's value, still valid JSON.
    let good = fs::read(case.journal()).unwrap();
    let count = good.iter().filter(|byte| **byte == b'\n').count();
    let start = good[..good.len() - 1]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    let mut altered = good.clone();
    let key = b"\"committedRevision\":";
    let at = start
        + altered[start..]
            .windows(key.len())
            .position(|window| window == key)
            .expect("the last record is a state record")
        + key.len();
    altered[at] = if altered[at] == b'7' { b'8' } else { b'7' };
    assert!(Json::parse(std::str::from_utf8(&altered[start..]).unwrap().trim_end()).is_ok());
    fs::write(case.journal(), &altered).unwrap();
    let before = case.files();
    let stderr = case.refused("altered", &case.a, 3);
    assert!(
        stderr.contains(&format!(
            "error: host journal record {count}: digest mismatch"
        )),
        "{stderr}"
    );
    assert_eq!(case.files(), before, "a refused host changes no file");
    assert_reported(
        &case,
        "altered-offline",
        damaged(count as i64, "digest mismatch", None),
    );

    // Row 22: cut at the end of the line before the last state record.
    let cut = &good[..start];
    fs::write(case.journal(), cut).unwrap();
    let before = case.files();
    let stderr = case.refused("cut", &case.a, 3);
    assert!(
        stderr.contains(&format!(
            "error: host journal ends before record {count} that status.json anchors"
        )),
        "{stderr}"
    );
    assert_eq!(case.files(), before);
    assert_reported(
        &case,
        "cut-offline",
        damaged(count as i64, "shorter than status.json", None),
    );

    fs::write(case.journal(), &good).unwrap();
    let mut host = Host::start(&case, "restored", &case.a);
    host.stop();
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Row 23: after a committed apply of the terminal design (Rust), one byte
/// of the last revision-journal record the host journal holds a copy of is
/// changed; the host refuses to start and `inspect` names the file.
#[test]
fn an_altered_revision_journal_line_refuses_the_host() {
    let case = Case::new("revision");
    let fixture = case.root.join("terminal");
    fs::create_dir_all(&fixture).unwrap();
    for entry in fs::read_dir(repo().join("examples/terminal")).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "ignite")
        {
            fs::copy(&path, fixture.join(path.file_name().unwrap())).unwrap();
        }
    }
    let source = fixture.join("terminal.design.ignite");
    let original = read(&source);
    assert!(original.contains("port = 2323"), "terminal fixture port");
    fs::write(
        &source,
        original.replace("port = 2323", &format!("port = {}", free_port())),
    )
    .unwrap();
    crate::common::apply::shared_build(&case.state);

    let mut host = Host::start(&case, "host", &case.a);
    let plan = case.root.join("plan.json");
    let mut args = vec![
        "plan".to_string(),
        source.display().to_string(),
        "--assembly".into(),
        "local".into(),
        "--for-apply".into(),
        "--state".into(),
        case.state_arg().into(),
        "--out".into(),
        plan.display().to_string(),
    ];
    args.extend(crate::common::apply::target_args("rust"));
    {
        let _build = crate::common::apply::build_lock("rust");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        case.cli("prepare", &case.a, &args, 0);
    }
    case.cli(
        "apply",
        &case.a,
        &["apply", plan.to_str().unwrap(), "--state", case.state_arg()],
        0,
    );
    host.stop();

    let copies: Vec<i64> = read(&case.journal())
        .lines()
        .filter_map(|line| Json::parse(line).ok())
        .filter(|record| record.get("kind").and_then(Json::as_str) == Some("revisionRecord"))
        .filter_map(|record| record.get("value")?.get("record")?.get("seq")?.as_i64())
        .collect();
    let seq = *copies.iter().max().expect("the run's records are copied");
    let revisions: Vec<PathBuf> = fs::read_dir(case.state.join("revisions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("journal.jsonl").is_file())
        .collect();
    assert_eq!(revisions.len(), 1, "{revisions:?}");
    let name = revisions[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let path = revisions[0].join("journal.jsonl");
    let mut bytes = fs::read(&path).unwrap();
    let start = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .take(seq as usize)
        .map(<[u8]>::len)
        .sum::<usize>();
    let line = Json::parse(
        std::str::from_utf8(&bytes[start..])
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(field(&line, &["seq"]).as_i64(), Some(seq));
    let key = b"\"kind\":\"";
    let at = start
        + bytes[start..]
            .windows(key.len())
            .position(|window| window == key)
            .unwrap()
        + key.len();
    bytes[at] ^= 0x20;
    fs::write(&path, &bytes).unwrap();

    let file = format!("revisions/{name}/journal.jsonl");
    let before = case.files();
    let stderr = case.refused("altered", &case.a, 3);
    assert!(
        stderr.contains(&format!(
            "error: {file} record {seq}: differs from the host journal"
        )),
        "{stderr}"
    );
    assert_eq!(case.files(), before, "a refused host changes no file");
    assert_reported(
        &case,
        "offline",
        damaged(seq, "revision journal differs", Some(&file)),
    );
}
