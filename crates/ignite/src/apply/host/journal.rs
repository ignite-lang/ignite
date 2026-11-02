//! The host journal: hash-chained records in `host-journal.jsonl`, the
//! state snapshot they carry and its `status.json` copy.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use ignite_eval::Json;

use super::Host;
use crate::apply::{Failure, fail, object, required, text};

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

pub(super) fn save_json(path: &Path, value: &Json) -> Result<(), Failure> {
    let temporary = path.with_extension(format!("{}.writing", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all((value.compact() + "\n").as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)?;
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

/// What a verified journal says about the host before this start.
pub(super) struct History {
    /// The last `state` record's snapshot.
    pub(super) state: Option<Json>,
    /// The digest the next record chains to.
    pub(super) last_digest: String,
    /// Operations with at least one journaled effect.
    pub(super) intents: BTreeSet<String>,
}

impl History {
    /// Verify the digest chain of the journal at `path`, dropping a torn
    /// final line.
    pub(super) fn load(path: &Path) -> Result<History, Failure> {
        let mut history = History {
            state: None,
            last_digest: String::new(),
            intents: BTreeSet::new(),
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
        file.set_len(complete as u64)?;
        let content =
            std::str::from_utf8(&bytes[..complete]).map_err(|error| fail(3, error.to_string()))?;
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
            if record.get("kind").and_then(Json::as_str) == Some("revisionRecord")
                && let Some(value) = record.get("value")
                && value
                    .get("record")
                    .and_then(|record| record.get("kind"))
                    .and_then(Json::as_str)
                    == Some("effect")
                && let Some(id) = value.get("operationId").and_then(Json::as_str)
            {
                history.intents.insert(id.to_string());
            }
            if record.get("kind").and_then(Json::as_str) == Some("state") {
                history.state = record.get("value").cloned();
            }
        }
        Ok(history)
    }
}

/// Open the journal for appending.
pub(super) fn open(path: &Path) -> Result<File, Failure> {
    Ok(OpenOptions::new().create(true).append(true).open(path)?)
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
                self.recovery_reason.as_ref().map_or(Json::Null, text),
            ),
            ("operations", Json::Object(self.receipts.clone())),
            ("ownership", Json::Object(self.owned.clone())),
            ("retiringInstances", Json::Object(self.retiring.clone())),
            ("lastStandings", Json::Object(self.standings.clone())),
        ])
    }

    /// Take the fields of a journaled snapshot; the inverse of `snapshot`.
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
            .map(str::to_string);
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

    pub(super) fn append(&mut self, kind: &str, value: Json) -> Result<(), Failure> {
        let mut record = object([
            ("kind", text(kind)),
            ("value", value),
            ("previousDigest", text(&self.last_digest)),
        ]);
        let digest =
            crate::apply::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
        if let Json::Object(fields) = &mut record {
            fields.insert("digest".into(), text(&digest));
        }
        self.journal
            .write_all((record.compact() + "\n").as_bytes())?;
        self.journal.sync_data()?;
        self.last_digest = digest;
        Ok(())
    }

    pub(super) fn persist(&mut self) -> Result<(), Failure> {
        let snapshot = self.snapshot(true);
        self.append("state", snapshot.clone())?;
        save_json(&self.root.join("status.json"), &snapshot)
    }
}
