//! The host journal: hash-chained records in `host-journal.jsonl`, the
//! state snapshot they carry and its `status.json` copy.
//!
//! Durability (`docs/slice.md` 17.5): every record is written and synced
//! before the host acts on it, so a reply, an acknowledged run record or a
//! dispatched effect never runs ahead of the journal. A failed write or sync
//! truncates the journal back to its last synced record and marks the
//! storage lost: the host then stops before any further mutation.
//!
//! Besides the `state` snapshots, the journal carries an `ownership` record
//! whenever the owned and retiring instances change (pid, OS start time,
//! generation), and a `reconciled` record when recovery has accounted for
//! the effect intents without a recorded completion. [`History::load`]
//! folds both into what the next start recovers from.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use ignite_eval::Json;

use super::Host;
use crate::apply::{Failure, fail, object, required, text};
use crate::run::process::{crash_at, fault};

pub(super) const FORMAT: &str = "ignite-host-journal/0.1";

pub(super) fn sync_directories(root: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sync_directories(&entry.path())?;
        }
    }
    File::open(root)?.sync_all()
}

/// Write `value` to `path` through a synced temporary and a rename. Every
/// write runs on the host's queue thread, one at a time, so a temporary of
/// the same name that exists already is the leftover of a crashed earlier
/// process with this pid (a restarted container's pid 1): it is truncated
/// and written over, never a reason to fail.
pub(super) fn save_json(path: &Path, value: &Json) -> Result<(), Failure> {
    save_bytes(path, (value.compact() + "\n").as_bytes())
}

/// [`save_json`] of raw bytes.
pub(super) fn save_bytes(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let temporary = write_temporary(path, bytes)?;
    replace_with(&temporary, path)?;
    Ok(())
}

/// The first half of [`save_bytes`]: `bytes` written and synced to the
/// temporary beside `path`, which is returned; `path` is untouched.
fn write_temporary(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let temporary = path.with_extension(format!("{}.writing", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(temporary)
}

/// The second half of [`save_bytes`]: rename `temporary` over `path`, then
/// sync the directory so the rename itself is durable.
fn replace_with(temporary: &Path, path: &Path) -> io::Result<()> {
    std::fs::rename(temporary, path)?;
    File::open(path.parent().unwrap())?.sync_all()
}

/// An effect intent: a journaled `Spawn` of `member` whose `Started` or
/// `SpawnFailed` was never recorded, so the effect may or may not have run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Intent {
    pub(super) operation: String,
    pub(super) member: String,
    pub(super) generation: i64,
}

impl Intent {
    pub(super) fn json(&self) -> Json {
        object([
            ("operationId", text(&self.operation)),
            ("member", text(&self.member)),
            ("generation", Json::Int(self.generation)),
            ("effect", text("Spawn")),
        ])
    }
}

/// What a verified journal says about the host before this start.
pub(super) struct History {
    /// The last `state` record's snapshot.
    pub(super) state: Option<Json>,
    /// The owned and retiring instances of the last `ownership` record after
    /// that snapshot.
    pub(super) ownership: Option<(Json, Json)>,
    /// The digest the next record chains to.
    pub(super) last_digest: String,
    /// The effect intents without a recorded completion since the last
    /// `reconciled` record, by member.
    pub(super) pending: Vec<Intent>,
    /// The number of complete records.
    pub(super) records: u64,
    /// The length in bytes of the complete records.
    length: u64,
    /// The bytes after the last newline: a record whose write was never
    /// synced, so the host never acted on it (22.7). Empty when none.
    pub(super) torn: Vec<u8>,
    /// The value of every `journalRepaired` record, in order.
    pub(super) repairs: Vec<Json>,
    /// The `revisionRecord` copies by controller plan: one list per run of
    /// that plan (a run's `seq` starts again at 1), each copy its `seq` and
    /// compact line.
    copies: BTreeMap<String, Vec<Vec<(u64, String)>>>,
}

/// Why a state's journals are not intact (`docs/slice.md` 22.7): the
/// 1-based record (or, for a revision journal, the `seq`), the reason
/// `inspect` reports and, for a revision journal, its path in the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Damage {
    pub(super) record: u64,
    pub(super) reason: &'static str,
    pub(super) file: Option<String>,
}

pub(super) const DIGEST_MISMATCH: &str = "digest mismatch";
pub(super) const UNPARSED: &str = "does not parse";
pub(super) const DIVERGED: &str = "chain diverged";
pub(super) const SHORTER: &str = "shorter than status.json";
pub(super) const REVISION_DIFFERS: &str = "revision journal differs";

impl Damage {
    fn at(record: u64, reason: &'static str) -> Damage {
        Damage {
            record,
            reason,
            file: None,
        }
    }

    /// The host's error line (exit 3).
    pub(super) fn message(&self) -> String {
        match (&self.file, self.reason) {
            (Some(file), _) => format!(
                "{file} record {}: differs from the host journal",
                self.record
            ),
            (None, SHORTER) => format!(
                "host journal ends before record {} that status.json anchors",
                self.record
            ),
            (None, reason) => format!("host journal record {}: {reason}", self.record),
        }
    }

    /// The `journal` field of `inspect`.
    pub(super) fn json(&self) -> Json {
        let mut value = object([
            ("intact", Json::Bool(false)),
            ("record", Json::Int(self.record as i64)),
            ("reason", text(self.reason)),
        ]);
        if let (Some(file), Json::Object(fields)) = (&self.file, &mut value) {
            fields.insert("file".into(), text(file));
        }
        value
    }
}

/// The `journalAnchor` of the `status.json` beside `path`: the record it
/// anchors and that record's digest. A missing or unreadable `status.json`,
/// or one without an anchor (written before slice 18), anchors nothing.
fn anchor(path: &Path) -> Option<(u64, String)> {
    let source = std::fs::read_to_string(path.with_file_name("status.json")).ok()?;
    let status = Json::parse(&source).ok()?;
    let anchor = status.get("journalAnchor")?;
    let record = u64::try_from(anchor.get("record")?.as_i64()?).ok()?;
    Some((record, anchor.get("digest")?.as_str()?.to_string()))
}

/// The run record of a `revisionRecord` value, with its kind and variant.
fn run_record(value: &Json) -> Option<(&str, &Json, &str)> {
    let record = value.get("record")?;
    let kind = record.get("kind").and_then(Json::as_str)?;
    let variant = match kind {
        "effect" => record.get("value")?.get("variant"),
        "event" => record
            .get("value")?
            .get("fields")?
            .get("kind")?
            .get("variant"),
        _ => None,
    }
    .and_then(Json::as_str)?;
    Some((kind, record, variant))
}

/// Whether `kind` and `value` are the record that makes an operation
/// accepted: a `state` whose active operation's receipt is `accepted`.
fn accepts(kind: &str, value: &Json) -> bool {
    kind == "state"
        && value
            .get("activeOperation")
            .and_then(Json::as_str)
            .and_then(|id| value.get("operations")?.get(id)?.get("state"))
            .and_then(Json::as_str)
            == Some("accepted")
}

/// Whether `kind` and `value` are the intent of a `Spawn` effect.
fn spawn_intent(kind: &str, value: &Json) -> bool {
    kind == "revisionRecord"
        && run_record(value)
            .is_some_and(|(kind, _, variant)| kind == "effect" && variant == "Spawn")
}

/// `kind` and `value` chained to `previous`: the record line and its digest.
fn chained(kind: &str, value: Json, previous: &str) -> (String, String) {
    let mut record = object([
        ("kind", text(kind)),
        ("value", value),
        ("previousDigest", text(previous)),
    ]);
    let digest = crate::apply::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
    if let Json::Object(fields) = &mut record {
        fields.insert("digest".into(), text(&digest));
    }
    (record.compact() + "\n", digest)
}

/// Whether the debug fault of [`crate::run::process::FAULT`] fails the sync
/// of this record.
fn sync_fault(kind: &str, value: &Json) -> bool {
    (fault("sync_intent") && accepts(kind, value))
        || (fault("sync_effect") && spawn_intent(kind, value))
}

/// Append one record to `file`, chained to `previous`, and sync it: its
/// digest. On a failed write or sync (`fail_sync` injects one after the
/// write) the file is truncated back to its length before the record, so
/// the chain on disk stays the synced one. The debug crash points of
/// [`crate::run::process::FAULT`] are here, after the sync.
fn append_to(
    file: &mut File,
    previous: &str,
    kind: &str,
    value: &Json,
    fail_sync: bool,
) -> io::Result<String> {
    let before = file.seek(io::SeekFrom::End(0))?;
    let (line, digest) = chained(kind, value.clone(), previous);
    let intent = accepts(kind, value);
    let effect = spawn_intent(kind, value);
    let written = file.write_all(line.as_bytes()).and_then(|()| {
        if fail_sync {
            return Err(io::Error::other("injected journal sync failure"));
        }
        file.sync_data()
    });
    if let Err(error) = written {
        let _ = file.set_len(before).and_then(|()| file.sync_data());
        return Err(error);
    }
    if intent {
        crash_at("after_intent");
    }
    if effect {
        crash_at("before_dispatch");
    }
    if kind == "transition" {
        crash_at("mid_transition");
    }
    Ok(digest)
}

impl History {
    /// Verify the journal at `path`: its digest chain and the anchor of the
    /// `status.json` beside it. Read only: a torn final line is kept in
    /// [`History::torn`] for [`Writer::repair`]. A damaged journal is exit 3
    /// naming the record (22.7).
    pub(super) fn load(path: &Path) -> Result<History, Failure> {
        History::scan(path)?.map_err(|damage| fail(3, damage.message()))
    }

    /// [`History::load`] with the damage as a value, for `inspect`; the
    /// outer error is a failed read. A missing journal reads as an empty
    /// one, so the anchor check still runs: a `status.json` that anchors a
    /// record refuses it (row 22 at record 0).
    pub(super) fn scan(path: &Path) -> Result<Result<History, Damage>, Failure> {
        let mut history = History {
            state: None,
            ownership: None,
            last_digest: String::new(),
            pending: Vec::new(),
            records: 0,
            length: 0,
            torn: Vec::new(),
            repairs: Vec::new(),
            copies: BTreeMap::new(),
        };
        let mut bytes = Vec::new();
        if path.exists() {
            File::open(path)?.read_to_end(&mut bytes)?;
        }
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |last| last + 1);
        history.torn = bytes[complete..].to_vec();
        history.length = complete as u64;
        let anchor = anchor(path);
        let mut anchored = None;
        let mut pending: BTreeMap<String, Intent> = BTreeMap::new();
        for (index, line) in bytes[..complete]
            .split_inclusive(|byte| *byte == b'\n')
            .enumerate()
        {
            let number = index as u64 + 1;
            let line = &line[..line.len() - 1];
            let parsed = std::str::from_utf8(line)
                .ok()
                .and_then(|line| Json::parse(line).ok())
                .and_then(|record| {
                    let digest = record.get("digest")?.as_str()?.to_string();
                    let previous = record.get("previousDigest")?.as_str()?.to_string();
                    Some((record, digest, previous))
                });
            let Some((mut record, digest, previous)) = parsed else {
                return Ok(Err(Damage::at(number, UNPARSED)));
            };
            if previous != history.last_digest {
                return Ok(Err(Damage::at(number, DIVERGED)));
            }
            if let Json::Object(fields) = &mut record {
                fields.remove("digest");
            }
            let expected =
                crate::apply::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
            if digest != expected {
                return Ok(Err(Damage::at(number, DIGEST_MISMATCH)));
            }
            if anchor.as_ref().is_some_and(|(at, _)| *at == number) {
                anchored = Some(digest.clone());
            }
            history.last_digest = digest;
            history.records = number;
            let value = record.get("value").cloned().unwrap_or(Json::Null);
            match record.get("kind").and_then(Json::as_str) {
                Some("journalRepaired") => history.repairs.push(value.clone()),
                Some("revisionRecord") => history.copy(&value),
                _ => {}
            }
            match record.get("kind").and_then(Json::as_str) {
                Some("state") => {
                    history.state = Some(value);
                    history.ownership = None;
                }
                Some("ownership") => {
                    history.ownership = Some((
                        value.get("ownership").cloned().unwrap_or(Json::Null),
                        value
                            .get("retiringInstances")
                            .cloned()
                            .unwrap_or(Json::Null),
                    ));
                }
                Some("reconciled") => pending.clear(),
                Some("revisionRecord") => {
                    let Some((kind, run, variant)) = run_record(&value) else {
                        continue;
                    };
                    let operation = value
                        .get("operationId")
                        .and_then(Json::as_str)
                        .unwrap_or_default();
                    let machine = run.get("machine").and_then(Json::as_str).unwrap_or("");
                    let member = machine
                        .strip_suffix(".supervisor")
                        .map(str::to_string)
                        .or_else(|| {
                            run.get("value")?
                                .get("fields")?
                                .as_array()?
                                .first()?
                                .as_str()
                                .map(str::to_string)
                        })
                        .unwrap_or_default();
                    match (kind, variant) {
                        ("effect", "Spawn") => {
                            let generation = run
                                .get("value")
                                .and_then(|value| value.get("fields"))
                                .and_then(Json::as_array)
                                .and_then(|fields| fields.get(1))
                                .and_then(Json::as_i64)
                                .unwrap_or(0);
                            pending.insert(
                                member.clone(),
                                Intent {
                                    operation: operation.to_string(),
                                    member,
                                    generation,
                                },
                            );
                        }
                        ("event", "Started" | "SpawnFailed")
                            if machine.ends_with(".supervisor") =>
                        {
                            pending.remove(&member);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        history.pending = pending.into_values().collect();
        if let Some((record, digest)) = anchor
            && anchored.as_ref() != Some(&digest)
        {
            return Ok(Err(Damage::at(record, SHORTER)));
        }
        Ok(Ok(history))
    }

    /// Keep the run record of a `revisionRecord` value as the copy of its
    /// revision journal's line. A `seq` that does not rise starts a new run:
    /// every run of a plan writes a fresh revision journal from `seq` 1.
    fn copy(&mut self, value: &Json) {
        let (Some(plan), Some(record)) = (
            value.get("controllerPlanId").and_then(Json::as_str),
            value.get("record"),
        ) else {
            return;
        };
        let Some(seq) = record
            .get("seq")
            .and_then(Json::as_i64)
            .and_then(|seq| u64::try_from(seq).ok())
        else {
            return;
        };
        let runs = self.copies.entry(plan.to_string()).or_default();
        match runs.last_mut() {
            Some(run) if run.last().is_some_and(|(last, _)| *last < seq) => {
                run.push((seq, record.compact()));
            }
            _ => runs.push(vec![(seq, record.compact())]),
        }
    }
}

/// A retained revision journal: its directory name and complete lines,
/// indexed by `seq` (the header is `seq` 0).
struct Retained {
    name: String,
    lines: Vec<Vec<u8>>,
}

impl Retained {
    /// How many leading copies of `run` equal their lines.
    fn agrees(&self, run: &[(u64, String)]) -> usize {
        run.iter()
            .take_while(|(seq, copy)| {
                usize::try_from(*seq)
                    .ok()
                    .and_then(|seq| self.lines.get(seq))
                    .is_some_and(|line| line.as_slice() == copy.as_bytes())
            })
            .count()
    }
}

/// Assign `run` (and, recursively, the runs it displaces) to a journal it
/// fits: the augmenting step of a bipartite matching.
fn assign(run: usize, fits: &[Vec<bool>], owner: &mut [Option<usize>], seen: &mut [bool]) -> bool {
    for journal in 0..owner.len() {
        if fits[run][journal] && !seen[journal] {
            seen[journal] = true;
            if owner[journal].is_none_or(|other| assign(other, fits, owner, seen)) {
                owner[journal] = Some(run);
                return true;
            }
        }
    }
    false
}

/// Check every retained revision journal under `root` against the
/// `revisionRecord` copies of `history` (22.7). The copies of a plan come in
/// runs; each run must belong to one revision journal of that plan whose
/// lines equal every copy of it. Lines after a run's last copy were written
/// and never acknowledged, and are kept. When no assignment fits, the first
/// differing line of the closest unassigned journal and run is the damage.
pub(super) fn check_revisions(root: &Path, history: &History) -> Result<Option<Damage>, Failure> {
    let mut journals: BTreeMap<String, Vec<Retained>> = BTreeMap::new();
    let revisions = root.join("revisions");
    if revisions.is_dir() {
        let mut directories = Vec::new();
        for entry in std::fs::read_dir(&revisions)? {
            let path = entry?.path();
            if path.is_dir() {
                directories.push(path);
            }
        }
        directories.sort();
        for path in directories {
            let plan = std::fs::read_to_string(path.join("context.json"))
                .ok()
                .and_then(|source| Json::parse(&source).ok())
                .and_then(|context| context.get("planId")?.as_str().map(str::to_string));
            let (Some(plan), Ok(bytes)) = (plan, std::fs::read(path.join("journal.jsonl"))) else {
                continue;
            };
            let complete = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |last| last + 1);
            let lines = bytes[..complete]
                .split_inclusive(|byte| *byte == b'\n')
                .map(|line| line[..line.len() - 1].to_vec())
                .collect();
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            journals
                .entry(plan)
                .or_default()
                .push(Retained { name, lines });
        }
    }
    for (plan, runs) in &history.copies {
        let retained = journals.get(plan).map_or(&[][..], Vec::as_slice);
        let fits: Vec<Vec<bool>> = runs
            .iter()
            .map(|run| {
                retained
                    .iter()
                    .map(|journal| journal.agrees(run) == run.len())
                    .collect()
            })
            .collect();
        let mut owner = vec![None; retained.len()];
        let mut unassigned = Vec::new();
        for run in 0..runs.len() {
            if !assign(run, &fits, &mut owner, &mut vec![false; retained.len()]) {
                unassigned.push(run);
            }
        }
        let closest = unassigned
            .iter()
            .flat_map(|run| {
                owner
                    .iter()
                    .enumerate()
                    .filter(|(_, owner)| owner.is_none())
                    .map(move |(journal, _)| (*run, journal))
            })
            .map(|(run, journal)| (retained[journal].agrees(&runs[run]), run, journal))
            .min_by_key(|(agrees, run, journal)| (usize::MAX - agrees, *run, *journal));
        if let Some((agrees, run, journal)) = closest {
            return Ok(Some(Damage {
                record: runs[run][agrees].0,
                reason: REVISION_DIFFERS,
                file: Some(format!(
                    "revisions/{}/journal.jsonl",
                    retained[journal].name
                )),
            }));
        }
    }
    Ok(None)
}

/// The host journal open for appending, with what the host reports about
/// it: the number of its records (the `journalAnchor` of `status.json`) and
/// its `journalRepaired` records.
pub(super) struct Writer {
    path: PathBuf,
    file: File,
    pub(super) records: u64,
    pub(super) repairs: Vec<Json>,
}

impl Writer {
    /// Replace a torn final line of `history` by a `journalRepaired` record
    /// (D9) in one atomic step: the complete records and the repair record
    /// are written to a temporary in the journal's directory and synced,
    /// the temporary is renamed over the journal, the directory is synced,
    /// and the journal is reopened for appending. The journal on disk is
    /// therefore either the old one, torn line included, which the next
    /// start repairs again, or the repaired one; never the good records
    /// without the repair record. The repair's value, or `None` when
    /// nothing was torn. Takes the history's records and repairs either
    /// way.
    pub(super) fn repair(&mut self, history: &mut History) -> Result<Option<Json>, Failure> {
        self.repair_until(history, false)
    }

    /// [`Writer::repair`]; `crash_after_temporary` stops it with an error
    /// once the temporary is synced and before the rename, the crash the
    /// unit test `a_crash_during_the_repair_leaves_the_torn_journal` takes.
    fn repair_until(
        &mut self,
        history: &mut History,
        crash_after_temporary: bool,
    ) -> Result<Option<Json>, Failure> {
        self.records = history.records;
        let repaired = if history.torn.is_empty() {
            None
        } else {
            let value = object([
                ("droppedBytes", Json::Int(history.torn.len() as i64)),
                (
                    "droppedDigest",
                    text(crate::apply::digest::bytes(&history.torn)),
                ),
                ("previousDigest", text(&history.last_digest)),
            ]);
            let mut bytes = Vec::new();
            File::open(&self.path)?.read_to_end(&mut bytes)?;
            let Some(complete) = bytes.get(..history.length as usize) else {
                return Err(fail(
                    3,
                    "host journal: shorter than its loaded records while repairing",
                ));
            };
            let (line, digest) = chained("journalRepaired", value.clone(), &history.last_digest);
            let mut journal = complete.to_vec();
            journal.extend_from_slice(line.as_bytes());
            let temporary = write_temporary(&self.path, &journal)?;
            if crash_after_temporary {
                return Err(io::Error::other("injected crash before the journal rename").into());
            }
            replace_with(&temporary, &self.path)?;
            self.file = OpenOptions::new().append(true).open(&self.path)?;
            history.last_digest = digest;
            history.records += 1;
            history.length = journal.len() as u64;
            history.torn.clear();
            history.repairs.push(value.clone());
            self.records = history.records;
            Some(value)
        };
        self.repairs = history.repairs.clone();
        Ok(repaired)
    }
}

/// Open the journal for appending; a journal created here has its directory
/// entry synced before the first record.
pub(super) fn open(path: &Path) -> Result<Writer, Failure> {
    let existed = path.exists();
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    if !existed {
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    Ok(Writer {
        path: path.to_path_buf(),
        file,
        records: 0,
        repairs: Vec::new(),
    })
}

impl Host {
    pub(super) fn snapshot(&self, online: bool) -> Json {
        object([
            ("formatVersion", text(FORMAT)),
            ("stateIdentity", text(&self.identity)),
            ("online", Json::Bool(online)),
            (
                "runtimeOperation",
                self.runtime
                    .as_ref()
                    .map_or(Json::Null, |runtime| text(&runtime.operation)),
            ),
            ("committedRevision", Json::Int(self.revision)),
            (
                "desiredRevision",
                Json::Int(if self.active.is_some() && self.active != self.committed {
                    self.revision + 1
                } else {
                    self.revision
                }),
            ),
            (
                "desiredPlan",
                self.active
                    .as_ref()
                    .or(self.committed.as_ref())
                    .map_or(Json::Null, text),
            ),
            (
                "committedPlan",
                self.committed.as_ref().map_or(Json::Null, text),
            ),
            (
                "activeOperation",
                self.active.as_ref().map_or(Json::Null, text),
            ),
            (
                "recoveryReason",
                self.recovery_reason
                    .as_deref()
                    .or_else(|| {
                        self.recovery
                            .as_ref()
                            .and_then(|recovery| recovery.get("reason"))
                            .and_then(Json::as_str)
                    })
                    .map_or(Json::Null, text),
            ),
            ("recovery", self.recovery.clone().unwrap_or(Json::Null)),
            ("history", Json::Array(self.history.clone())),
            ("operations", Json::Object(self.receipts.clone())),
            ("ownership", Json::Object(self.owned.clone())),
            ("retiringInstances", Json::Object(self.retiring.clone())),
            ("lastStandings", Json::Object(self.standings.clone())),
            ("journalRepairs", Json::Array(self.journal.repairs.clone())),
        ])
    }

    /// Take the fields of a journaled snapshot; the inverse of `snapshot`.
    /// A recovery reason that only reported a startup recovery
    /// (`reducer_revision_changed`, with or without a block kept beside it)
    /// is not taken: the next start recomputes it, or keeps it from the
    /// restored `recovery` until the next commit ([`Host::recover`]).
    pub(super) fn restore(&mut self, previous: &Json) -> Result<(), Failure> {
        if required(previous, "formatVersion")? != FORMAT
            || required(previous, "stateIdentity")? != self.identity
        {
            return Err(fail(3, "host journal version/state identity mismatch"));
        }
        self.revision = previous
            .get("committedRevision")
            .and_then(Json::as_i64)
            .unwrap_or(0);
        self.committed = previous
            .get("committedPlan")
            .and_then(Json::as_str)
            .map(str::to_string);
        self.active = previous
            .get("activeOperation")
            .and_then(Json::as_str)
            .map(str::to_string);
        self.recovery_reason = previous
            .get("recoveryReason")
            .and_then(Json::as_str)
            .filter(|reason| *reason != super::recovery::REDUCER_CHANGED)
            .map(str::to_string);
        self.recovery = previous
            .get("recovery")
            .filter(|recovery| !matches!(recovery, Json::Null))
            .cloned();
        self.receipts = previous
            .get("operations")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        self.owned = previous
            .get("ownership")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        self.retiring = previous
            .get("retiringInstances")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        self.standings = previous
            .get("lastStandings")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        Ok(())
    }

    /// Take the instances of the last `ownership` record after the restored
    /// snapshot.
    pub(super) fn restore_ownership(&mut self, owned: &Json, retiring: &Json) {
        self.owned = owned.as_object().cloned().unwrap_or_default();
        self.retiring = retiring.as_object().cloned().unwrap_or_default();
        self.journaled_ownership = (self.owned.clone(), self.retiring.clone());
    }

    /// Append and sync one record. A failure marks the storage lost; the
    /// queue stops the host before it handles anything else.
    pub(super) fn append(&mut self, kind: &str, value: Json) -> Result<(), Failure> {
        if let Some(reason) = &self.storage_failed {
            return Err(fail(3, format!("host storage failed earlier: {reason}")));
        }
        let fail_sync = sync_fault(kind, &value);
        match append_to(
            &mut self.journal.file,
            &self.last_digest,
            kind,
            &value,
            fail_sync,
        ) {
            Ok(digest) => {
                self.last_digest = digest;
                self.journal.records += 1;
                Ok(())
            }
            Err(error) => {
                let reason = format!("host journal {kind} record not durable: {error}");
                self.storage_failed = Some(reason.clone());
                Err(fail(3, reason))
            }
        }
    }

    /// Journal the snapshot as a `state` record, then copy it to
    /// `status.json` with the `journalAnchor` of that record (22.7): a later
    /// start refuses a journal that no longer holds it.
    pub(super) fn persist(&mut self) -> Result<(), Failure> {
        let mut snapshot = self.snapshot(true);
        self.append("state", snapshot.clone())?;
        self.journaled_ownership = (self.owned.clone(), self.retiring.clone());
        if let Json::Object(fields) = &mut snapshot {
            fields.insert(
                "journalAnchor".into(),
                object([
                    ("record", Json::Int(self.journal.records as i64)),
                    ("digest", text(&self.last_digest)),
                ]),
            );
        }
        save_json(&self.root.join("status.json"), &snapshot)
    }

    /// Journal the owned and retiring instances when they changed since they
    /// were last journaled, before the host acknowledges what changed them:
    /// recovery proves an instance by the pid and start time recorded here.
    pub(super) fn journal_ownership(&mut self) -> Result<(), Failure> {
        if self.journaled_ownership.0 == self.owned && self.journaled_ownership.1 == self.retiring {
            return Ok(());
        }
        self.append(
            "ownership",
            object([
                ("ownership", Json::Object(self.owned.clone())),
                ("retiringInstances", Json::Object(self.retiring.clone())),
            ]),
        )?;
        self.journaled_ownership = (self.owned.clone(), self.retiring.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ignite-host-journal-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("host-journal.jsonl")
    }

    fn run(operation: &str, machine: &str, kind: &str, value: Json) -> Json {
        object([
            ("operationId", text(operation)),
            ("controllerPlanId", text(operation)),
            (
                "record",
                object([
                    ("kind", text(kind)),
                    ("machine", text(machine)),
                    ("value", value),
                ]),
            ),
        ])
    }

    fn spawn(member: &str, generation: i64) -> Json {
        object([
            ("variant", text("Spawn")),
            (
                "fields",
                Json::Array(vec![text(member), Json::Int(generation)]),
            ),
        ])
    }

    fn event(variant: &str) -> Json {
        object([(
            "fields",
            object([("kind", object([("variant", text(variant))]))]),
        )])
    }

    fn write(path: &Path, records: &[(&str, Json)]) {
        let mut file = open(path).unwrap().file;
        let mut previous = String::new();
        for (kind, value) in records {
            previous = append_to(&mut file, &previous, kind, value, false).unwrap();
        }
    }

    #[test]
    fn spawn_intents_without_a_recorded_start_are_pending() {
        let path = scratch("pending");
        write(
            &path,
            &[
                (
                    "revisionRecord",
                    run("op", "T.server.supervisor", "effect", spawn("T.server", 1)),
                ),
                (
                    "revisionRecord",
                    run("op", "T.server.supervisor", "event", event("Started")),
                ),
                (
                    "revisionRecord",
                    run("op", "T.server.supervisor", "effect", spawn("T.server", 2)),
                ),
                (
                    "revisionRecord",
                    run("op", "T.client.supervisor", "effect", spawn("T.client", 1)),
                ),
                (
                    "revisionRecord",
                    run("op", "T.client.supervisor", "event", event("SpawnFailed")),
                ),
            ],
        );
        let history = History::load(&path).unwrap();
        assert_eq!(
            history.pending,
            vec![Intent {
                operation: "op".into(),
                member: "T.server".into(),
                generation: 2,
            }]
        );
    }

    #[test]
    fn a_reconciled_record_settles_the_pending_intents() {
        let path = scratch("reconciled");
        write(
            &path,
            &[
                (
                    "revisionRecord",
                    run("op", "T.server.supervisor", "effect", spawn("T.server", 1)),
                ),
                ("reconciled", object([("outcome", text("ended"))])),
            ],
        );
        assert!(History::load(&path).unwrap().pending.is_empty());
    }

    #[test]
    fn the_last_ownership_record_after_the_snapshot_is_recovered() {
        let path = scratch("ownership");
        let owner = |pid| object([("T.server", object([("pid", Json::Int(pid))]))]);
        write(
            &path,
            &[
                ("ownership", object([("ownership", owner(1))])),
                ("state", object([("ownership", owner(2))])),
                ("ownership", object([("ownership", owner(3))])),
            ],
        );
        let history = History::load(&path).unwrap();
        assert_eq!(history.ownership.map(|(owned, _)| owned), Some(owner(3)));
    }

    /// Row 24: one byte inside the `value` of the third record changed, the
    /// line still valid JSON.
    #[test]
    fn an_altered_record_names_its_index() {
        let path = scratch("altered");
        write(
            &path,
            &[
                ("state", object([("n", text("a"))])),
                ("state", object([("n", text("b"))])),
                ("state", object([("n", text("c"))])),
                ("state", object([("n", text("d"))])),
            ],
        );
        let content = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        lines[2] = lines[2].replace("\"n\":\"c\"", "\"n\":\"x\"");
        assert!(Json::parse(&lines[2]).is_ok());
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();
        let error = History::load(&path)
            .err()
            .expect("an altered record is refused");
        assert_eq!(error.status, 3);
        assert_eq!(error.reason, "host journal record 3: digest mismatch");
    }

    /// The chain continues after the repair of a torn line (since slice 18
    /// the load itself is read only and the repair drops the line, 22.7).
    #[test]
    fn a_torn_final_line_is_dropped_and_the_chain_continues() {
        let path = scratch("torn");
        write(&path, &[("state", object([]))]);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"kind\":\"sta").unwrap();
        let mut history = History::load(&path).unwrap();
        assert!(history.state.is_some());
        let mut writer = open(&path).unwrap();
        writer.repair(&mut history).unwrap();
        append_to(
            &mut writer.file,
            &history.last_digest,
            "state",
            &object([]),
            false,
        )
        .unwrap();
        assert!(History::load(&path).is_ok());
    }

    /// Row 24 (row 20's bytes): the load leaves the torn bytes on disk, the
    /// repair replaces them with one `journalRepaired` record.
    #[test]
    fn a_torn_final_line_is_dropped_and_repaired() {
        let path = scratch("repaired");
        write(&path, &[("state", object([]))]);
        let good = std::fs::read(&path).unwrap();
        let torn = b"{\"kind\":\"sta";
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(torn).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut history = History::load(&path).unwrap();
        assert_eq!(history.torn, torn);
        assert_eq!(history.records, 1);
        let previous = history.last_digest.clone();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the load is read only"
        );
        let mut writer = open(&path).unwrap();
        let repaired = writer.repair(&mut history).unwrap();
        let expected = object([
            ("droppedBytes", Json::Int(12)),
            ("droppedDigest", text(crate::apply::digest::bytes(torn))),
            ("previousDigest", text(&previous)),
        ]);
        assert_eq!(repaired.as_ref(), Some(&expected));
        assert_eq!(writer.records, 2);
        let after = std::fs::read(&path).unwrap();
        assert!(after.starts_with(&good), "the good records stay");
        let last = std::str::from_utf8(&after[good.len()..]).unwrap();
        assert_eq!(
            last.matches('\n').count(),
            1,
            "one record replaces the bytes"
        );
        let record = Json::parse(last.trim_end()).unwrap();
        assert_eq!(
            record.get("kind").and_then(Json::as_str),
            Some("journalRepaired")
        );
        assert_eq!(record.get("value"), Some(&expected));
        let reloaded = History::load(&path).unwrap();
        assert!(reloaded.torn.is_empty());
        assert_eq!(reloaded.records, 2);
        assert_eq!(reloaded.repairs, vec![expected]);
        assert_eq!(reloaded.last_digest, history.last_digest);
        let mut writer = open(&path).unwrap();
        let mut reloaded = reloaded;
        assert_eq!(writer.repair(&mut reloaded).unwrap(), None);
        assert_eq!(writer.repairs.len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), after);
    }

    /// D9, 22.7: a host that dies during the repair, after the repaired
    /// temporary is synced and before its rename, leaves the old journal
    /// byte for byte, torn line included, and the next start repairs it
    /// over the leftover temporary; the good records never stand without
    /// the `journalRepaired` record.
    #[test]
    fn a_crash_during_the_repair_leaves_the_torn_journal() {
        let path = scratch("repair-crash");
        write(&path, &[("state", object([])), ("state", object([]))]);
        let good = std::fs::read(&path).unwrap();
        let torn = b"{\"kind\":\"sta";
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(torn).unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut history = History::load(&path).unwrap();
        let mut writer = open(&path).unwrap();
        let error = writer
            .repair_until(&mut history, true)
            .expect_err("the injected crash stops the repair");
        assert_eq!(error.reason, "injected crash before the journal rename");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the journal is untouched until the rename"
        );
        let leftover = path.with_extension(format!("{}.writing", std::process::id()));
        assert!(leftover.exists(), "the temporary stays behind");
        drop(writer);

        let mut history = History::load(&path).unwrap();
        assert_eq!(history.torn, torn);
        assert_eq!(history.records, 2);
        let mut writer = open(&path).unwrap();
        let repaired = writer.repair(&mut history).unwrap().expect("a repair");
        assert_eq!(repaired.get("droppedBytes"), Some(&Json::Int(12)));
        assert!(!leftover.exists(), "the rename consumed the temporary");
        let after = std::fs::read(&path).unwrap();
        assert!(after.starts_with(&good), "the good records stay");
        let reloaded = History::load(&path).unwrap();
        assert!(reloaded.torn.is_empty());
        assert_eq!(reloaded.records, 3);
        assert_eq!(reloaded.repairs, vec![repaired]);
        assert_eq!(reloaded.last_digest, history.last_digest);
        append_to(
            &mut writer.file,
            &history.last_digest,
            "state",
            &object([]),
            false,
        )
        .unwrap();
        assert_eq!(
            History::load(&path).unwrap().records,
            4,
            "appends after the repair go to the renamed journal"
        );
    }

    /// A line that is no record, and a record chained to another digest,
    /// are named by their line.
    #[test]
    fn an_unparsed_or_rechained_record_names_its_index() {
        let path = scratch("unparsed");
        write(&path, &[("state", object([])), ("state", object([]))]);
        let good = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, good.clone() + "not json\n").unwrap();
        let error = History::load(&path).err().unwrap();
        assert_eq!(error.reason, "host journal record 3: does not parse");
        let lines: Vec<&str> = good.lines().collect();
        std::fs::write(&path, format!("{}\n", lines[1])).unwrap();
        let error = History::load(&path).err().unwrap();
        assert_eq!(error.reason, "host journal record 1: chain diverged");
    }

    fn status(path: &Path, anchor: Option<(i64, &str)>) {
        let mut status = object([("formatVersion", text(FORMAT))]);
        if let (Some((record, digest)), Json::Object(fields)) = (anchor, &mut status) {
            fields.insert(
                "journalAnchor".into(),
                object([("record", Json::Int(record)), ("digest", text(digest))]),
            );
        }
        save_json(&path.with_file_name("status.json"), &status).unwrap();
    }

    /// Row 24: a journal cut at a line boundary before the record
    /// `status.json` anchors, or holding another record there, is refused.
    #[test]
    fn a_journal_shorter_than_its_anchor_is_refused() {
        let path = scratch("anchor");
        write(
            &path,
            &[
                ("state", object([("n", Json::Int(1))])),
                ("state", object([("n", Json::Int(2))])),
                ("state", object([("n", Json::Int(3))])),
            ],
        );
        let full = std::fs::read_to_string(&path).unwrap();
        let digest = History::load(&path).unwrap().last_digest;
        status(&path, Some((3, &digest)));
        assert_eq!(History::load(&path).unwrap().records, 3);
        let lines: Vec<&str> = full.lines().collect();
        std::fs::write(&path, format!("{}\n{}\n", lines[0], lines[1])).unwrap();
        let error = History::load(&path)
            .err()
            .expect("a cut journal is refused");
        assert_eq!(error.status, 3);
        assert_eq!(
            error.reason,
            "host journal ends before record 3 that status.json anchors"
        );
        assert_eq!(
            History::scan(&path).unwrap().err(),
            Some(Damage {
                record: 3,
                reason: SHORTER,
                file: None
            })
        );
        std::fs::write(&path, &full).unwrap();
        status(&path, Some((2, &digest)));
        let error = History::load(&path)
            .err()
            .expect("another digest is refused");
        assert_eq!(
            error.reason,
            "host journal ends before record 2 that status.json anchors"
        );
    }

    /// Row 24: no `status.json`, or one without `journalAnchor` (before
    /// slice 18), loads as before.
    #[test]
    fn a_missing_anchor_is_accepted() {
        let path = scratch("no-anchor");
        write(&path, &[("state", object([])), ("state", object([]))]);
        assert_eq!(History::load(&path).unwrap().records, 2);
        status(&path, None);
        assert_eq!(History::load(&path).unwrap().records, 2);
    }

    /// A revision directory of `plan` under the state of `path` with the
    /// header and `records` as its journal lines.
    fn revision(path: &Path, name: &str, plan: &str, records: &[Json]) {
        let directory = path.with_file_name("revisions").join(name);
        std::fs::create_dir_all(&directory).unwrap();
        save_json(
            &directory.join("context.json"),
            &object([("planId", text(plan))]),
        )
        .unwrap();
        let mut lines = String::from("{\"header\":true}\n");
        for record in records {
            lines.push_str(&record.compact());
            lines.push('\n');
        }
        std::fs::write(directory.join("journal.jsonl"), lines).unwrap();
    }

    fn line_record(seq: i64, at: i64) -> Json {
        object([
            ("seq", Json::Int(seq)),
            ("kind", text("event")),
            ("at", Json::Int(at)),
            ("machine", text("T.server.supervisor")),
        ])
    }

    fn copy(plan: &str, record: &Json) -> (&'static str, Json) {
        (
            "revisionRecord",
            object([
                ("operationId", text(plan)),
                ("controllerPlanId", text(plan)),
                ("record", record.clone()),
            ]),
        )
    }

    /// Row 23 at unit level: two runs of one plan, each in its own revision
    /// journal, one with a line written and never acknowledged. Equal
    /// journals pass; one changed acknowledged line is named by its `seq`
    /// and file.
    #[test]
    fn a_revision_journal_line_that_differs_from_its_copy_is_named() {
        let path = scratch("revisions");
        let first = [line_record(1, 10), line_record(2, 20)];
        let second = [line_record(1, 30), line_record(2, 40), line_record(3, 50)];
        write(
            &path,
            &[
                copy("p", &first[0]),
                copy("p", &first[1]),
                copy("p", &second[0]),
                copy("p", &second[1]),
            ],
        );
        revision(&path, "1-9-2", "p", &first);
        revision(&path, "1-8-5", "p", &second);
        let history = History::load(&path).unwrap();
        let root = path.parent().unwrap();
        assert_eq!(check_revisions(root, &history).unwrap(), None);
        revision(
            &path,
            "1-8-5",
            "p",
            &[second[0].clone(), line_record(2, 41)],
        );
        assert_eq!(
            check_revisions(root, &history).unwrap(),
            Some(Damage {
                record: 2,
                reason: REVISION_DIFFERS,
                file: Some("revisions/1-8-5/journal.jsonl".into()),
            })
        );
        assert_eq!(
            check_revisions(root, &history).unwrap().unwrap().message(),
            "revisions/1-8-5/journal.jsonl record 2: differs from the host journal"
        );
    }

    #[test]
    fn a_failed_append_leaves_the_synced_chain_intact() {
        let path = scratch("failed");
        write(&path, &[("state", object([]))]);
        let before = std::fs::read(&path).unwrap();
        let history = History::load(&path).unwrap();
        let mut file = open(&path).unwrap().file;
        let failed = append_to(&mut file, &history.last_digest, "state", &object([]), true);
        assert!(failed.is_err(), "an injected sync failure is reported");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the record written before the failed sync is truncated"
        );
        let mut read_only = OpenOptions::new().read(true).open(&path).unwrap();
        assert!(
            append_to(
                &mut read_only,
                &history.last_digest,
                "state",
                &object([]),
                false
            )
            .is_err(),
            "a failed write is reported"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let digest =
            append_to(&mut file, &history.last_digest, "state", &object([]), false).unwrap();
        let reloaded = History::load(&path).unwrap();
        assert_eq!(reloaded.last_digest, digest, "the chain continues intact");
    }

    /// A host killed between creating `status.<pid>.writing` and its rename
    /// leaves the temporary behind; a restarted host with the same pid (pid 1
    /// of a container, any pid namespace) must still persist its snapshot, or
    /// it can never start again on that state.
    #[test]
    fn a_crash_leftover_temporary_does_not_block_the_next_save() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/adversary-u5")
            .join(format!(
                "leftover-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("status.json");
        let temporary = path.with_extension(format!("{}.writing", std::process::id()));
        std::fs::write(&temporary, b"{\"torn\":").unwrap();
        let saved = save_json(&path, &object([("saved", Json::Bool(true))]));
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            saved.is_ok(),
            "a crash leftover blocks every later save: {saved:?}"
        );
        assert_eq!(content, "{\"saved\":true}\n");
    }
}
