//! Startup recovery (`docs/slice.md` 17.5, decisions D2 and D3): verify
//! every retained revision against this host, reconcile the instances and
//! effect intents the journal recorded, then resume the active or committed
//! revision once that is proven safe.
//!
//! Reconciliation. An owned or retiring instance is identified by its pid
//! and OS start time. One whose pid is absent, or held by a process with
//! another start time (a reused pid), provably ended and is never signalled.
//! One whose pid and start time match provably runs: it is this host's
//! instance, which a new host cannot adopt (its pipes ended with the old
//! host), so it is killed through a pidfd and recovery waits until it ended.
//! Any other live process whose executable lies in this state's retained
//! generations is one the journal cannot account for: a dispatched effect
//! whose `Started` was never recorded, or something foreign. It is never
//! signalled or adopted; the operation is blocked as `outcome_unknown` with
//! the processes and the unsettled intents as evidence, and a `status` on
//! the host's stdin or the next start examines the evidence again. Once no
//! such process is left the outcome is known (no instance of the intent
//! survives) and the operation continues.
//!
//! A listener port of the revision to resume that another process holds is
//! a foreign listener: it is neither signalled nor adopted, and resuming is
//! blocked as `foreign_listener` until a `status` or the next start finds
//! the port free.
//!
//! Decision D2: a revision recorded under another reducer revision than this
//! host's is not re-stepped and not driven. Its journal segments are listed
//! in the snapshot's `history` as not re-stepped; the committed (or active)
//! revision is reported as `recovery_required` with the reason
//! `reducer_revision_changed` and the next action "replan and apply", which
//! does not block an apply: the host starts, owns nothing, and the next
//! admitted plan starts its members afresh.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ignite_eval::Json;

use super::admission::retained_plan;
use super::journal::FORMAT;
use super::{Host, read_json};
use crate::apply::{Failure, fail, object, plan::Plan, required, text};
use crate::run::process::{signal_instance, start_time};

pub(super) const OUTCOME_UNKNOWN: &str = "outcome_unknown";
pub(super) const FOREIGN_LISTENER: &str = "foreign_listener";
pub(super) const REDUCER_CHANGED: &str = "reducer_revision_changed";
/// Read only by debug builds: the reducer revision this host claims, so one
/// test binary can play a host whose reducers changed (decision D2).
pub const REDUCER_OVERRIDE: &str = "IGNITE_DEBUG_HOST_REDUCER_REVISION";
/// How long recovery waits for instances to end after a crash or a kill.
const SETTLE: Duration = Duration::from_secs(3);

/// This host's reducer revision for `plan`'s retained standard library.
fn host_reducer(plan: &Plan) -> String {
    #[cfg(debug_assertions)]
    if let Some(value) = std::env::var(REDUCER_OVERRIDE)
        .ok()
        .filter(|value| !value.is_empty())
    {
        return value;
    }
    plan.host_reducer_revision()
}

/// What recovery can prove about one recorded instance.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Proof {
    /// The pid is absent, or another process (another start time) holds it.
    Ended,
    /// The pid and start time match the record.
    Runs { pid: i64, started: String },
}

pub(super) fn prove(owner: &Json) -> Proof {
    let pid = owner.get("pid").and_then(Json::as_i64);
    let instance = owner.get("instance").and_then(Json::as_str);
    match pid.zip(instance) {
        Some((pid, started)) if start_time(pid).as_deref() == Some(started) => Proof::Runs {
            pid,
            started: started.to_string(),
        },
        _ => Proof::Ended,
    }
}

/// Every live process whose executable lies under `generations`: pid,
/// executable and start time.
pub(super) fn generation_processes(generations: &Path) -> Vec<(i64, PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut found: Vec<(i64, PathBuf, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid: i64 = entry.file_name().to_str()?.parse().ok()?;
            let executable = std::fs::read_link(entry.path().join("exe")).ok()?;
            if !executable.starts_with(generations) {
                return None;
            }
            Some((pid, executable, start_time(pid)?))
        })
        .collect();
    found.sort();
    found
}

/// Poll `done` until it holds or `SETTLE` passes.
fn settle(mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + SETTLE;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The fixed listeners `plan` provisions for its own executables: member,
/// address and port.
fn listeners(plan: &Plan) -> Vec<(String, String, u16)> {
    let model = &plan.model;
    let resources = model.get("resources").and_then(Json::as_array);
    model
        .get("edges")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter(|edge| edge.get("kind").and_then(Json::as_str) == Some("provisioning"))
        .filter_map(|edge| {
            let member = edge.get("to").and_then(Json::as_str)?;
            if !plan.artifacts.contains_key(member) {
                return None;
            }
            let resource = edge.get("from").and_then(Json::as_str)?;
            let properties = resources?
                .iter()
                .find(|r| r.get("id").and_then(Json::as_str) == Some(resource))?
                .get("properties")?;
            let port = u16::try_from(properties.get("port").and_then(Json::as_i64)?).ok()?;
            let address = properties
                .get("address")
                .and_then(Json::as_str)
                .unwrap_or("127.0.0.1");
            (port > 0).then(|| (member.to_string(), address.to_string(), port))
        })
        .collect()
}

/// The D2 report of a `recovery` that a start cannot find again, without
/// the blocks kept beside it: what [`Host::recover`] starts from, so that
/// the report stays the first reason until the next commit (17.5). That is
/// a report a block was kept beside, or one naming an operation other than
/// `committed` as not driven: that operation is no longer active, and no
/// later start examines it. A report of the committed revision alone, and
/// any other recovery, is examined anew.
fn lifted_report(recovery: Json, committed: Option<&str>) -> Option<Json> {
    let Json::Object(mut fields) = recovery else {
        return None;
    };
    let blocked_beside = fields
        .get("laterReasons")
        .and_then(Json::as_array)
        .is_some_and(|later| {
            later
                .iter()
                .any(|entry| entry.get("blocking") == Some(&Json::Bool(true)))
        });
    let other_not_driven = not_driven_ids(&Json::Object(fields.clone()))
        .iter()
        .any(|id| Some(id.as_str()) != committed);
    if fields.get("reason").and_then(Json::as_str) != Some(REDUCER_CHANGED)
        || !(blocked_beside || other_not_driven)
    {
        return None;
    }
    fields.remove("laterReasons");
    fields.insert("blocking".into(), Json::Bool(false));
    Some(Json::Object(fields))
}

/// The plan ids a D2 report lists in `evidence.notDriven`.
fn not_driven_ids(report: &Json) -> Vec<String> {
    report
        .get("evidence")
        .and_then(|evidence| evidence.get("notDriven"))
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("planId").and_then(Json::as_str))
        .map(str::to_string)
        .collect()
}

fn instance_json(key: &str, owner: &Json) -> Json {
    object([
        ("key", text(key)),
        ("pid", owner.get("pid").cloned().unwrap_or(Json::Null)),
        (
            "instance",
            owner.get("instance").cloned().unwrap_or(Json::Null),
        ),
        (
            "operationId",
            owner.get("operationId").cloned().unwrap_or(Json::Null),
        ),
    ])
}

impl Host {
    /// Re-verify each retained revision's journal under its retained context
    /// and load the retained context of every other admitted operation.
    /// Revisions under another reducer revision are not re-stepped (D2);
    /// `history` lists every revision state and whether it was.
    pub(super) fn verify_history(&mut self) -> Result<(), Failure> {
        let root = self.root.clone();
        self.discard_unadmitted()?;
        let mut checked_plans = std::collections::BTreeSet::new();
        let mut history = Vec::new();
        let mut directories = Vec::new();
        if root.join("revisions").is_dir() {
            for entry in std::fs::read_dir(root.join("revisions"))? {
                let path = entry?.path();
                if path.is_dir() {
                    directories.push(path);
                }
            }
        }
        directories.sort();
        for path in directories {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            // A crash between creating a revision state and writing its
            // context leaves a directory in which nothing ran.
            if !path.join("context.json").exists() && !path.join("journal.jsonl").exists() {
                history.push(object([
                    ("revisionState", text(&name)),
                    ("planId", Json::Null),
                    ("restepped", Json::Bool(false)),
                    ("reason", text("interrupted before its run started")),
                ]));
                continue;
            }
            let metadata = read_json(&path.join("context.json"))?;
            if required(&metadata, "formatVersion")? != FORMAT
                || required(&metadata, "controllerAbi")? != crate::apply::plan::CONTROLLER_ABI
            {
                return Err(fail(3, "unsupported historical host/reducer version"));
            }
            let id = required(&metadata, "planId")?;
            checked_plans.insert(id.to_string());
            let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            let restepped = host_reducer(&plan) == plan.reducer_revision;
            if restepped {
                let context = crate::apply::revision::Context::load(retained_plan(&root, &plan)?)?;
                crate::apply::revision::verify(context, &path)?;
            }
            history.push(object([
                ("revisionState", text(&name)),
                ("planId", text(id)),
                ("reducerRevision", text(&plan.reducer_revision)),
                ("restepped", Json::Bool(restepped)),
                (
                    "reason",
                    if restepped {
                        Json::Null
                    } else {
                        text("recorded under another reducer revision than this host's")
                    },
                ),
            ]));
        }
        for (id, receipt) in &self.receipts {
            if receipt.get("noop").and_then(Json::as_bool) == Some(true)
                || checked_plans.contains(id)
            {
                continue;
            }
            let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            if host_reducer(&plan) == plan.reducer_revision {
                crate::apply::revision::Context::load(retained_plan(&root, &plan)?)?;
            }
        }
        self.history = history;
        Ok(())
    }

    /// Remove the retained context of every plan without a receipt: an
    /// admission whose acceptance never became durable (a failed sync, a
    /// crash before it), which no record refers to. Its retained files are
    /// read-only copies, so a later admission of the same plan could not
    /// retain it again.
    fn discard_unadmitted(&mut self) -> Result<(), Failure> {
        let retained = self.root.join("retained");
        if !retained.is_dir() {
            return Ok(());
        }
        let mut discarded = Vec::new();
        for entry in std::fs::read_dir(&retained)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() && !self.receipts.contains_key(&name) {
                std::fs::remove_dir_all(entry.path())?;
                discarded.push(name);
            }
        }
        if discarded.is_empty() {
            return Ok(());
        }
        std::fs::File::open(&retained)?.sync_all()?;
        discarded.sort();
        self.append(
            "discarded",
            object([
                (
                    "retained",
                    Json::Array(discarded.into_iter().map(text).collect()),
                ),
                (
                    "reason",
                    text("admission never became durable; no record refers to it"),
                ),
            ]),
        )
    }

    /// Reconcile the recorded instances and the pending effect intents,
    /// then resume the active or committed revision when that is proven
    /// safe (module doc). A block an earlier recovery set (`outcome_unknown`,
    /// `foreign_listener`) is lifted first, so its evidence is examined anew.
    pub(super) fn recover(&mut self) -> Result<(), Failure> {
        if self
            .recovery_reason
            .as_deref()
            .is_some_and(|reason| matches!(reason, OUTCOME_UNKNOWN | FOREIGN_LISTENER))
        {
            self.unblock();
        }
        // A report a block was kept beside, or one no start finds again,
        // stays the first reason recorded until the next commit; a block
        // found again goes beside it.
        let committed = self.committed.clone();
        self.recovery = self
            .recovery
            .take()
            .and_then(|recovery| lifted_report(recovery, committed.as_deref()));
        let intents: Vec<Json> = self.pending.iter().map(|intent| intent.json()).collect();
        let recorded: Vec<(String, Json)> = self
            .owned
            .iter()
            .chain(self.retiring.iter())
            .map(|(key, owner)| (key.clone(), owner.clone()))
            .collect();
        let mut ended = Vec::new();
        let mut stopped = Vec::new();
        for (key, owner) in &recorded {
            match prove(owner) {
                Proof::Ended => ended.push(instance_json(key, owner)),
                Proof::Runs { pid, started } => {
                    if signal_instance(pid, &started, nix::libc::SIGKILL)? {
                        stopped.push((pid, started.clone(), instance_json(key, owner)));
                    } else {
                        ended.push(instance_json(key, owner));
                    }
                }
            }
        }
        settle(|| {
            stopped
                .iter()
                .all(|(pid, started, _)| start_time(*pid).as_deref() != Some(started))
        });
        let generations = self.root.join("revisions");
        let mut unproven = Vec::new();
        settle(|| {
            unproven = generation_processes(&generations);
            unproven.is_empty()
        });
        if !unproven.is_empty() {
            let evidence = object([
                (
                    "processes",
                    Json::Array(
                        unproven
                            .iter()
                            .map(|(pid, executable, started)| {
                                object([
                                    ("pid", Json::Int(*pid)),
                                    ("startTime", text(started)),
                                    ("executable", text(executable.to_string_lossy())),
                                ])
                            })
                            .collect(),
                    ),
                ),
                ("intents", Json::Array(intents)),
                ("endedInstances", Json::Array(ended)),
                (
                    "stoppedInstances",
                    Json::Array(stopped.into_iter().map(|(_, _, json)| json).collect()),
                ),
            ]);
            if let Some(id) = self.active.clone().or_else(|| self.committed.clone()) {
                self.block(
                    &id,
                    OUTCOME_UNKNOWN,
                    "a process of this state's generations runs that the journal does not account for; it is neither signalled nor adopted: end it, then send `status` to the host or restart it",
                    evidence,
                )?;
            }
            return Ok(());
        }
        self.owned.clear();
        self.retiring.clear();
        let mut target = match self.recovery_reason.as_deref() {
            None => self.active.clone().or_else(|| self.committed.clone()),
            Some(_) if self.correctable() => self.committed.clone(),
            Some(_) => None,
        };
        let mut not_driven = Vec::new();
        let mut resume = None;
        while let Some(id) = target.take() {
            let value = read_json(&self.root.join("plans").join(format!("{id}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            let host = host_reducer(&plan);
            if host == plan.reducer_revision {
                resume = Some(plan);
                break;
            }
            not_driven.push(object([
                ("planId", text(&id)),
                ("reducerRevision", text(&plan.reducer_revision)),
                ("hostReducerRevision", text(&host)),
            ]));
            if self.active.as_ref() == Some(&id) && self.committed.as_ref() != Some(&id) {
                if let Some(Json::Object(receipt)) = self.receipts.get_mut(&id) {
                    receipt.insert("state".into(), text("recovery_required"));
                    receipt.insert("reason".into(), text(REDUCER_CHANGED));
                    receipt.insert("nextAction".into(), text("replan and apply"));
                }
                self.active = None;
                target = self.committed.clone();
            }
        }
        // The kept report already names what this start found again.
        let reported = self
            .recovery
            .as_ref()
            .filter(|recovery| {
                recovery.get("reason").and_then(Json::as_str) == Some(REDUCER_CHANGED)
            })
            .map(not_driven_ids)
            .unwrap_or_default();
        let found_again = not_driven.iter().all(|entry| {
            entry
                .get("planId")
                .and_then(Json::as_str)
                .is_some_and(|id| reported.iter().any(|kept| kept == id))
        });
        if let Some(first) = not_driven.first()
            && !found_again
        {
            self.note_recovery(object([
                ("state", text("recovery_required")),
                ("reason", text(REDUCER_CHANGED)),
                ("planId", first.get("planId").cloned().unwrap_or(Json::Null)),
                ("nextAction", text("replan and apply")),
                ("blocking", Json::Bool(false)),
                (
                    "evidence",
                    object([("notDriven", Json::Array(not_driven.clone()))]),
                ),
            ]));
        }
        let occupied = resume.as_ref().and_then(|plan| {
            listeners(plan).into_iter().find(|(_, address, port)| {
                std::net::TcpListener::bind((address.as_str(), *port)).is_err()
            })
        });
        let outcome = match (&resume, &occupied) {
            (Some(_), None) => "resumed",
            (Some(_), Some(_)) => FOREIGN_LISTENER,
            (None, _) if !not_driven.is_empty() => REDUCER_CHANGED,
            (None, _) => "nothing to resume",
        };
        if resume.is_some() || !intents.is_empty() || !recorded.is_empty() {
            self.append(
                "reconciled",
                object([
                    (
                        "planId",
                        resume.as_ref().map_or(Json::Null, |plan| text(&plan.id)),
                    ),
                    ("intents", Json::Array(intents)),
                    ("endedInstances", Json::Array(ended)),
                    (
                        "stoppedInstances",
                        Json::Array(stopped.into_iter().map(|(_, _, json)| json).collect()),
                    ),
                    ("outcome", text(outcome)),
                    (
                        "observation",
                        text("no instance of an earlier host survives in this state's generations"),
                    ),
                ]),
            )?;
            self.pending.clear();
        }
        match (resume, occupied) {
            (Some(plan), Some((member, address, port))) => self.block(
                &plan.id,
                FOREIGN_LISTENER,
                "another process holds a listener port of this revision; it is neither signalled nor adopted: free the port, then send `status` to the host or restart it",
                object([
                    ("member", text(member)),
                    ("address", text(address)),
                    ("port", Json::Int(i64::from(port))),
                ]),
            ),
            (Some(plan), None) => self.start(plan),
            (None, _) => Ok(()),
        }
    }

    /// `status` on stdin: a blocked recovery examines its evidence again.
    pub(super) fn recheck(&mut self) -> Result<(), Failure> {
        if self.blocked() && self.runtime.is_none() && self.transition.is_none() {
            self.recover()?;
            self.persist()?;
        }
        Ok(())
    }

    /// Block operation `id` as `recovery_required` for `reason`: no apply
    /// other than its retry is admitted and no effect is dispatched until a
    /// recovery lifts it.
    ///
    /// An unresolved recovery ([`Host::unresolved`], such as `gave_up`) is
    /// never overwritten (17.5): its receipt, the active operation and the
    /// recovery reason stay, admission refuses every apply but a retry, the
    /// block lives in `recovery` alone and names that operation as
    /// `unresolved`, and lifting it leaves the unresolved recovery as it was.
    /// Nor does a block replace a report recorded before it (D2's
    /// `reducer_revision_changed`): it goes beside the report
    /// ([`Host::note_recovery`]), which then blocks, and changes neither the
    /// active operation nor the recovery reason.
    fn block(
        &mut self,
        id: &str,
        reason: &str,
        next_action: &str,
        evidence: Json,
    ) -> Result<(), Failure> {
        let unresolved = self.active.clone().filter(|active| self.unresolved(active));
        let reported = self.recovery.is_some();
        let mut recovery = object([
            ("state", text("recovery_required")),
            ("reason", text(reason)),
            ("planId", text(id)),
            ("nextAction", text(next_action)),
            ("blocking", Json::Bool(true)),
            ("evidence", evidence.clone()),
        ]);
        if let Some(active) = &unresolved {
            if let Json::Object(fields) = &mut recovery {
                fields.insert(
                    "unresolved".into(),
                    object([
                        ("operationId", text(active)),
                        (
                            "reason",
                            self.recovery_reason.as_deref().map_or(Json::Null, text),
                        ),
                    ]),
                );
            }
            self.note_recovery(recovery);
            return self.persist();
        }
        if reported {
            self.note_recovery(recovery);
            return self.persist();
        }
        self.reject_operation(id, reason)?;
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(id)
            && receipt.get("state").and_then(Json::as_str) == Some("recovery_required")
        {
            receipt.insert("nextAction".into(), text(next_action));
            receipt.insert("evidence".into(), evidence);
        }
        self.note_recovery(recovery);
        self.persist()
    }

    /// Record `entry` as the startup recovery (17.5): the first recorded
    /// stands as `recovery`, and a later one is kept in its `laterReasons`
    /// without replacing it; a later block makes the recovery block.
    fn note_recovery(&mut self, entry: Json) {
        let Some(Json::Object(first)) = &mut self.recovery else {
            self.recovery = Some(entry);
            return;
        };
        if entry.get("blocking") == Some(&Json::Bool(true)) {
            first.insert("blocking".into(), Json::Bool(true));
        }
        match first
            .entry("laterReasons".to_string())
            .or_insert_with(|| Json::Array(Vec::new()))
        {
            Json::Array(later) => later.push(entry),
            other => *other = Json::Array(vec![entry]),
        }
    }

    /// Whether `active` carries a recovery no block may overwrite: a reason
    /// other than a block's own, on an operation whose receipt is
    /// `recovery_required` or on the committed revision itself (its
    /// controller ended; its receipt stays `completed`).
    pub(super) fn unresolved(&self, active: &str) -> bool {
        let reason = self
            .recovery_reason
            .as_deref()
            .is_some_and(|reason| !matches!(reason, OUTCOME_UNKNOWN | FOREIGN_LISTENER));
        let failed = self.committed.as_deref() == Some(active)
            || self
                .receipts
                .get(active)
                .and_then(|receipt| receipt.get("state"))
                .and_then(Json::as_str)
                == Some("recovery_required");
        reason && failed
    }

    /// Whether a startup recovery block (`outcome_unknown`,
    /// `foreign_listener`) holds: no apply is admitted while it does.
    pub(super) fn blocked(&self) -> bool {
        self.recovery
            .as_ref()
            .is_some_and(|recovery| recovery.get("blocking").and_then(Json::as_bool) == Some(true))
    }

    /// Lift the block of [`Host::block`]: the operation is accepted again,
    /// or, for the committed revision, no operation is active. A block over
    /// an unresolved recovery changed neither, so it has nothing to undo.
    fn unblock(&mut self) {
        let reason = self.recovery_reason.take();
        let Some(id) = self.active.clone() else {
            return;
        };
        if self.committed.as_ref() == Some(&id) {
            self.active = None;
        } else if let Some(Json::Object(receipt)) = self.receipts.get_mut(&id)
            && receipt.get("state").and_then(Json::as_str) == Some("recovery_required")
            && receipt.get("reason").and_then(Json::as_str) == reason.as_deref()
        {
            receipt.insert("state".into(), text("accepted"));
            for key in ["reason", "nextAction", "evidence"] {
                receipt.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Command;

    fn owner(pid: i64, instance: &str) -> Json {
        object([("pid", Json::Int(pid)), ("instance", text(instance))])
    }

    #[test]
    fn a_reused_or_absent_pid_is_ended_and_a_matching_one_runs() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let pid = i64::from(child.id());
        let started = start_time(pid).unwrap();
        assert_eq!(
            prove(&owner(pid, &started)),
            Proof::Runs {
                pid,
                started: started.clone()
            }
        );
        assert_eq!(
            prove(&owner(pid, "1")),
            Proof::Ended,
            "another start time is a reused pid"
        );
        assert_eq!(prove(&object([("pid", Json::Int(pid))])), Proof::Ended);
        assert!(!signal_instance(pid, "1", 9).unwrap());
        assert!(child.try_wait().unwrap().is_none(), "never signalled");
        child.kill().unwrap();
        assert_eq!(child.wait().unwrap().signal(), Some(9));
        assert_eq!(prove(&owner(pid, &started)), Proof::Ended);
    }

    #[test]
    fn processes_of_the_generations_are_found_by_executable() {
        let root = std::env::temp_dir().join(format!(
            "ignite-recovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bin = root.join("revisions/1-1-1/bin/1");
        std::fs::create_dir_all(&bin).unwrap();
        let sleep = bin.join("sleep");
        std::fs::copy("/bin/sleep", &sleep).unwrap();
        let generations = root.join("revisions");
        assert!(generation_processes(&generations).is_empty());
        let mut child = Command::new(&sleep).arg("30").spawn().unwrap();
        let pid = i64::from(child.id());
        let found = generation_processes(&generations);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, pid);
        assert_eq!(Some(found[0].2.clone()), start_time(pid));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(settle(|| generation_processes(&generations).is_empty()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A host on `root` with `committed` at revision 1 and `active` as the
    /// operation a machine gave up on (D3: only a corrective apply clears it).
    fn gave_up_host(root: &Path) -> Host {
        let journal = super::super::journal::open(&root.join("host-journal.jsonl")).unwrap();
        let (tx, _rx) = std::sync::mpsc::channel::<super::super::Message>();
        let receipt = |id: &str, state: &str| {
            let mut receipt = object([
                ("operationId", text(id)),
                ("planId", text(id)),
                ("state", text(state)),
                ("noop", Json::Bool(false)),
            ]);
            if state == "recovery_required"
                && let Json::Object(fields) = &mut receipt
            {
                fields.insert("reason".into(), text("gave_up"));
            }
            receipt
        };
        Host {
            root: root.to_path_buf(),
            identity: "state".into(),
            revision: 1,
            committed: Some("committed".into()),
            active: Some("failed".into()),
            recovery_reason: Some("gave_up".into()),
            receipts: [
                ("committed".to_string(), receipt("committed", "completed")),
                ("failed".to_string(), receipt("failed", "recovery_required")),
            ]
            .into_iter()
            .collect(),
            owned: Default::default(),
            retiring: Default::default(),
            standings: Default::default(),
            journal,
            last_digest: String::new(),
            runtime: None,
            transition: None,
            contexts: Default::default(),
            recovery: None,
            history: Vec::new(),
            pending: Vec::new(),
            storage_failed: None,
            journaled_ownership: Default::default(),
            preparing: false,
            waiters: Default::default(),
            tx,
        }
    }

    /// D3 and 17.5 "No unrelated operation may overwrite unresolved
    /// recovery": an operation a machine gave up on stays `recovery_required`
    /// (`gave_up`) through an `outcome_unknown` block at restart and its
    /// lifting by `status`; it is never re-admitted as `accepted` and driven
    /// again without a corrective apply.
    #[test]
    fn an_outcome_unknown_block_never_turns_a_gave_up_operation_back_into_accepted() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/adversary-u5")
            .join(format!(
                "gave-up-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let bin = root.join("revisions/2-1-1/bin/1");
        std::fs::create_dir_all(&bin).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        let sleep = root.join("revisions/2-1-1/bin/1/sleep");
        std::fs::copy("/bin/sleep", &sleep).unwrap();
        let mut unknown = Command::new(&sleep).arg("30").spawn().unwrap();
        let mut host = gave_up_host(&root);

        let blocked = host.recover();
        let after_block = host.receipts.get("failed").cloned();
        let block_reason = host
            .recovery
            .as_ref()
            .and_then(|recovery| recovery.get("reason"))
            .and_then(Json::as_str)
            .map(str::to_string);
        unknown.kill().unwrap();
        unknown.wait().unwrap();
        // `status` on stdin once the unknown process ended; the plan files
        // are absent here, so the resume after the lift fails to read one.
        let _ = host.recheck();
        let after_status = host.receipts.get("failed").cloned();
        let _ = std::fs::remove_dir_all(&root);

        assert!(blocked.is_ok(), "{blocked:?}");
        assert_eq!(
            block_reason.as_deref(),
            Some(OUTCOME_UNKNOWN),
            "the unknown generation process blocks recovery"
        );
        let state = |receipt: &Option<Json>, key: &str| {
            receipt
                .as_ref()
                .and_then(|receipt| receipt.get(key))
                .and_then(Json::as_str)
                .map(str::to_string)
        };
        assert_eq!(
            state(&after_status, "state").as_deref(),
            Some("recovery_required"),
            "a gave-up operation is cleared only by a corrective apply, \
             never resumed by lifting an outcome_unknown block: {after_status:?}"
        );
        assert_eq!(
            state(&after_block, "reason").as_deref(),
            Some("gave_up"),
            "the outcome_unknown block overwrote the unresolved gave_up: {after_block:?}"
        );
    }

    /// The same rule where the unresolved recovery is the committed
    /// revision's own (its controller ended; its receipt stays `completed`,
    /// so only the host's recovery reason records it): an `outcome_unknown`
    /// block and its lifting leave that reason and the committed revision
    /// active, and recovery does not resume a revision it was told to keep
    /// for inspection.
    #[test]
    fn an_outcome_unknown_block_keeps_an_unresolved_committed_revision() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/adversary-u5")
            .join(format!(
                "committed-ended-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(root.join("revisions/2-1-1/bin/1")).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        let sleep = root.join("revisions/2-1-1/bin/1/sleep");
        std::fs::copy("/bin/sleep", &sleep).unwrap();
        let mut unknown = Command::new(&sleep).arg("30").spawn().unwrap();
        let ended = "committed revision controller ended: Ok(1)";
        let mut host = gave_up_host(&root);
        host.receipts.remove("failed");
        host.active = Some("committed".into());
        host.recovery_reason = Some(ended.into());

        let blocked = host.recover();
        let during = (
            host.active.clone(),
            host.recovery_reason.clone(),
            host.blocked(),
        );
        unknown.kill().unwrap();
        unknown.wait().unwrap();
        let lifted = host.recheck();
        let after = (
            host.active.clone(),
            host.recovery_reason.clone(),
            host.blocked(),
        );
        let _ = std::fs::remove_dir_all(&root);

        assert!(blocked.is_ok(), "{blocked:?}");
        assert_eq!(
            during,
            (Some("committed".into()), Some(ended.into()), true),
            "the block keeps the unresolved reason"
        );
        assert!(lifted.is_ok(), "nothing is resumed: {lifted:?}");
        assert_eq!(
            after,
            (Some("committed".into()), Some(ended.into()), false),
            "lifting the block restores the unresolved recovery"
        );
    }

    /// The same rule for the other block: a gave-up operation whose committed
    /// revision cannot resume because another process holds its listener port
    /// (`recover` blocks the committed plan with `foreign_listener`) keeps the
    /// failed operation active and `gave_up`, through the journaled snapshot
    /// a restart restores, and the block admits no apply while it holds.
    #[test]
    fn a_foreign_listener_block_of_the_committed_revision_keeps_the_gave_up_operation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/adversary-u5")
            .join(format!(
                "foreign-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let mut host = gave_up_host(&root);
        let blocked = host.block(
            "committed",
            FOREIGN_LISTENER,
            "free the port",
            object([("port", Json::Int(1))]),
        );
        let snapshot = host.snapshot(true);
        let mut restarted = gave_up_host(&root);
        restarted.active = None;
        restarted.recovery_reason = None;
        let restored = restarted.restore(&snapshot);
        let _ = std::fs::remove_dir_all(&root);

        assert!(blocked.is_ok(), "{blocked:?}");
        assert!(restored.is_ok(), "{restored:?}");
        for host in [&host, &restarted] {
            assert_eq!(
                host.active.as_deref(),
                Some("failed"),
                "the gave-up operation stays active"
            );
            assert_eq!(host.recovery_reason.as_deref(), Some("gave_up"));
            let failed = host.receipts.get("failed");
            assert_eq!(
                failed
                    .and_then(|receipt| receipt.get("reason"))
                    .and_then(Json::as_str),
                Some("gave_up"),
                "{failed:?}"
            );
            let committed = host.receipts.get("committed");
            assert_eq!(
                committed
                    .and_then(|receipt| receipt.get("state"))
                    .and_then(Json::as_str),
                Some("completed"),
                "{committed:?}"
            );
            assert!(
                host.blocked(),
                "the foreign listener blocks: {:?}",
                host.recovery
            );
            let unresolved = host
                .recovery
                .as_ref()
                .and_then(|recovery| recovery.get("unresolved"))
                .and_then(|unresolved| unresolved.get("operationId"))
                .and_then(Json::as_str);
            assert_eq!(unresolved, Some("failed"));
        }
    }

    /// A fresh directory under the workspace target for one host.
    fn scratch(label: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/adversary-u5")
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// The committed revision's run, its records attributed to `operation`.
    fn committed_runtime(operation: &str) -> super::super::Runtime {
        use std::sync::{Arc, Mutex};
        super::super::Runtime {
            operation: operation.into(),
            controller_plan: "committed".into(),
            desired: "committed".into(),
            model: Json::Null,
            reducer: String::new(),
            operation_sink: Arc::new(Mutex::new(operation.into())),
            artifacts: Arc::new(Mutex::new(Default::default())),
            minimum_generation: Default::default(),
            control: std::sync::mpsc::channel().0,
            ready: Default::default(),
            expected: Default::default(),
            adopted: Default::default(),
            supervised: Default::default(),
            restore_to: None,
        }
    }

    /// The state, reason and next action of `id`'s receipt.
    fn receipt_of(host: &Host, id: &str) -> (Option<String>, Option<String>, Option<String>) {
        let field = |key: &str| {
            host.receipts
                .get(id)
                .and_then(|receipt| receipt.get(key))
                .and_then(Json::as_str)
                .map(str::to_string)
        };
        (field("state"), field("reason"), field("nextAction"))
    }

    /// The operation and reason of each later reason `id`'s receipt keeps.
    fn later_of(host: &Host, id: &str) -> Vec<(String, String)> {
        host.receipts
            .get(id)
            .and_then(|receipt| receipt.get("laterReasons"))
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|later| {
                let field = |key: &str| {
                    later
                        .get(key)
                        .and_then(Json::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                (field("operationId"), field("reason"))
            })
            .collect()
    }

    const INSPECT: &str = "inspect retained ownership and journal before recovery";

    /// 17.5, the precedence of recovery reasons: a reducer failure of the
    /// committed revision's run while a `gave_up` is unresolved, whether the
    /// run's records carry the committed operation or still the failed one,
    /// leaves the failed operation active with `gave_up` as its reason and
    /// keeps the failure beside it in that receipt. A corrective plan does
    /// not resolve the failure, so the next action is to inspect, and the
    /// run dispatches no further effect.
    #[test]
    fn a_reducer_failure_of_the_committed_run_keeps_the_gave_up_operation() {
        for sink in ["committed", "failed"] {
            let root = scratch("reducer-failed");
            let mut host = gave_up_host(&root);
            host.runtime = Some(committed_runtime(sink));
            let error = object([
                ("kind", text("error")),
                ("machine", text("Terminal.probe.scheduler")),
                (
                    "value",
                    object([
                        ("code", text("E128")),
                        ("report", text("reducer of Terminal.probe.scheduler failed")),
                    ]),
                ),
            ]);
            let failed = host.record(sink, "committed", error);
            let effect = host.record(
                sink,
                "committed",
                object([
                    ("kind", text("effect")),
                    ("machine", text("Terminal.server.supervisor")),
                    ("value", object([("variant", text("Spawn"))])),
                ]),
            );
            let _ = std::fs::remove_dir_all(&root);

            assert!(failed.is_ok(), "{sink}: {failed:?}");
            assert_eq!(host.active.as_deref(), Some("failed"), "{sink}");
            assert_eq!(host.recovery_reason.as_deref(), Some("gave_up"), "{sink}");
            assert_eq!(
                receipt_of(&host, "failed"),
                (
                    Some("recovery_required".into()),
                    Some("gave_up".into()),
                    Some(INSPECT.into())
                ),
                "{sink}"
            );
            assert_eq!(
                later_of(&host, "failed"),
                vec![(
                    sink.to_string(),
                    "revision reducer failed; inspect retained journal".to_string()
                )],
                "{sink}"
            );
            assert_eq!(
                receipt_of(&host, "committed").0.as_deref(),
                Some("completed"),
                "{sink}"
            );
            assert!(
                effect.is_err(),
                "{sink}: the reducer failure still blocks every effect"
            );
        }
    }

    /// `finished`: the committed revision's controller that ends while a
    /// `gave_up` is unresolved leaves that operation active with its reason
    /// and keeps the end beside it; nothing resumes the failed operation.
    #[test]
    fn an_ended_committed_controller_is_kept_beside_the_gave_up_operation() {
        let root = scratch("controller-ended");
        let mut host = gave_up_host(&root);
        host.runtime = Some(committed_runtime("committed"));
        let ended = host.finished("committed", Ok(1), false);
        let _ = std::fs::remove_dir_all(&root);

        assert!(ended.is_ok(), "{ended:?}");
        assert!(host.runtime.is_none());
        assert_eq!(host.active.as_deref(), Some("failed"));
        assert_eq!(host.recovery_reason.as_deref(), Some("gave_up"));
        assert_eq!(
            receipt_of(&host, "failed"),
            (
                Some("recovery_required".into()),
                Some("gave_up".into()),
                Some(INSPECT.into())
            )
        );
        assert_eq!(
            later_of(&host, "failed"),
            vec![(
                "committed".to_string(),
                "committed revision controller ended: Ok(1)".to_string()
            )]
        );
    }

    /// 17.5 lists "a process that failed to start" among the later failures
    /// kept beside an unresolved `gave_up`, "whichever operation the run's
    /// records carry". A `SpawnFailed` of the committed revision's run whose
    /// records carry the committed operation (the run after an abandoned
    /// transition gave it back, `release_operation`) is a reason a corrective
    /// does not resolve: it must be kept beside `gave_up`, turn the next
    /// action into an inspection and block further effects.
    #[test]
    fn adversary_a_spawn_failure_of_the_committed_run_is_kept_beside_the_gave_up_operation() {
        let root = scratch("spawn-failed");
        let mut host = gave_up_host(&root);
        let mut runtime = committed_runtime("committed");
        runtime
            .supervised
            .insert("Terminal.server.supervisor".to_string());
        host.runtime = Some(runtime);
        let failed = host.record(
            "committed",
            "committed",
            object([
                ("kind", text("event")),
                ("machine", text("Terminal.server.supervisor")),
                (
                    "value",
                    object([(
                        "fields",
                        object([(
                            "kind",
                            object([
                                ("variant", text("SpawnFailed")),
                                ("fields", Json::Array(vec![text("no such file")])),
                            ]),
                        )]),
                    )]),
                ),
            ]),
        );
        let effect = host.record(
            "committed",
            "committed",
            object([
                ("kind", text("effect")),
                ("machine", text("Terminal.server.supervisor")),
                ("value", object([("variant", text("Spawn"))])),
            ]),
        );
        let _ = std::fs::remove_dir_all(&root);

        assert!(failed.is_ok(), "{failed:?}");
        assert_eq!(host.active.as_deref(), Some("failed"));
        assert_eq!(host.recovery_reason.as_deref(), Some("gave_up"));
        assert_eq!(
            later_of(&host, "failed"),
            vec![(
                "committed".to_string(),
                "a required owned process failed to start".to_string()
            )],
            "the spawn failure is kept beside gave_up"
        );
        assert_eq!(
            receipt_of(&host, "failed").2.as_deref(),
            Some(INSPECT),
            "a corrective does not resolve a failed start"
        );
        assert!(
            effect.is_err(),
            "the unresolvable later reason blocks every effect"
        );
    }

    /// 17.5: a later reason a corrective does not resolve means "no
    /// corrective is admitted". The live inspection's top-level next action
    /// must then not tell the operator to prepare and apply one.
    #[test]
    fn adversary_inspect_after_a_later_reducer_failure_does_not_ask_for_a_corrective() {
        let root = scratch("inspect-next");
        let mut host = gave_up_host(&root);
        host.runtime = Some(committed_runtime("committed"));
        let failed = host.record(
            "committed",
            "committed",
            object([
                ("kind", text("error")),
                ("machine", text("Terminal.probe.scheduler")),
                (
                    "value",
                    object([
                        ("code", text("E128")),
                        ("report", text("reducer of Terminal.probe.scheduler failed")),
                    ]),
                ),
            ]),
        );
        let view = host.inspection();
        let _ = std::fs::remove_dir_all(&root);

        assert!(failed.is_ok(), "{failed:?}");
        assert!(!host.correctable(), "admission refuses a corrective");
        let next = view
            .get("nextAction")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(
            !next.contains("corrective"),
            "inspect asks for a corrective that admission refuses: {next}"
        );
    }

    /// 17.5's last later failure, a ready artifact that differs: kept
    /// beside an unresolved `gave_up` under the operation the run's records
    /// carry, as the reducer failure and the failed start are, and blocking
    /// as a reason a corrective does not resolve.
    #[test]
    fn a_differing_ready_artifact_of_the_committed_run_is_kept_under_its_operation() {
        let root = scratch("artifact");
        std::fs::create_dir_all(root.join("plans")).unwrap();
        std::fs::write(
            root.join("plans/committed.json"),
            object([(
                "artifacts",
                object([(
                    "Terminal.server",
                    object([("digest", text("sha256:another"))]),
                )]),
            )])
            .compact(),
        )
        .unwrap();
        let pid = i64::from(std::process::id());
        let mut host = gave_up_host(&root);
        host.owned.insert(
            "Terminal.server".into(),
            object([
                ("pid", Json::Int(pid)),
                ("ready", Json::Bool(false)),
                ("operationId", text("committed")),
                (
                    "instance",
                    super::super::controller::process_identity(pid).map_or(Json::Null, text),
                ),
                ("generation", Json::Int(1)),
            ]),
        );
        let mut runtime = committed_runtime("committed");
        runtime
            .supervised
            .insert("Terminal.server.supervisor".to_string());
        host.runtime = Some(runtime);
        let ready = host.record(
            "committed",
            "committed",
            object([
                ("kind", text("event")),
                ("machine", text("Terminal.server.supervisor")),
                (
                    "value",
                    object([(
                        "fields",
                        object([(
                            "kind",
                            object([
                                ("variant", text("Ready")),
                                ("fields", Json::Array(vec![Json::Int(pid)])),
                            ]),
                        )]),
                    )]),
                ),
            ]),
        );
        let _ = std::fs::remove_dir_all(&root);

        assert!(ready.is_err(), "the mismatch fails the record: {ready:?}");
        assert_eq!(host.active.as_deref(), Some("failed"));
        assert_eq!(host.recovery_reason.as_deref(), Some("gave_up"));
        assert_eq!(
            later_of(&host, "failed"),
            vec![(
                "committed".to_string(),
                "ready process artifact differs from the admitted desired artifact".to_string()
            )]
        );
        assert_eq!(receipt_of(&host, "failed").2.as_deref(), Some(INSPECT));
        assert!(!host.correctable());
    }
}
