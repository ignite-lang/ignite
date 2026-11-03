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
use std::path::Path;

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
    let temporary = path.with_extension(format!("{}.writing", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all((value.compact() + "\n").as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
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
    /// Verify the digest chain of the journal at `path`, dropping a torn
    /// final line.
    pub(super) fn load(path: &Path) -> Result<History, Failure> {
        let mut history = History {
            state: None,
            ownership: None,
            last_digest: String::new(),
            pending: Vec::new(),
        };
        if !path.exists() {
            return Ok(history);
        }
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |last| last + 1);
        if complete < bytes.len() {
            file.set_len(complete as u64)?;
            file.sync_data()?;
        }
        let content =
            std::str::from_utf8(&bytes[..complete]).map_err(|error| fail(3, error.to_string()))?;
        let mut pending: BTreeMap<String, Intent> = BTreeMap::new();
        for line in content.lines() {
            let mut record = Json::parse(line).map_err(|reason| fail(3, reason))?;
            let digest = required(&record, "digest")?.to_string();
            if required(&record, "previousDigest")? != history.last_digest {
                return Err(fail(3, "host journal digest chain diverged"));
            }
            if let Json::Object(fields) = &mut record {
                fields.remove("digest");
            }
            let expected =
                crate::apply::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
            if digest != expected {
                return Err(fail(3, "host journal record digest mismatch"));
            }
            history.last_digest = digest;
            let value = record.get("value").cloned().unwrap_or(Json::Null);
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
        Ok(history)
    }
}

/// Open the journal for appending; a journal created here has its directory
/// entry synced before the first record.
pub(super) fn open(path: &Path) -> Result<File, Failure> {
    let existed = path.exists();
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    if !existed {
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    Ok(file)
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
        ])
    }

    /// Take the fields of a journaled snapshot; the inverse of `snapshot`.
    /// A recovery reason that only reported a startup recovery without
    /// blocking (`reducer_revision_changed`) is not taken: the next start
    /// recomputes it.
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
        let reported = previous
            .get("recovery")
            .filter(|recovery| recovery.get("blocking").and_then(Json::as_bool) == Some(false))
            .and_then(|recovery| recovery.get("reason"))
            .and_then(Json::as_str);
        self.recovery_reason = previous
            .get("recoveryReason")
            .and_then(Json::as_str)
            .filter(|reason| Some(*reason) != reported)
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
            &mut self.journal,
            &self.last_digest,
            kind,
            &value,
            fail_sync,
        ) {
            Ok(digest) => {
                self.last_digest = digest;
                Ok(())
            }
            Err(error) => {
                let reason = format!("host journal {kind} record not durable: {error}");
                self.storage_failed = Some(reason.clone());
                Err(fail(3, reason))
            }
        }
    }

    pub(super) fn persist(&mut self) -> Result<(), Failure> {
        let snapshot = self.snapshot(true);
        self.append("state", snapshot.clone())?;
        self.journaled_ownership = (self.owned.clone(), self.retiring.clone());
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
        let mut file = open(path).unwrap();
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

    #[test]
    fn a_torn_final_line_is_dropped_and_the_chain_continues() {
        let path = scratch("torn");
        write(&path, &[("state", object([]))]);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"kind\":\"sta").unwrap();
        let history = History::load(&path).unwrap();
        assert!(history.state.is_some());
        let mut file = open(&path).unwrap();
        append_to(&mut file, &history.last_digest, "state", &object([]), false).unwrap();
        assert!(History::load(&path).is_ok());
    }

    #[test]
    fn a_failed_append_leaves_the_synced_chain_intact() {
        let path = scratch("failed");
        write(&path, &[("state", object([]))]);
        let before = std::fs::read(&path).unwrap();
        let history = History::load(&path).unwrap();
        let mut file = open(&path).unwrap();
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
