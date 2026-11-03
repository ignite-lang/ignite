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
use super::controller::resolvable;
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
        self.recovery = None;
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
            Some(reason) if resolvable(reason) => self.committed.clone(),
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
        if let Some(first) = not_driven.first() {
            self.recovery = Some(object([
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
        let blocked = self
            .recovery
            .as_ref()
            .is_some_and(|recovery| recovery.get("blocking").and_then(Json::as_bool) == Some(true));
        if blocked && self.runtime.is_none() && self.transition.is_none() {
            self.recover()?;
            self.persist()?;
        }
        Ok(())
    }

    /// Block operation `id` as `recovery_required` for `reason`: no apply
    /// other than its retry is admitted and no effect is dispatched until a
    /// recovery lifts it.
    fn block(
        &mut self,
        id: &str,
        reason: &str,
        next_action: &str,
        evidence: Json,
    ) -> Result<(), Failure> {
        self.reject_operation(id, reason)?;
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(id)
            && receipt.get("state").and_then(Json::as_str) == Some("recovery_required")
        {
            receipt.insert("nextAction".into(), text(next_action));
            receipt.insert("evidence".into(), evidence.clone());
        }
        self.recovery = Some(object([
            ("state", text("recovery_required")),
            ("reason", text(reason)),
            ("planId", text(id)),
            ("nextAction", text(next_action)),
            ("blocking", Json::Bool(true)),
            ("evidence", evidence),
        ]));
        self.persist()
    }

    /// Lift the block of [`Host::block`]: the operation is accepted again,
    /// or, for the committed revision, no operation is active.
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
}
