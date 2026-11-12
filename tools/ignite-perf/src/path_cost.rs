//! Mode `path-cost` (`docs/quality.md` row 3.10, "The path cost at every merge", `task gate`):
//! the instructions and system calls of a fixed set of scenarios whose programs are frozen at a
//! release, counted on every merge and held against their counts at the last release.
//!
//! A scenario is a directory under [`SCENARIOS`]: its program files and a script, `scenario`
//! ([`parse_scenario`]). A server scenario builds one program per target (`--release`) and runs
//! its executable once under callgrind and once under `strace -f`, driving the operations of the
//! script over loopback; a cell is the start, or one batch of `repeat` executions of an operation
//! after `warm` unmeasured ones. A CLI scenario runs `ignite` commands, each once natively to warm
//! caches and once under each instrument. A program file of a frozen scenario is the file of the
//! tag it names, and never edited ([`frozen_differences`]): a cost that rises on such a scenario
//! was added to a path that already existed, since the program cannot use what came later.
//!
//! The counts of the last release are files under [`COUNTS`], one per scenario and target
//! ([`parse_counts`]), written by `--record` over several runs: per cell the median, every run's
//! value, and the functions (self instructions) and system calls of the median run. A merge
//! measures each cell once and is refused ([`judge`]) when a cell is above its recorded value by
//! more than the noise band ([`band`]); the refusal names the functions and system calls that
//! grew or are new, each with both values. A cell whose recorded runs spread further than the
//! band is named and not judged; a scenario recorded with `first` (added after the freeze) is
//! shown and compared from the next release on.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use crate::cli::default_repo;
use crate::json::{Json, obj};
use crate::perf::{R, ensure, io_err};

/// The row of `docs/quality.md`.
pub const METRIC: &str = "3.10";
/// The scenario directories, relative to the repository.
pub const SCENARIOS: &str = "bench/path-cost";
/// The recorded counts, one file per scenario and target.
pub const COUNTS: &str = "bench/path-cost/counts";
/// The most functions or system calls one refused cell lists.
pub const MOST_LISTED: usize = 15;
/// The targets a scenario may name: the two backends, and `all` for a command whose cost does
/// not depend on one.
pub const TARGETS: [&str; 3] = ["all", "llvm", "rust"];

/// The noise band of instructions: a cell is refused above its recorded value by more than this
/// many thousandths of it.
pub const INSTRUCTION_BAND_PERMILLE: u64 = 10;
/// The noise band of system calls: thousandths of the recorded value, and at least
/// [`SYSCALL_BAND_LEAST`] calls.
pub const SYSCALL_BAND_PERMILLE: u64 = 10;
pub const SYSCALL_BAND_LEAST: u64 = 1;

/// The unit of a cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Instructions,
    Syscalls,
}

impl Unit {
    pub fn word(self) -> &'static str {
        match self {
            Unit::Instructions => "instructions",
            Unit::Syscalls => "syscalls",
        }
    }
}

/// How far a cell may rise above its recorded `value` without being refused.
pub fn band(unit: Unit, value: u64) -> u64 {
    match unit {
        Unit::Instructions => value * INSTRUCTION_BAND_PERMILLE / 1000,
        Unit::Syscalls => (value * SYSCALL_BAND_PERMILLE / 1000).max(SYSCALL_BAND_LEAST),
    }
}

/// The least self instructions a function has to be listed in a count file: a ten-thousandth
/// of the cell, at least 1. A function under it at the release that is over it now is listed as
/// new, "under" this value then.
pub fn listed_floor(total: u64) -> u64 {
    (total / 10_000).max(1)
}

// ---- scenarios ----------------------------------------------------------------------------

/// A scenario directory and its script.
#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: String,
    /// The release its program files were frozen at; `None` for a scenario added after the
    /// freeze, whose files are frozen from the first release that holds them.
    pub frozen: Option<String>,
    pub files: Vec<FrozenFile>,
    pub kind: Kind,
}

/// A program file of a scenario and, when frozen, its path in the tag's tree.
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenFile {
    pub name: String,
    pub origin: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Server(Server),
    Cli(Vec<CliOp>),
}

/// A program built per target whose executable serves on loopback.
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub source: String,
    pub executable: String,
    pub targets: Vec<String>,
    pub warm: usize,
    pub repeat: usize,
    pub setup: Vec<(usize, Request)>,
    pub ops: Vec<ServerOp>,
}

/// An HTTP request and the status its answer must have. `{id:N}` in the path is `N` at the
/// first execution of the operation and one more at each one after it, warm-up included.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub status: u16,
    pub method: String,
    pub path: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerOp {
    pub name: String,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// One request on the keep-alive connection the run opens after its start.
    Http(Request),
    /// A connection opened and closed.
    Connect,
    /// A Telnet session: connect, wait for the first prompt (`IAC GA`), send the line, read
    /// until the server closes.
    Telnet(String),
}

/// An `ignite` command of a CLI scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct CliOp {
    pub name: String,
    pub target: String,
    pub scope: Scope,
    /// The arguments; `{out}` is a fresh directory per run, `{cargo}` the cargo target
    /// directory the runs share.
    pub args: Vec<String>,
}

/// What a CLI operation counts: the `ignite` process alone, or with every process it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Driver,
    Tree,
}

impl Scenario {
    /// The targets the scenario measures, in order.
    pub fn targets(&self) -> Vec<String> {
        match &self.kind {
            Kind::Server(server) => server.targets.clone(),
            Kind::Cli(ops) => {
                let mut out: Vec<String> = Vec::new();
                for op in ops {
                    if !out.contains(&op.target) {
                        out.push(op.target.clone());
                    }
                }
                out
            }
        }
    }

    /// Every `(target, op)` the scenario measures; a cell is one of them with a unit.
    pub fn cells(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for target in self.targets() {
            match &self.kind {
                Kind::Server(server) => {
                    out.push((target.clone(), "start".to_string()));
                    for op in &server.ops {
                        out.push((target.clone(), op.name.clone()));
                    }
                }
                Kind::Cli(ops) => {
                    for op in ops.iter().filter(|op| op.target == target) {
                        out.push((target.clone(), op.name.clone()));
                    }
                }
            }
        }
        out
    }
}

/// The first `n` words of `line` and the rest of it, trimmed.
fn words(line: &str, n: usize) -> (Vec<&str>, &str) {
    let mut out = Vec::new();
    let mut rest = line.trim_start();
    while out.len() < n && !rest.is_empty() {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        out.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    (out, rest.trim_end())
}

fn number(word: &str, what: &str) -> R<usize> {
    word.parse()
        .map_err(|_| format!("{what} {word:?} is no number"))
}

fn request(words: &[&str], body: &str) -> R<Request> {
    let [status, method, path] = words else {
        return Err("a request is `<status> <METHOD> <path> [body]`".to_string());
    };
    let status: u16 = status
        .parse()
        .map_err(|_| format!("status {status:?} is no number"))?;
    ensure(
        path.starts_with('/'),
        format!("path {path:?} is not absolute"),
    )?;
    Ok(Request {
        status,
        method: method.to_string(),
        path: path.to_string(),
        body: body.to_string(),
    })
}

/// Read the script of the scenario `name`. Lines:
///
/// - `frozen <tag>`: the release the program files were frozen at;
/// - `file <name> [<path at the tag>]`: a program file of the directory; a frozen scenario
///   names each file's path in the tag's tree;
/// - a server: `server <source> <executable>`, `targets <target>...`, `warm <n>`,
///   `repeat <n>`, `setup <times> <status> <METHOD> <path> [body]`, and operations
///   `op <name> <status> <METHOD> <path> [body]`, `op <name> connect`,
///   `op <name> telnet <line>`; the operation `start` is every server's;
/// - a CLI scenario: `op <name> <target> driver|tree <ignite arguments>...`.
pub fn parse_scenario(name: &str, text: &str) -> R<Scenario> {
    let at = |n: usize, e: String| format!("bench/path-cost/{name}/scenario:{n}: {e}");
    let mut frozen = None;
    let mut files = Vec::new();
    let mut server: Option<(String, String)> = None;
    let (mut targets, mut warm, mut repeat) = (Vec::new(), None, None);
    let mut setup = Vec::new();
    let mut ops: Vec<(usize, String, Vec<String>, String)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let n = index + 1;
        let (head, rest) = words(line, 1);
        let Some(&keyword) = head.first() else {
            continue;
        };
        match keyword {
            "frozen" => frozen = Some(rest.to_string()),
            "file" => {
                let (w, extra) = words(rest, 2);
                if w.is_empty() || !extra.is_empty() {
                    return Err(at(n, "`file <name> [<path at the tag>]`".into()));
                }
                files.push(FrozenFile {
                    name: w[0].to_string(),
                    origin: w.get(1).map(|s| s.to_string()),
                });
            }
            "server" => {
                let (w, extra) = words(rest, 2);
                if w.len() != 2 || !extra.is_empty() {
                    return Err(at(n, "`server <source> <executable>`".into()));
                }
                server = Some((w[0].to_string(), w[1].to_string()));
            }
            "targets" => {
                targets = rest.split_whitespace().map(str::to_string).collect();
            }
            "warm" => warm = Some(number(rest, "warm").map_err(|e| at(n, e))?),
            "repeat" => repeat = Some(number(rest, "repeat").map_err(|e| at(n, e))?),
            "setup" => {
                let (w, body) = words(rest, 4);
                if w.len() != 4 {
                    return Err(at(
                        n,
                        "`setup <times> <status> <METHOD> <path> [body]`".into(),
                    ));
                }
                let times = number(w[0], "times").map_err(|e| at(n, e))?;
                setup.push((times, request(&w[1..], body).map_err(|e| at(n, e))?));
            }
            "op" => {
                let (w, more) = words(rest, 1);
                let Some(&op) = w.first() else {
                    return Err(at(n, "`op <name> ...`".into()));
                };
                let (args, body) = words(more, 3);
                ops.push((
                    n,
                    op.to_string(),
                    args.iter().map(|s| s.to_string()).collect(),
                    body.to_string(),
                ));
            }
            other => return Err(at(n, format!("unknown line `{other}`"))),
        }
    }
    if frozen.is_some()
        && let Some(file) = files.iter().find(|f| f.origin.is_none())
    {
        return Err(format!(
            "bench/path-cost/{name}/scenario: `{}` of a frozen scenario names no path at its tag",
            file.name
        ));
    }
    let kind = match server {
        Some((source, executable)) => {
            for target in &targets {
                ensure(
                    target == "llvm" || target == "rust",
                    at(
                        0,
                        format!("a server's target is llvm or rust, not {target:?}"),
                    ),
                )?;
            }
            ensure(
                !targets.is_empty(),
                at(0, "a server names its `targets`".into()),
            )?;
            let repeat = repeat.ok_or_else(|| at(0, "a server names `repeat`".into()))?;
            ensure(repeat > 0, at(0, "`repeat` is at least 1".into()))?;
            let mut server_ops: Vec<ServerOp> = Vec::new();
            for (n, op, args, body) in ops {
                ensure(op != "start", at(n, "`start` is every server's own".into()))?;
                ensure(
                    !server_ops.iter().any(|o| o.name == op),
                    at(n, format!("operation `{op}` twice")),
                )?;
                let action = match args.first().map(String::as_str) {
                    Some("connect") if args.len() == 1 && body.is_empty() => Action::Connect,
                    Some("telnet") => {
                        let mut parts = args[1..].to_vec();
                        if !body.is_empty() {
                            parts.push(body.clone());
                        }
                        let line = parts.join(" ");
                        ensure(!line.is_empty(), at(n, "`telnet <line>`".into()))?;
                        Action::Telnet(line)
                    }
                    _ => {
                        let words: Vec<&str> = args.iter().map(String::as_str).collect();
                        Action::Http(request(&words, &body).map_err(|e| at(n, e))?)
                    }
                };
                server_ops.push(ServerOp { name: op, action });
            }
            Kind::Server(Server {
                source,
                executable,
                targets,
                warm: warm.unwrap_or(0),
                repeat,
                setup,
                ops: server_ops,
            })
        }
        None => {
            let mut cli = Vec::new();
            for (n, op, args, body) in ops {
                let [target, scope, first] = args.as_slice() else {
                    return Err(at(
                        n,
                        "`op <name> <target> driver|tree <arguments>...`".into(),
                    ));
                };
                ensure(
                    TARGETS.contains(&target.as_str()),
                    at(n, format!("target {target:?} is not one of {TARGETS:?}")),
                )?;
                let scope = match scope.as_str() {
                    "driver" => Scope::Driver,
                    "tree" => Scope::Tree,
                    other => return Err(at(n, format!("scope {other:?} is driver or tree"))),
                };
                let mut arguments = vec![first.clone()];
                arguments.extend(body.split_whitespace().map(str::to_string));
                cli.push(CliOp {
                    name: op,
                    target: target.clone(),
                    scope,
                    args: arguments,
                });
            }
            let mut seen: Vec<(String, String)> = Vec::new();
            for op in &cli {
                let key = (op.target.clone(), op.name.clone());
                ensure(
                    !seen.contains(&key),
                    format!(
                        "bench/path-cost/{name}/scenario: `{}` twice on {}",
                        op.name, op.target
                    ),
                )?;
                seen.push(key);
            }
            ensure(
                !cli.is_empty(),
                format!("bench/path-cost/{name}/scenario: no operation"),
            )?;
            Kind::Cli(cli)
        }
    };
    Ok(Scenario {
        name: name.to_string(),
        frozen,
        files,
        kind,
    })
}

/// Every scenario under `root` (each directory with a `scenario` file), by name.
pub fn read_scenarios(root: &Path) -> R<Vec<Scenario>> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .map_err(io_err(root.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.join("scenario").is_file())
        .collect();
    dirs.sort();
    dirs.iter()
        .map(|dir| {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let path = dir.join("scenario");
            let text = fs::read_to_string(&path).map_err(io_err(path.display()))?;
            parse_scenario(&name, &text)
        })
        .collect()
}

// ---- frozen files -------------------------------------------------------------------------

/// What a tag holds at a path.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    File(Vec<u8>),
    NoFile,
    /// The tag is not in this repository.
    NoTag,
}

/// The differences of the scenarios under `root` from their tags, and what the check could not
/// compare yet. `last_release` is the last release tag; `show(tag, path)` reads a file of a
/// tag's tree.
///
/// A scenario that the last release holds (its `scenario` file is in the tag's tree) is
/// compared whole: every file of its directory is the release's. One that no release holds yet
/// is compared by its program files: each `file` with a path is the file of the `frozen` tag
/// at that path, and that tag must be in the repository. A file in the directory that the
/// script does not list is an error either way.
pub fn frozen_differences(
    root: &Path,
    scenarios: &[Scenario],
    last_release: Option<&str>,
    show: &dyn Fn(&str, &str) -> Result<Shown, String>,
) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut notes) = (Vec::new(), Vec::new());
    for scenario in scenarios {
        let dir = root.join(&scenario.name);
        let shown_dir = format!("{SCENARIOS}/{}", scenario.name);
        let mut present: Vec<String> = match fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect(),
            Err(e) => {
                errors.push(format!("{shown_dir}: {e}"));
                continue;
            }
        };
        present.sort();
        for file in &present {
            if file != "scenario" && !scenario.files.iter().any(|f| &f.name == file) {
                errors.push(format!(
                    "{shown_dir}/{file}: a file its scenario does not list"
                ));
            }
        }
        let read =
            |name: &str| fs::read(dir.join(name)).map_err(|e| format!("{shown_dir}/{name}: {e}"));
        let released = match last_release {
            Some(tag) => match show(tag, &format!("{shown_dir}/scenario")) {
                Ok(Shown::File(_)) => Some(tag),
                Ok(_) => None,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            },
            None => None,
        };
        if let Some(tag) = released {
            for file in &present {
                let path = format!("{shown_dir}/{file}");
                match (show(tag, &path), read(file)) {
                    (Err(e), _) | (_, Err(e)) => errors.push(e),
                    (Ok(Shown::File(then)), Ok(now)) if then == now => {}
                    (Ok(Shown::File(_)), Ok(_)) => errors.push(format!(
                        "{path} differs from its file at {tag}: a scenario a release holds is never edited; a changed program is a new scenario"
                    )),
                    (Ok(_), _) => errors.push(format!(
                        "{path} is not in {tag}, which holds the scenario: a scenario a release holds is never edited"
                    )),
                }
            }
            notes.push(format!("{shown_dir}: every file is the file of {tag}"));
            continue;
        }
        notes.push(format!(
            "{shown_dir}/scenario is in no release yet{}: the directory is compared whole from the first release that holds it",
            last_release.map_or(String::new(), |t| format!(" (the last is {t})"))
        ));
        let Some(tag) = &scenario.frozen else {
            notes.push(format!(
                "{shown_dir}: added after the freeze, its program files are frozen from the first release that holds them"
            ));
            continue;
        };
        let before = errors.len();
        for file in &scenario.files {
            let Some(origin) = &file.origin else {
                continue;
            };
            let path = format!("{shown_dir}/{}", file.name);
            match (show(tag, origin), read(&file.name)) {
                (Err(e), _) | (_, Err(e)) => errors.push(e),
                (Ok(Shown::NoTag), _) => {
                    errors.push(format!(
                        "tag {tag} is not in this repository: the frozen files of {shown_dir} cannot be compared with it; fetch it (`git fetch --tags`)"
                    ));
                    break;
                }
                (Ok(Shown::NoFile), _) => {
                    errors.push(format!("{path}: {origin} is not in {tag}"));
                }
                (Ok(Shown::File(then)), Ok(now)) if then == now => {}
                (Ok(Shown::File(_)), Ok(_)) => errors.push(format!(
                    "{path} differs from {origin} at {tag}: a frozen scenario is never edited; a changed program is a new scenario"
                )),
            }
        }
        if errors.len() == before {
            notes.push(format!("{shown_dir}: its program files are those of {tag}"));
        }
    }
    (errors, notes)
}

/// `git` in `repo`, its stdout, or `None` when it fails.
fn git_out(repo: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// The last release: the nearest `v*` tag reachable from HEAD, as `git describe` finds it.
pub fn last_release_tag(repo: &Path) -> Option<String> {
    let out = git_out(
        repo,
        &["describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD"],
    )?;
    let tag = String::from_utf8_lossy(&out).trim().to_string();
    (!tag.is_empty()).then_some(tag)
}

/// [`frozen_differences`] of the scenarios of `repo`, read through `git`.
pub fn frozen_in_repository(repo: &Path, scenarios: &[Scenario]) -> (Vec<String>, Vec<String>) {
    let show = |tag: &str, path: &str| -> Result<Shown, String> {
        if git_out(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/tags/{tag}"),
            ],
        )
        .is_none()
        {
            return Ok(Shown::NoTag);
        }
        Ok(git_out(repo, &["show", &format!("{tag}:{path}")]).map_or(Shown::NoFile, Shown::File))
    };
    let last = last_release_tag(repo);
    frozen_differences(&repo.join(SCENARIOS), scenarios, last.as_deref(), &show)
}

// ---- counts -------------------------------------------------------------------------------

/// What the values of a count file are compared as.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    /// The values at this release: a merge is held against them.
    Release(String),
    /// The first values of a scenario added after the freeze, at this commit: shown, compared
    /// from the next release on.
    First(String),
}

/// The counts of one scenario on one target.
#[derive(Clone, Debug, PartialEq)]
pub struct Counts {
    pub scenario: String,
    pub target: String,
    pub reference: Reference,
    pub ops: Vec<OpCounts>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpCounts {
    pub op: String,
    pub instructions: Measure,
    pub syscalls: Measure,
}

/// One cell: its value (the median of the runs), every run's value, and the functions or
/// system calls of the median run with their counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    pub value: u64,
    pub runs: Vec<u64>,
    pub items: BTreeMap<String, u64>,
}

impl Measure {
    /// The highest run less the lowest.
    pub fn spread(&self) -> u64 {
        let low = self.runs.iter().min().copied().unwrap_or(self.value);
        let high = self.runs.iter().max().copied().unwrap_or(self.value);
        high - low
    }
}

impl OpCounts {
    pub fn measure(&self, unit: Unit) -> &Measure {
        match unit {
            Unit::Instructions => &self.instructions,
            Unit::Syscalls => &self.syscalls,
        }
    }
}

/// The file name of the counts of `scenario` on `target` under [`COUNTS`].
pub fn counts_file(scenario: &str, target: &str) -> String {
    format!("{scenario}-{target}.counts")
}

fn sorted_items(items: &BTreeMap<String, u64>) -> Vec<(&String, &u64)> {
    let mut out: Vec<(&String, &u64)> = items.iter().collect();
    out.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    out
}

/// The text of a count file: `path-cost counts`, `scenario`, `target`, `release <tag>` or
/// `first <commit>`, then per operation `op <name>`, `instructions <value> runs <v>...`,
/// `syscalls <value> runs <v>...`, `fn <count> <name>` and `syscall <count> <name>` lines,
/// most first.
pub fn write_counts(counts: &Counts) -> String {
    let mut out = format!(
        "path-cost counts\nscenario {}\ntarget {}\n",
        counts.scenario, counts.target
    );
    match &counts.reference {
        Reference::Release(tag) => out.push_str(&format!("release {tag}\n")),
        Reference::First(commit) => out.push_str(&format!("first {commit}\n")),
    }
    for op in &counts.ops {
        out.push_str(&format!("op {}\n", op.op));
        for unit in [Unit::Instructions, Unit::Syscalls] {
            let m = op.measure(unit);
            let runs: Vec<String> = m.runs.iter().map(u64::to_string).collect();
            out.push_str(&format!(
                "{} {} runs {}\n",
                unit.word(),
                m.value,
                runs.join(" ")
            ));
        }
        for (unit, word) in [(Unit::Instructions, "fn"), (Unit::Syscalls, "syscall")] {
            for (name, count) in sorted_items(&op.measure(unit).items) {
                out.push_str(&format!("{word} {count} {name}\n"));
            }
        }
    }
    out
}

/// Read a count file ([`write_counts`]).
pub fn parse_counts(text: &str) -> R<Counts> {
    let mut lines = text.lines().enumerate();
    ensure(
        lines.next().map(|(_, l)| l) == Some("path-cost counts"),
        "a count file begins with `path-cost counts`",
    )?;
    let mut field = |key: &str| -> R<String> {
        match lines.next() {
            Some((_, line)) => line
                .strip_prefix(&format!("{key} "))
                .map(str::to_string)
                .ok_or_else(|| format!("`{key}` expected, found {line:?}")),
            None => Err(format!("`{key}` expected at the end")),
        }
    };
    let scenario = field("scenario")?;
    let target = field("target")?;
    let reference = match lines.next() {
        Some((_, line)) => match line.split_once(' ') {
            Some(("release", tag)) => Reference::Release(tag.to_string()),
            Some(("first", commit)) => Reference::First(commit.to_string()),
            _ => return Err(format!("`release` or `first` expected, found {line:?}")),
        },
        None => return Err("`release` or `first` expected at the end".to_string()),
    };
    let empty = Measure {
        value: 0,
        runs: Vec::new(),
        items: BTreeMap::new(),
    };
    let mut ops: Vec<OpCounts> = Vec::new();
    let mut seen: Vec<(usize, bool, bool)> = Vec::new();
    for (index, line) in lines {
        let n = index + 1;
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let at = |e: &str| format!("line {n}: {e}: {line:?}");
        if word == "op" {
            ops.push(OpCounts {
                op: rest.to_string(),
                instructions: empty.clone(),
                syscalls: empty.clone(),
            });
            seen.push((n, false, false));
            continue;
        }
        let (Some(op), Some(state)) = (ops.last_mut(), seen.last_mut()) else {
            return Err(at("before the first `op`"));
        };
        match word {
            "instructions" | "syscalls" => {
                let mut parts = rest.split(' ');
                let value = parts.next().and_then(|v| v.parse::<u64>().ok());
                let runs_word = parts.next();
                let runs: Option<Vec<u64>> = parts.map(|v| v.parse::<u64>().ok()).collect();
                let (Some(value), Some("runs"), Some(runs)) = (value, runs_word, runs) else {
                    return Err(at("`<unit> <value> runs <value>...` expected"));
                };
                ensure(!runs.is_empty(), at("no runs"))?;
                let measure = if word == "instructions" {
                    state.1 = true;
                    &mut op.instructions
                } else {
                    state.2 = true;
                    &mut op.syscalls
                };
                measure.value = value;
                measure.runs = runs;
            }
            "fn" | "syscall" => {
                let (count, name) = rest
                    .split_once(' ')
                    .ok_or_else(|| at("`<word> <count> <name>` expected"))?;
                let count: u64 = count.parse().map_err(|_| at("a count"))?;
                let measure = if word == "fn" {
                    &mut op.instructions
                } else {
                    &mut op.syscalls
                };
                measure.items.insert(name.to_string(), count);
            }
            _ => return Err(at("unknown line")),
        }
    }
    for (n, instructions, syscalls) in seen {
        ensure(
            instructions && syscalls,
            format!("the op of line {n} has no `instructions` or no `syscalls` line"),
        )?;
    }
    Ok(Counts {
        scenario,
        target,
        reference,
        ops,
    })
}

/// Every count file under `dir`.
pub fn read_counts_dir(dir: &Path) -> R<Vec<Counts>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(io_err(dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "counts"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).map_err(io_err(path.display()))?;
            let counts = parse_counts(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let expected = counts_file(&counts.scenario, &counts.target);
            ensure(
                path.file_name()
                    .is_some_and(|n| n.to_string_lossy() == expected),
                format!("{}: the counts of {expected}", path.display()),
            )?;
            Ok(counts)
        })
        .collect()
}

// ---- the profiles the instruments write ---------------------------------------------------

/// A count and what it is made of: functions with their self instructions, or system calls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub total: u64,
    pub items: BTreeMap<String, u64>,
}

impl Profile {
    pub fn add(&mut self, other: &Profile) {
        self.total += other.total;
        for (name, count) in &other.items {
            *self.items.entry(name.clone()).or_default() += count;
        }
    }
}

/// A function's name as it is compared across builds: a Rust symbol's hash
/// (`::h0123456789abcdef`) left out, and a function without a symbol named by its object as
/// `??? <file name>`, since its address moves with every change.
fn function_name(name: &str, object: &str) -> String {
    if name.starts_with("0x") && name[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        let file = object.rsplit('/').next().unwrap_or(object);
        return format!("??? {file}");
    }
    match name.rsplit_once("::h") {
        Some((head, hash)) if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) => {
            head.to_string()
        }
        _ => name.to_string(),
    }
}

/// The self instructions per function of a callgrind output written with
/// `--compress-strings=no --compress-pos=no` (one part: a dump or the whole run). A cost line
/// after `calls=` is the inclusive cost of that call and belongs to the callee, so it is left
/// out; the total is the file's `totals:` line.
pub fn callgrind_profile(text: &str) -> R<Profile> {
    let mut profile = Profile::default();
    let (mut object, mut function) = (String::new(), String::new());
    let mut after_call = false;
    let mut totals = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("totals:") {
            totals = rest.trim().parse::<u64>().ok();
            continue;
        }
        if line.starts_with(|c: char| c.is_ascii_digit() || c == '+' || c == '-' || c == '*') {
            let cost: u64 = line
                .split_whitespace()
                .last()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("a cost line {line:?}"))?;
            if after_call {
                after_call = false;
            } else {
                *profile.items.entry(function.clone()).or_default() += cost;
            }
            continue;
        }
        if let Some(name) = line.strip_prefix("ob=") {
            object = name.to_string();
        } else if let Some(name) = line.strip_prefix("fn=") {
            function = function_name(name, &object);
        } else if line.starts_with("calls=") {
            after_call = true;
        }
    }
    profile.total = totals.ok_or("no `totals:` line in the callgrind output")?;
    profile.items.retain(|_, count| *count > 0);
    Ok(profile)
}

/// The system calls of a `strace` output (`-o`, with `-f` or without): a call counts once, at
/// the line that begins it; a resumed call, a signal and an exit are no call; a last line that
/// is not complete yet is left out.
pub fn strace_profile(text: &str) -> Profile {
    let mut profile = Profile::default();
    let complete = match text.rfind('\n') {
        Some(end) => &text[..end],
        None => "",
    };
    for line in complete.lines() {
        let call = match line.split_once(' ') {
            Some((pid, rest)) if pid.chars().all(|c| c.is_ascii_digit()) => rest.trim_start(),
            _ => line,
        };
        if call.starts_with(['<', '+', '-']) {
            continue;
        }
        let Some((name, _)) = call.split_once('(') else {
            continue;
        };
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        profile.total += 1;
        *profile.items.entry(name.to_string()).or_default() += 1;
    }
    profile
}

// ---- the judgement ------------------------------------------------------------------------

/// What [`judge`] found: the refused cells (each a block of text whose first line names the
/// cell), what it did not judge and why, and every measured value by cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verdict {
    pub refused: Vec<String>,
    pub notes: Vec<String>,
    pub values: Vec<(String, u64)>,
}

impl Verdict {
    /// The values, the notes, then the refusals.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for (cell, value) in &self.values {
            out.push_str(&format!("{METRIC} {cell} {value}\n"));
        }
        for note in &self.notes {
            out.push_str(&format!("{note}\n"));
        }
        if !self.refused.is_empty() {
            out.push_str(&format!("{METRIC}: {} refused:\n", self.refused.len()));
            for refusal in &self.refused {
                out.push_str(refusal);
            }
        }
        out
    }
}

/// The refusal of one cell: its values and the functions or system calls that grew or are
/// new, largest growth first, at most [`MOST_LISTED`].
fn refusal(cell: &str, tag: &str, unit: Unit, then: &Measure, now: &Measure) -> String {
    let slack = band(unit, then.value);
    let mut out = format!(
        "{METRIC} {cell}: {} at {tag}, {} now, above it by more than the noise band of {slack}\n",
        then.value, now.value
    );
    let mut grown: Vec<(u64, &str, Option<u64>, u64)> = now
        .items
        .iter()
        .filter_map(|(name, &count)| match then.items.get(name) {
            Some(&before) if count > before => {
                Some((count - before, name.as_str(), Some(before), count))
            }
            Some(_) => None,
            None => Some((count, name.as_str(), None, count)),
        })
        .collect();
    grown.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let what = match unit {
        Unit::Instructions => "functions (self instructions)",
        Unit::Syscalls => "system calls",
    };
    if grown.is_empty() {
        out.push_str(&format!("  no {what} grew one by one\n"));
    }
    for (_, name, before, count) in grown.iter().take(MOST_LISTED) {
        match before {
            Some(before) => out.push_str(&format!("  grew {name}: {before} → {count}\n")),
            None => {
                let before = match unit {
                    Unit::Instructions => format!("under {} at {tag}", listed_floor(then.value)),
                    Unit::Syscalls => "0".to_string(),
                };
                out.push_str(&format!("  new {name}: {before} → {count}\n"));
            }
        }
    }
    if grown.len() > MOST_LISTED {
        out.push_str(&format!(
            "  and {} more {what}\n",
            grown.len() - MOST_LISTED
        ));
    }
    out
}

/// Hold the `measured` counts against the `recorded` ones of the same scenario and target. A
/// cell is refused when it is above its recorded value by more than [`band`]; not judged when
/// its recorded runs spread further than the band; shown only when its file is a `first`
/// value. A scenario, target or operation without a recorded value is refused, and so is a
/// recorded operation the run did not measure.
pub fn judge(recorded: &[Counts], measured: &[Counts]) -> Verdict {
    let mut verdict = Verdict::default();
    for now in measured {
        let place = format!("{}/{}", now.target, now.scenario);
        for op in &now.ops {
            for unit in [Unit::Instructions, Unit::Syscalls] {
                verdict.values.push((
                    format!("{place}/{}/{}", op.op, unit.word()),
                    op.measure(unit).value,
                ));
            }
        }
        let Some(then) = recorded
            .iter()
            .find(|c| c.scenario == now.scenario && c.target == now.target)
        else {
            verdict.refused.push(format!(
                "{METRIC} {place}: no recorded counts ({COUNTS}/{}); record them with `ignite-perf path-cost --record first --scenario {}`\n",
                counts_file(&now.scenario, &now.target),
                now.scenario
            ));
            continue;
        };
        let tag = match &then.reference {
            Reference::First(commit) => {
                verdict.notes.push(format!(
                    "{METRIC} {place}: first value, recorded at {commit}; compared from the next release on"
                ));
                continue;
            }
            Reference::Release(tag) => tag,
        };
        for op in &now.ops {
            let Some(before) = then.ops.iter().find(|o| o.op == op.op) else {
                verdict.refused.push(format!(
                    "{METRIC} {place}/{}: no value at {tag}; a frozen scenario gains no operation\n",
                    op.op
                ));
                continue;
            };
            for unit in [Unit::Instructions, Unit::Syscalls] {
                let cell = format!("{place}/{}/{}", op.op, unit.word());
                let (old, new) = (before.measure(unit), op.measure(unit));
                let slack = band(unit, old.value);
                if old.spread() > slack {
                    let runs: Vec<String> = old.runs.iter().map(u64::to_string).collect();
                    verdict.notes.push(format!(
                        "{METRIC} {cell}: not judged, its runs at {tag} spread {} beyond the noise band of {slack}: {} (now {})",
                        old.spread(),
                        runs.join(" "),
                        new.value
                    ));
                } else if new.value > old.value + slack {
                    verdict.refused.push(refusal(&cell, tag, unit, old, new));
                }
            }
        }
        for op in &then.ops {
            if !now.ops.iter().any(|o| o.op == op.op) {
                verdict.refused.push(format!(
                    "{METRIC} {place}/{}: recorded at {tag} and not measured\n",
                    op.op
                ));
            }
        }
    }
    verdict
}

/// One cell of several runs as a count file holds it: the median run's value and items (the
/// lower of the two middle runs for an even count), every run's value, and of the functions only
/// those at or over [`listed_floor`].
pub fn combine(unit: Unit, runs: &[&Profile]) -> Measure {
    let mut order: Vec<usize> = (0..runs.len()).collect();
    order.sort_by_key(|&i| runs[i].total);
    let median = runs[order[(runs.len() - 1) / 2]];
    let mut items = median.items.clone();
    if unit == Unit::Instructions {
        let floor = listed_floor(median.total);
        items.retain(|_, count| *count >= floor);
    }
    Measure {
        value: median.total,
        runs: runs.iter().map(|p| p.total).collect(),
        items,
    }
}

// ---- the mode -----------------------------------------------------------------------------

#[derive(Args, Debug, Clone)]
pub struct PathCostArgs {
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    /// The compiler; default `debug/ignite` under `CARGO_TARGET_DIR`, else under `<repo>/target`.
    #[arg(long)]
    pub cli: Option<PathBuf>,
    /// The directory this mode owns: staged sources, builds, outputs of the instruments.
    #[arg(long)]
    pub work: PathBuf,
    /// The file the measured values are written to, as `{"values": [{metric, target, value}]}`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Write the counts under bench/path-cost/counts instead of judging: `vX.Y.Z` at a release,
    /// `first` for a scenario added after the freeze (with `--scenario`).
    #[arg(long)]
    pub record: Option<String>,
    /// Runs per scenario and target; a recording takes at least 5.
    #[arg(long, default_value_t = 1)]
    pub runs: usize,
    /// Only these scenarios.
    #[arg(long)]
    pub scenario: Vec<String>,
}

fn head_commit(repo: &Path) -> String {
    git_out(repo, &["rev-parse", "--short=12", "HEAD"])
        .map(|out| String::from_utf8_lossy(&out).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// `ignite-perf path-cost`: check the frozen files, measure every scenario on every target
/// `--runs` times, then record the counts (`--record`) or judge them against the recorded ones.
/// Exit 1 when a frozen file differs or a cell is refused.
pub fn main(args: &PathCostArgs) -> R<i32> {
    ensure(args.runs >= 1, "--runs is at least 1")?;
    if let Some(label) = &args.record {
        ensure(
            args.runs >= 5,
            "a recording takes at least 5 runs (--runs 5), so that each cell's spread is known",
        )?;
        ensure(
            label == "first" || label.starts_with('v'),
            "--record takes a release tag `vX.Y.Z` or `first`",
        )?;
        ensure(
            label != "first" || !args.scenario.is_empty(),
            "--record first records the scenarios named by --scenario",
        )?;
    }
    let cpus = crate::path_cost_run::pin_two_cpus()?;
    let scenarios: Vec<Scenario> = read_scenarios(&args.repo.join(SCENARIOS))?
        .into_iter()
        .filter(|s| args.scenario.is_empty() || args.scenario.contains(&s.name))
        .collect();
    ensure(!scenarios.is_empty(), "no scenario to measure")?;
    let (errors, notes) = frozen_in_repository(&args.repo, &scenarios);
    for note in &notes {
        println!("{note}");
    }
    if !errors.is_empty() {
        for error in &errors {
            eprintln!("{error}");
        }
        return Ok(1);
    }
    let cli = args.cli.clone().unwrap_or_else(|| {
        crate::quality::default_cli(&args.repo, std::env::var_os("CARGO_TARGET_DIR").as_deref())
    });
    let mut tools = crate::path_cost_run::Tools::new(&cli, &args.repo, &args.work)?;
    println!(
        "{METRIC}: measuring on CPUs {cpus:?}, {} run(s) each",
        args.runs
    );
    let mut measured = Vec::new();
    for scenario in &scenarios {
        for target in scenario.targets() {
            let mut runs = Vec::new();
            for run in 0..args.runs {
                println!("{METRIC}: {target}/{}, run {}", scenario.name, run + 1);
                runs.push(tools.measure(scenario, &target)?);
            }
            let mut ops = Vec::new();
            for (index, (op, _, _)) in runs[0].iter().enumerate() {
                let instructions: Vec<&Profile> = runs.iter().map(|r| &r[index].1).collect();
                let syscalls: Vec<&Profile> = runs.iter().map(|r| &r[index].2).collect();
                ops.push(OpCounts {
                    op: op.clone(),
                    instructions: combine(Unit::Instructions, &instructions),
                    syscalls: combine(Unit::Syscalls, &syscalls),
                });
            }
            measured.push(Counts {
                scenario: scenario.name.clone(),
                target: target.clone(),
                reference: match args.record.as_deref() {
                    Some("first") => Reference::First(head_commit(&args.repo)),
                    Some(tag) => Reference::Release(tag.to_string()),
                    None => Reference::Release("now".to_string()),
                },
                ops,
            });
        }
    }
    println!("{}", tools.tally.text());
    if args.record.is_some() {
        let dir = args.repo.join(COUNTS);
        fs::create_dir_all(&dir).map_err(io_err(dir.display()))?;
        for counts in &measured {
            let path = dir.join(counts_file(&counts.scenario, &counts.target));
            fs::write(&path, write_counts(counts)).map_err(io_err(path.display()))?;
            for op in &counts.ops {
                for unit in [Unit::Instructions, Unit::Syscalls] {
                    let m = op.measure(unit);
                    let slack = band(unit, m.value);
                    let runs: Vec<String> = m.runs.iter().map(u64::to_string).collect();
                    println!(
                        "{METRIC} {}/{}/{}/{} {} (runs {}; spread {}, band {slack}{})",
                        counts.target,
                        counts.scenario,
                        op.op,
                        unit.word(),
                        m.value,
                        runs.join(" "),
                        m.spread(),
                        if m.spread() > slack {
                            ", not judged"
                        } else {
                            ""
                        }
                    );
                }
            }
            println!("wrote {}", path.display());
        }
        return Ok(0);
    }
    let recorded = read_counts_dir(&args.repo.join(COUNTS))?;
    let verdict = judge(&recorded, &measured);
    if let Some(out) = &args.out {
        let values = verdict
            .values
            .iter()
            .map(|(cell, value)| {
                obj(vec![
                    ("metric", Json::from(METRIC)),
                    ("target", Json::from(cell.as_str())),
                    ("value", Json::Int(*value as i64)),
                ])
            })
            .collect();
        let text = obj(vec![("values", Json::Arr(values))]).dumps_indent(true, 2) + "\n";
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir).map_err(io_err(dir.display()))?;
        }
        fs::write(out, text).map_err(io_err(out.display()))?;
    }
    print!("{}", verdict.report());
    Ok(if verdict.refused.is_empty() { 0 } else { 1 })
}
