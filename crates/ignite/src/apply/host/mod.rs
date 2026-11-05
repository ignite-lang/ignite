//! The foreground local host: one queue owns the host journal, admits plans
//! and drives revisions. `transport` carries the control socket, the state
//! lock and the state identity; `admission` prepares and admits saved plans;
//! `controller` drives admitted revisions through the run driver; `journal`
//! persists the hash-chained host history; `recovery` replays that history at
//! startup.

mod admission;
mod controller;
mod journal;
mod recovery;
mod transport;

use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{atomic::Ordering, mpsc};

use ignite_eval::Json;

use super::{Failure, fail, plan::Plan};
use crate::run::control::Observation;
use controller::{Driving, Runtime};

type Reply = mpsc::Sender<Result<Json, Failure>>;

enum Message {
    Request(Json, Reply),
    Prepared(Box<Result<Plan, Failure>>, Reply),
    Record(
        String,
        String,
        Json,
        mpsc::Sender<io::Result<crate::run::live::Admission>>,
    ),
    Finished(String, Result<i32, String>),
    Initialized(String),
    Handover(String, Box<crate::run::live::Carried>),
    Status,
    Stop,
}

struct Host {
    root: PathBuf,
    identity: String,
    revision: i64,
    committed: Option<String>,
    active: Option<String>,
    recovery_reason: Option<String>,
    receipts: BTreeMap<String, Json>,
    owned: BTreeMap<String, Json>,
    retiring: BTreeMap<String, Json>,
    standings: BTreeMap<String, Json>,
    journal: journal::Writer,
    last_digest: String,
    runtime: Option<Runtime>,
    transition: Option<Driving>,
    contexts: BTreeMap<String, super::revision::Context>,
    /// The startup recovery (`recovery`): its reason, evidence and next
    /// action, and whether it blocks the operation it names.
    recovery: Option<Json>,
    /// Every retained revision state and whether it was re-stepped (D2).
    history: Vec<Json>,
    /// Effect intents without a recorded completion, until reconciled.
    pending: Vec<journal::Intent>,
    /// Set by a failed journal write or sync; the queue stops the host.
    storage_failed: Option<String>,
    /// The owned and retiring instances as last journaled.
    journaled_ownership: (BTreeMap<String, Json>, BTreeMap<String, Json>),
    preparing: bool,
    waiters: BTreeMap<String, Vec<Reply>>,
    tx: mpsc::Sender<Message>,
}

fn read_json(path: &Path) -> Result<Json, Failure> {
    Json::parse(&std::fs::read_to_string(path)?)
        .map_err(|reason| fail(3, format!("{}: {reason}", path.display())))
}

/// The file in a state that names the working directory of its host.
const WORKDIR: &str = "workdir";

/// The canonical working directory of this process, refused with exit 2
/// when the state records another (`docs/slice.md` 22.4, D7): a relative
/// `Directory` of a hosted program resolves against it, so a host started
/// elsewhere would open other data.
fn hosted_from(root: &Path) -> Result<PathBuf, Failure> {
    use std::os::unix::ffi::OsStringExt;
    let here = std::fs::canonicalize(std::env::current_dir()?)?;
    let path = root.join(WORKDIR);
    match std::fs::read(&path) {
        Ok(mut bytes) => {
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
            }
            let recorded = PathBuf::from(std::ffi::OsString::from_vec(bytes));
            if recorded != here {
                return Err(fail(
                    2,
                    format!(
                        "state {} is hosted from {}; start ignite host there",
                        root.display(),
                        recorded.display()
                    ),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(fail(3, format!("{}: {error}", path.display()))),
    }
    Ok(here)
}

/// Record `here` as the state's working directory unless it is recorded
/// already (a state written before slice 18 gets one at its next start).
fn record_workdir(root: &Path, here: &Path) -> Result<(), Failure> {
    use std::os::unix::ffi::OsStrExt;
    let path = root.join(WORKDIR);
    if path.exists() {
        return Ok(());
    }
    let mut bytes = here.as_os_str().as_bytes().to_vec();
    bytes.push(b'\n');
    journal::save_bytes(&path, &bytes)
}

/// The integrity of `state`'s journals, read only, as offline `inspect`
/// reports it (22.7): `{"intact": true}`, with `pendingRepair` when only a
/// torn final line waits for the next host, or the damage.
pub(super) fn integrity(state: &Path) -> Result<Json, Failure> {
    let history = match journal::History::scan(&state.join("host-journal.jsonl"))? {
        Ok(history) => history,
        Err(damage) => return Ok(damage.json()),
    };
    if let Some(damage) = journal::check_revisions(state, &history)? {
        return Ok(damage.json());
    }
    let mut fields = BTreeMap::from([("intact".to_string(), Json::Bool(true))]);
    if !history.torn.is_empty() {
        fields.insert(
            "pendingRepair".into(),
            Json::Object(BTreeMap::from([(
                "droppedBytes".to_string(),
                Json::Int(history.torn.len() as i64),
            )])),
        );
    }
    Ok(Json::Object(fields))
}

/// Run one explicit host. Only this queue writes its journal and acknowledges
/// effects; socket clients never dispatch process or journal paths. Nothing
/// is written before the working directory and the journals are verified
/// (22.7): a damaged journal is exit 3, and the host neither truncates nor
/// rewrites it (D10).
pub fn run(state: &Path) -> Result<i32, Failure> {
    std::fs::create_dir_all(state)?;
    let root = std::fs::canonicalize(state)?;
    let _lock = transport::lock(&root)?;
    let workdir = hosted_from(&root)?;
    let identity = transport::identity(&root)?;
    std::fs::create_dir_all(root.join("plans"))?;
    File::open(&root)?.sync_all()?;
    let journal_path = root.join("host-journal.jsonl");
    let mut history = journal::History::load(&journal_path)?;
    if let Some(damage) = journal::check_revisions(&root, &history)? {
        return Err(fail(3, damage.message()));
    }
    let (tx, rx) = mpsc::channel();
    let mut journal = journal::open(&journal_path)?;
    if let Some(repair) = journal.repair(&mut history)? {
        eprintln!(
            "ignite host: dropped a torn final journal record of {} bytes",
            repair
                .get("droppedBytes")
                .and_then(Json::as_i64)
                .unwrap_or(0)
        );
    }
    record_workdir(&root, &workdir)?;
    let mut host = Host {
        root: root.clone(),
        identity,
        revision: 0,
        committed: None,
        active: None,
        recovery_reason: None,
        receipts: BTreeMap::new(),
        owned: BTreeMap::new(),
        retiring: BTreeMap::new(),
        standings: BTreeMap::new(),
        journal,
        last_digest: history.last_digest,
        runtime: None,
        transition: None,
        contexts: BTreeMap::new(),
        recovery: None,
        history: Vec::new(),
        pending: history.pending,
        storage_failed: None,
        journaled_ownership: (BTreeMap::new(), BTreeMap::new()),
        preparing: false,
        waiters: BTreeMap::new(),
        tx: tx.clone(),
    };
    if let Some(previous) = history.state {
        host.restore(&previous)?;
    }
    host.journaled_ownership = (host.owned.clone(), host.retiring.clone());
    if let Some((owned, retiring)) = &history.ownership {
        host.restore_ownership(owned, retiring);
    }
    let (socket_path, stopping) = transport::listen(&root, tx.clone())?;
    transport::watch_control(tx.clone())?;
    host.verify_history()?;
    host.recover()?;
    host.persist()?;
    eprintln!("ignite host: {} revision {}", host.identity, host.revision);
    let mut shutdown = false;
    while let Ok(message) = rx.recv() {
        let revision = host.revision;
        match message {
            Message::Request(request, reply) => {
                if let Err(error) = host.request(&request, &reply, shutdown) {
                    let _ = reply.send(Err(error));
                }
            }
            Message::Prepared(result, reply) => {
                let _ = reply.send(host.prepared(*result));
            }
            // The ownership a record changed is journaled before the ack
            // lets the run driver dispatch its next effect.
            Message::Record(id, controller, record, ack) => {
                let result = host
                    .record(&id, &controller, record)
                    .and_then(|()| host.journal_ownership());
                let failed = result
                    .as_ref()
                    .err()
                    .filter(|error| error.status == 3)
                    .map(|error| error.reason.clone());
                let _ = ack.send(controller::admission(result));
                if let Some(reason) = failed {
                    stopping.store(true, Ordering::Relaxed);
                    return Err(fail(
                        3,
                        format!("host durable record failed: {reason}; mutation stopped"),
                    ));
                }
            }
            Message::Finished(id, result) => {
                host.finished(&id, result, shutdown)?;
                if shutdown {
                    break;
                }
            }
            Message::Initialized(id) => host.initialized(&id)?,
            Message::Handover(id, carried) => host.handed_over(&id, *carried)?,
            Message::Status => {
                host.recheck()?;
                println!("{}", host.snapshot(true).compact());
            }
            Message::Stop => {
                shutdown = true;
                if let Some(runtime) = &host.runtime {
                    let _ = runtime.control.send(Observation::Control("stop".into()));
                } else {
                    break;
                }
            }
        }
        // A commit settles a recovery that only reported, such as a
        // committed revision under another reducer revision (D2).
        if host.revision != revision
            && host
                .recovery
                .as_ref()
                .is_some_and(|recovery| recovery.get("blocking") == Some(&Json::Bool(false)))
        {
            host.recovery = None;
            host.persist()?;
        }
        if host.storage_failed.is_none() {
            let _ = host.journal_ownership();
        }
        // Losing durable storage stops new mutations (17.5): the host ends
        // here, after the failed request's reply, and its children end by
        // the parent-death rule.
        if let Some(reason) = &host.storage_failed {
            stopping.store(true, Ordering::Relaxed);
            return Err(fail(
                3,
                format!("{reason}; host stopped before any further mutation"),
            ));
        }
    }
    stopping.store(true, Ordering::Relaxed);
    host.persist()?;
    std::fs::remove_file(socket_path)?;
    Ok(0)
}
