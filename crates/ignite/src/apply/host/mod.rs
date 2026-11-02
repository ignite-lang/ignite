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
use controller::Runtime;

type Reply = mpsc::Sender<Result<Json, Failure>>;

enum Message {
    Request(Json, Reply),
    Prepared(Box<Result<Plan, Failure>>, Reply),
    Record(String, String, Json, mpsc::Sender<io::Result<()>>),
    Finished(String, Result<i32, String>),
    Initialized(String),
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
    journal: File,
    last_digest: String,
    runtime: Option<Runtime>,
    contexts: BTreeMap<String, super::revision::Context>,
    preparing: bool,
    waiters: BTreeMap<String, Vec<Reply>>,
    tx: mpsc::Sender<Message>,
}

fn read_json(path: &Path) -> Result<Json, Failure> {
    Json::parse(&std::fs::read_to_string(path)?)
        .map_err(|reason| fail(3, format!("{}: {reason}", path.display())))
}

/// Run one explicit host. Only this queue writes its journal and acknowledges
/// effects; socket clients never dispatch process or journal paths.
pub fn run(state: &Path) -> Result<i32, Failure> {
    std::fs::create_dir_all(state)?;
    let root = std::fs::canonicalize(state)?;
    let _lock = transport::lock(&root)?;
    let identity = transport::identity(&root)?;
    std::fs::create_dir_all(root.join("plans"))?;
    let journal_path = root.join("host-journal.jsonl");
    let history = journal::History::load(&journal_path)?;
    let (tx, rx) = mpsc::channel();
    let journal = journal::open(&journal_path)?;
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
        contexts: BTreeMap::new(),
        preparing: false,
        waiters: BTreeMap::new(),
        tx: tx.clone(),
    };
    if let Some(previous) = history.state {
        host.restore(&previous)?;
    }
    let (socket_path, stopping) = transport::listen(&root, tx.clone())?;
    transport::watch_control(tx.clone())?;
    host.verify_history()?;
    host.resume(&history.intents)?;
    host.persist()?;
    eprintln!("ignite host: {} revision {}", host.identity, host.revision);
    let mut shutdown = false;
    while let Ok(message) = rx.recv() {
        match message {
            Message::Request(request, reply) => {
                if let Err(error) = host.request(&request, &reply, shutdown) {
                    let _ = reply.send(Err(error));
                }
            }
            Message::Prepared(result, reply) => {
                let _ = reply.send(host.prepared(*result));
            }
            Message::Record(id, controller, record, ack) => {
                let result = host.record(&id, &controller, record);
                let failed = result.as_ref().is_err_and(|error| error.status == 3);
                let _ = ack.send(result.map_err(|error| io::Error::other(error.to_string())));
                if failed {
                    stopping.store(true, Ordering::Relaxed);
                    return Err(fail(3, "host durable record failed; mutation stopped"));
                }
            }
            Message::Finished(id, result) => {
                host.finished(&id, result, shutdown)?;
                if shutdown {
                    break;
                }
            }
            Message::Initialized(id) => host.initialized(&id)?,
            Message::Status => println!("{}", host.snapshot(true).compact()),
            Message::Stop => {
                shutdown = true;
                if let Some(runtime) = &host.runtime {
                    let _ = runtime.control.send(Observation::Control("stop".into()));
                } else {
                    break;
                }
            }
        }
    }
    stopping.store(true, Ordering::Relaxed);
    host.persist()?;
    std::fs::remove_file(socket_path)?;
    Ok(0)
}
