use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use ignite_eval::Json;
use nix::fcntl::{Flock, FlockArg};

use super::{Failure, PROTOCOL, fail, object, plan::Plan, read_frame, required, text, write_frame};
use crate::run::control::Observation;

const FORMAT: &str = "ignite-host-journal/0.1";
type Reply = mpsc::Sender<Result<Json, Failure>>;

enum Message {
    Request(Json, Reply),
    Prepared(Box<Result<Plan, Failure>>, Reply),
    Record(String, Json, mpsc::Sender<io::Result<()>>),
    Finished(String, Result<i32, String>),
    Initialized(String),
    Status,
    Stop,
}

struct Runtime {
    operation: String,
    control: mpsc::Sender<Observation>,
    ready: BTreeSet<String>,
    expected: BTreeSet<String>,
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

fn process_identity(pid: i64) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat.rsplit_once(") ")?.1;
    if matches!(fields.split_whitespace().next(), Some("Z" | "X")) {
        return None;
    }
    Some(fields.split_whitespace().nth(19)?.to_string())
}

fn sync_directories(root: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            sync_directories(&entry.path())?;
        }
    }
    File::open(root)?.sync_all()
}

fn generation_process_exists(root: &Path) -> Result<bool, Failure> {
    let generations = root.join("revisions");
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        if let Ok(executable) = std::fs::read_link(entry.path().join("exe"))
            && executable.starts_with(&generations)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn save_json(path: &Path, value: &Json) -> Result<(), Failure> {
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

/// Preserve the admitted source graph independently of its workspace spelling.
/// Only these frozen modules and pinned artifacts may enter the live worker.
fn retain_context(root: &Path, plan: &Plan) -> Result<(), Failure> {
    let generation = root.join("retained").join(&plan.id);
    std::fs::create_dir_all(&generation)?;
    let source_root = plan
        .source
        .parent()
        .ok_or_else(|| fail(2, "source has no directory"))?;
    let std_root = std::fs::canonicalize(plan.std.clone().unwrap_or_else(crate::default_std_path))?;
    for input in &plan.inputs {
        if input.path.extension().and_then(|value| value.to_str()) != Some("ignite") {
            continue;
        }
        let destination = if let Ok(relative) = input.path.strip_prefix(&std_root) {
            generation.join("std").join(relative)
        } else if let Ok(relative) = input.path.strip_prefix(source_root) {
            generation.join("source").join(relative)
        } else {
            return Err(fail(
                2,
                format!(
                    "source input is outside the retained graph: {}",
                    input.path.display()
                ),
            ));
        };
        std::fs::create_dir_all(destination.parent().unwrap())?;
        std::fs::copy(&input.path, &destination)?;
        if super::digest::file(&destination)? != input.digest {
            return Err(fail(2, "source changed while retaining admitted context"));
        }
        File::open(&destination)?.sync_all()?;
    }
    for artifact in plan.artifacts.values() {
        if Path::new(&artifact.name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(&artifact.name)
        {
            return Err(fail(2, "invalid artifact name"));
        }
        let destination = generation
            .join("artifacts")
            .join(&artifact.digest)
            .join(&artifact.name);
        std::fs::create_dir_all(destination.parent().unwrap())?;
        std::fs::copy(&artifact.path, &destination)?;
        if super::digest::file(&destination)? != artifact.digest {
            return Err(fail(2, "artifact changed while retaining admitted context"));
        }
        File::open(&destination)?.sync_all()?;
    }
    sync_directories(&generation)?;
    File::open(generation.parent().unwrap())?.sync_all()?;
    Ok(())
}

fn retained_plan(root: &Path, plan: &Plan) -> Result<Plan, Failure> {
    let executable = std::fs::canonicalize(std::env::current_exe()?)?;
    let compiler = plan
        .inputs
        .iter()
        .find(|input| input.path == executable)
        .ok_or_else(|| {
            fail(
                3,
                "admitted compiler identity unavailable at this host path",
            )
        })?;
    if super::digest::file(&executable)? != compiler.digest {
        return Err(fail(
            3,
            "unsupported host compiler change; admitted controller bytes differ",
        ));
    }
    let generation = root.join("retained").join(&plan.id);
    let mut frozen = plan.clone();
    frozen.source = generation.join("source").join(
        plan.source
            .file_name()
            .ok_or_else(|| fail(3, "retained source name missing"))?,
    );
    frozen.std = Some(generation.join("std"));
    for artifact in frozen.artifacts.values_mut() {
        artifact.path = generation
            .join("artifacts")
            .join(&artifact.digest)
            .join(&artifact.name);
        if super::digest::file(&artifact.path)? != artifact.digest {
            return Err(fail(3, "retained artifact digest mismatch"));
        }
    }
    let source_root = plan
        .source
        .parent()
        .ok_or_else(|| fail(3, "retained source parent missing"))?;
    let std_root = plan.std.clone().unwrap_or_else(crate::default_std_path);
    for input in &plan.inputs {
        if input.path.extension().and_then(|value| value.to_str()) != Some("ignite") {
            continue;
        }
        let path = if let Ok(relative) = input.path.strip_prefix(&std_root) {
            generation.join("std").join(relative)
        } else if let Ok(relative) = input.path.strip_prefix(source_root) {
            generation.join("source").join(relative)
        } else {
            return Err(fail(3, "retained input path mismatch"));
        };
        if super::digest::file(&path)? != input.digest {
            return Err(fail(3, "retained source digest mismatch"));
        }
    }
    Ok(frozen)
}

impl Host {
    fn snapshot(&self, online: bool) -> Json {
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
        ])
    }

    fn append(&mut self, kind: &str, value: Json) -> Result<(), Failure> {
        let mut record = object([
            ("kind", text(kind)),
            ("value", value),
            ("previousDigest", text(&self.last_digest)),
        ]);
        let digest = super::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
        if let Json::Object(fields) = &mut record {
            fields.insert("digest".into(), text(&digest));
        }
        self.journal
            .write_all((record.compact() + "\n").as_bytes())?;
        self.journal.sync_data()?;
        self.last_digest = digest;
        Ok(())
    }

    fn persist(&mut self) -> Result<(), Failure> {
        let snapshot = self.snapshot(true);
        self.append("state", snapshot.clone())?;
        save_json(&self.root.join("status.json"), &snapshot)
    }

    fn receipt(&self, id: &str) -> Json {
        let mut receipt = self.receipts.get(id).cloned().unwrap_or(Json::Null);
        if let Json::Object(fields) = &mut receipt {
            fields.insert("currentRevision".into(), Json::Int(self.revision));
        }
        receipt
    }

    fn complete(&mut self, id: &str) -> Result<(), Failure> {
        self.revision += 1;
        self.committed = Some(id.to_string());
        self.active = None;
        self.recovery_reason = None;
        let receipt = self
            .receipts
            .get_mut(id)
            .and_then(|receipt| match receipt {
                Json::Object(fields) => Some(fields),
                _ => None,
            })
            .unwrap();
        receipt.insert("state".into(), text("completed"));
        receipt.insert("revision".into(), Json::Int(self.revision));
        self.persist()?;
        let receipt = self.receipt(id);
        for waiter in self.waiters.remove(id).unwrap_or_default() {
            let _ = waiter.send(Ok(receipt.clone()));
        }
        Ok(())
    }

    fn reject_operation(&mut self, id: &str, reason: &str) -> Result<(), Failure> {
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(id)
            && receipt.get("state").and_then(Json::as_str) != Some("completed")
        {
            receipt.insert("state".into(), text("recovery_required"));
            receipt.insert("reason".into(), text(reason));
            receipt.insert(
                "nextAction".into(),
                text("inspect retained ownership and journal before recovery"),
            );
        }
        self.active = Some(id.to_string());
        self.recovery_reason = Some(reason.to_string());
        self.persist()?;
        for waiter in self.waiters.remove(id).unwrap_or_default() {
            let _ = waiter.send(Err(fail(1, reason)));
        }
        Ok(())
    }

    fn prepare(&mut self, payload: Json, reply: Reply) -> Result<(), Failure> {
        if self.preparing {
            return Err(fail(2, "host is already preparing a plan"));
        }
        let source = PathBuf::from(required(&payload, "source")?);
        let options = crate::PlanOptions {
            assembly: payload
                .get("assembly")
                .and_then(Json::as_str)
                .map(str::to_string),
            fuel: payload
                .get("fuel")
                .and_then(Json::as_i64)
                .and_then(|value| u64::try_from(value).ok()),
            std: payload.get("std").and_then(Json::as_str).map(PathBuf::from),
        };
        let build = crate::RunOptions {
            target: match required(&payload, "target")? {
                "rust" => crate::Target::Rust,
                "llvm" => crate::Target::Llvm,
                _ => return Err(fail(2, "unsupported target")),
            },
            release: payload
                .get("release")
                .and_then(Json::as_bool)
                .unwrap_or(false),
            std: options.std.clone(),
            runtime: payload
                .get("runtime")
                .and_then(Json::as_str)
                .map(PathBuf::from),
            build: Some(
                self.root.join("build").join(format!(
                    "{}-{}",
                    required(&payload, "target")?,
                    if payload
                        .get("release")
                        .and_then(Json::as_bool)
                        .unwrap_or(false)
                    {
                        "release"
                    } else {
                        "dev"
                    }
                )),
            ),
            ..crate::RunOptions::default()
        };
        let root = self.root.join("preparations").join(format!(
            "{}-{}",
            std::process::id(),
            super::NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
        ));
        let identity = self.identity.clone();
        let revision = self.revision;
        let tx = self.tx.clone();
        let previous = self
            .committed
            .as_ref()
            .map(|id| {
                read_json(&self.root.join("plans").join(format!("{id}.json")))
                    .and_then(|value| Plan::parse(&value).map_err(|reason| fail(3, reason)))
            })
            .transpose()?;
        let ownership = Json::Object(self.owned.clone());
        self.preparing = true;
        std::thread::spawn(move || {
            let result =
                super::plan::prepare(&source, &options, &identity, revision, &build, &root)
                    .map_err(Failure::from)
                    .and_then(|mut plan| {
                        plan.annotate(previous.as_ref(), &ownership)
                            .map_err(|reason| fail(2, reason))?;
                        Ok(plan)
                    });
            let _ = tx.send(Message::Prepared(Box::new(result), reply));
        });
        Ok(())
    }

    fn apply(&mut self, payload: Json, reply: Reply) -> Result<(), Failure> {
        let plan = Plan::parse(&payload).map_err(|reason| fail(2, reason))?;
        if plan.state_identity != self.identity {
            return Err(fail(2, "plan targets another host state"));
        }
        if let Some(receipt) = self.receipts.get(&plan.id) {
            let recorded = read_json(&self.root.join("plans").join(format!("{}.json", plan.id)))?;
            if recorded != payload {
                return Err(fail(2, "plan payload differs from accepted operation"));
            }
            match receipt.get("state").and_then(Json::as_str) {
                Some("completed") => {
                    let _ = reply.send(Ok(self.receipt(&plan.id)));
                }
                Some("accepted" | "applying") => {
                    self.waiters.entry(plan.id).or_default().push(reply)
                }
                _ => {
                    let _ = reply.send(Err(fail(
                        1,
                        "operation requires recovery; inspect its durable status",
                    )));
                }
            }
            return Ok(());
        }
        if self.active.is_some() {
            return Err(fail(2, "host has an active or unresolved operation"));
        }
        if plan.base_revision != self.revision {
            return Err(fail(2, "stale base revision; prepare a fresh plan"));
        }
        let trusted = read_json(&self.root.join("plans").join(format!("{}.json", plan.id)))?;
        if trusted != payload {
            return Err(fail(2, "plan was not prepared by this host"));
        }
        plan.verify_inputs().map_err(|reason| fail(2, reason))?;
        plan.verify_ownership(&Json::Object(self.owned.clone()))
            .map_err(|reason| fail(2, reason))?;
        let unchanged = self
            .committed
            .as_ref()
            .map(|id| read_json(&self.root.join("plans").join(format!("{id}.json"))))
            .transpose()?
            .and_then(|old| {
                old.get("operationalIdentity")
                    .and_then(Json::as_str)
                    .map(str::to_string)
            })
            .as_deref()
            == Some(&plan.operational_identity);
        if !unchanged {
            let actions = plan.actions.get("members").and_then(Json::as_array).ok_or_else(|| fail(2, "plan has no reviewed member actions"))?;
            if self.owned.len() > 1 || self.owned.keys().any(|member| actions.iter().any(|action| action.get("member").and_then(Json::as_str) == Some(member) && action.get("action").and_then(Json::as_str) == Some("unchanged"))) {
                return Err(fail(2, "unsupported partial revision: preserving unaffected owned members requires the differential transition driver"));
            }
        }
        if !unchanged {
            retain_context(&self.root, &plan)?;
            let context = super::revision::Context::load(retained_plan(&self.root, &plan)?)?;
            let supervised: BTreeSet<_> = context.model.machines.iter().filter(|machine| machine.variant() == "Supervise").map(|machine| machine.member.as_str()).collect();
            if context.model.edges.iter().any(|edge| supervised.contains(edge.from.as_str()) && supervised.contains(edge.to.as_str()) && matches!(edge.kind, ignite_eval::EdgeKind::Readiness | ignite_eval::EdgeKind::References | ignite_eval::EdgeKind::Uses)) {
                return Err(fail(2, "unsupported required startup order between native processes"));
            }
            self.contexts.insert(plan.id.clone(), context);
        }
        self.receipts.insert(
            plan.id.clone(),
            object([
                ("operationId", text(&plan.id)),
                ("planId", text(&plan.id)),
                (
                    "state",
                    text(if unchanged { "completed" } else { "accepted" }),
                ),
                ("baseRevision", Json::Int(self.revision)),
                ("revision", Json::Int(self.revision)),
                ("noop", Json::Bool(unchanged)),
            ]),
        );
        if unchanged {
            self.persist()?;
            let _ = reply.send(Ok(self.receipt(&plan.id)));
            return Ok(());
        }
        self.active = Some(plan.id.clone());
        self.persist()?;
        self.waiters.entry(plan.id.clone()).or_default().push(reply);
        if let Some(runtime) = &self.runtime {
            runtime
                .control
                .send(Observation::Control("stop".into()))
                .map_err(|_| fail(1, "old revision controller is unavailable"))?;
        } else {
            self.start(plan)?;
        }
        Ok(())
    }

    fn start(&mut self, plan: Plan) -> Result<(), Failure> {
        let id = plan.id.clone();
        let context = match self.contexts.remove(&id) {
            Some(context) => context,
            None => super::revision::Context::load(retained_plan(&self.root, &plan)?)?,
        };
        let expected = context
            .model
            .machines
            .iter()
            .filter(|machine| machine.variant() == "Supervise")
            .map(|machine| machine.id.clone())
            .collect();
        let (control, rx) = mpsc::channel();
        let tx = self.tx.clone();
        let operation = id.clone();
        let sink: crate::run::live::RecordSink = Box::new(move |record| {
            let (ack, wait) = mpsc::channel();
            let value = Json::parse(&record.line()).map_err(io::Error::other)?;
            tx.send(Message::Record(operation.clone(), value, ack))
                .map_err(io::Error::other)?;
            wait.recv().map_err(io::Error::other)?
        });
        let tx = self.tx.clone();
        let thread_control = control.clone();
        let runtime_state = self.root.join("revisions").join(format!(
            "{}-{}-{}",
            self.revision + 1,
            std::process::id(),
            super::NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&runtime_state)?;
        save_json(
            &runtime_state.join("context.json"),
            &object([
                ("formatVersion", text(FORMAT)),
                ("planId", text(&id)),
                ("controllerAbi", text(super::plan::CONTROLLER_ABI)),
            ]),
        )?;
        self.runtime = Some(Runtime {
            operation: id.clone(),
            control,
            ready: BTreeSet::new(),
            expected,
        });
        std::thread::spawn(move || {
            let initialized_tx = tx.clone();
            let initialized_id = id.clone();
            let started = Box::new(move || {
                initialized_tx
                    .send(Message::Initialized(initialized_id))
                    .map_err(io::Error::other)
            });
            let result =
                super::revision::run(context, &runtime_state, thread_control, rx, sink, started)
                    .map_err(|error| error.to_string());
            let _ = tx.send(Message::Finished(id, result));
        });
        Ok(())
    }

    fn record(&mut self, operation: &str, record: Json) -> Result<(), Failure> {
        self.append(
            "revisionRecord",
            object([("operationId", text(operation)), ("record", record.clone())]),
        )?;
        let kind = record.get("kind").and_then(Json::as_str);
        if kind == Some("effect") && self.recovery_reason.is_some() {
            return Err(fail(1, "effect dispatch blocked by unresolved recovery"));
        }
        if kind == Some("error") {
            self.reject_operation(
                operation,
                "revision reducer failed; inspect retained journal",
            )?;
        }
        let event = record
            .get("value")
            .and_then(|value| value.get("fields"))
            .and_then(|fields| fields.get("kind"));
        let variant = event
            .and_then(|value| value.get("variant"))
            .and_then(Json::as_str);
        let machine = record.get("machine").and_then(Json::as_str).unwrap_or("");
        let owns_machine = self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.expected.contains(machine));
        if kind == Some("event") && owns_machine {
            let member = machine
                .rsplit_once('.')
                .map_or(machine, |(member, _)| member);
            let pid = event
                .and_then(|value| value.get("fields"))
                .and_then(Json::as_array)
                .and_then(|fields| fields.first())
                .and_then(Json::as_i64);
            if let Some("Started") = variant {
                if let Some(runtime) = &mut self.runtime {
                    runtime.ready.remove(machine);
                }
                if let Some(pid) = event
                    .and_then(|value| value.get("fields"))
                    .and_then(Json::as_array)
                    .and_then(|fields| fields.first())
                    .and_then(Json::as_i64)
                {
                    self.owned.insert(
                        member.to_string(),
                        object([
                            ("pid", Json::Int(pid)),
                            ("ready", Json::Bool(false)),
                            ("operationId", text(operation)),
                            ("instance", process_identity(pid).map_or(Json::Null, text)),
                        ]),
                    );
                }
            }
            let matches_instance = self
                .owned
                .get(member)
                .and_then(|owned| owned.get("pid"))
                .and_then(Json::as_i64)
                == pid;
            if variant == Some("Ready") && matches_instance {
                if let Some(Json::Object(owned)) = self.owned.get_mut(member) {
                    owned.insert("ready".into(), Json::Bool(true));
                }
                if let Some(runtime) = &mut self.runtime {
                    runtime.ready.insert(machine.to_string());
                }
            }
            if matches!(variant, Some("Exited" | "Signalled")) && matches_instance {
                self.owned.remove(member);
                if let Some(runtime) = &mut self.runtime {
                    runtime.ready.remove(machine);
                }
            }
            if variant == Some("SpawnFailed") && self.active.as_deref() == Some(operation) {
                if let Some(runtime) = &mut self.runtime {
                    runtime.ready.remove(machine);
                }
                self.reject_operation(operation, "a required owned process failed to start")?;
            }
        }
        if self.recovery_reason.is_none()
            && self.active.as_deref() == Some(operation)
            && self.runtime.as_ref().is_some_and(|runtime| {
                !runtime.expected.is_empty() && runtime.expected.is_subset(&runtime.ready)
            })
        {
            self.complete(operation)?;
        }
        Ok(())
    }
}

/// Run one explicit host. Only this queue writes its journal and acknowledges
/// effects; socket clients never dispatch process or journal paths.
pub fn run(state: &Path) -> Result<i32, Failure> {
    std::fs::create_dir_all(state)?;
    let root = std::fs::canonicalize(state)?;
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join("lock"))?;
    let _lock = Flock::lock(lock_file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
        fail(
            3,
            format!("state is locked by another host or run: {error}"),
        )
    })?;
    let identity_path = root.join("identity");
    let identity = if identity_path.exists() {
        std::fs::read_to_string(&identity_path)?
    } else {
        let mut random = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let id = super::digest::bytes(&random);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&identity_path)?;
        file.write_all(id.as_bytes())?;
        file.sync_all()?;
        File::open(&root)?.sync_all()?;
        id
    };
    std::fs::create_dir_all(root.join("plans"))?;
    let journal_path = root.join("host-journal.jsonl");
    let mut previous = None;
    let mut last_digest = String::new();
    let mut intents = BTreeSet::new();
    if journal_path.exists() {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&journal_path)?;
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
            if required(&record, "previousDigest")? != last_digest {
                return Err(fail(3, "host journal digest chain diverged"));
            }
            if let Json::Object(fields) = &mut record {
                fields.remove("digest");
            }
            let expected =
                super::digest::bytes(format!("{FORMAT}\0{}", record.compact()).as_bytes());
            if digest != expected {
                return Err(fail(3, "host journal record digest mismatch"));
            }
            last_digest = digest;
            if record.get("kind").and_then(Json::as_str) == Some("revisionRecord")
                && let Some(value) = record.get("value")
                && value
                    .get("record")
                    .and_then(|record| record.get("kind"))
                    .and_then(Json::as_str)
                    == Some("effect")
                && let Some(id) = value.get("operationId").and_then(Json::as_str)
            {
                intents.insert(id.to_string());
            }
            if record.get("kind").and_then(Json::as_str) == Some("state") {
                previous = record.get("value").cloned();
            }
        }
    }
    let (tx, rx) = mpsc::channel();
    let journal = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&journal_path)?;
    let mut host = Host {
        root: root.clone(),
        identity,
        revision: 0,
        committed: None,
        active: None,
        recovery_reason: None,
        receipts: BTreeMap::new(),
        owned: BTreeMap::new(),
        journal,
        last_digest,
        runtime: None,
        contexts: BTreeMap::new(),
        preparing: false,
        waiters: BTreeMap::new(),
        tx: tx.clone(),
    };
    if let Some(previous) = previous {
        if required(&previous, "formatVersion")? != FORMAT
            || required(&previous, "stateIdentity")? != host.identity
        {
            return Err(fail(3, "host journal version/state identity mismatch"));
        }
        host.revision = previous
            .get("committedRevision")
            .and_then(Json::as_i64)
            .unwrap_or(0);
        host.committed = previous
            .get("committedPlan")
            .and_then(Json::as_str)
            .map(str::to_string);
        host.active = previous
            .get("activeOperation")
            .and_then(Json::as_str)
            .map(str::to_string);
        host.recovery_reason = previous
            .get("recoveryReason")
            .and_then(Json::as_str)
            .map(str::to_string);
        host.receipts = previous
            .get("operations")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        host.owned = previous
            .get("ownership")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
    }
    let socket_path = root.join("control.sock");
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }
    let (_state_directory, socket_address) = super::socket_address(&root)?;
    let listener = UnixListener::bind(socket_address)?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let stopping = Arc::new(AtomicBool::new(false));
    let accept_stop = Arc::clone(&stopping);
    let accept_tx = tx.clone();
    std::thread::spawn(move || {
        while !accept_stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let tx = accept_tx.clone();
                    std::thread::spawn(move || {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
                        let result = read_frame(&mut stream);
                        let id = result
                            .as_ref()
                            .ok()
                            .and_then(|request| request.get("requestId"))
                            .and_then(Json::as_str)
                            .unwrap_or("")
                            .to_string();
                        let result = result.map_err(Failure::from).and_then(|request| {
                            let (reply, wait) = mpsc::channel();
                            tx.send(Message::Request(request, reply))
                                .map_err(|_| fail(3, "host queue closed"))?;
                            wait.recv()
                                .map_err(|_| fail(3, "host stopped before replying"))?
                        });
                        let (status, reason, payload) = match result {
                            Ok(value) => (0, String::new(), value),
                            Err(error) => (error.status, error.reason, Json::Null),
                        };
                        let response = object([
                            ("version", text(PROTOCOL)),
                            ("requestId", text(id)),
                            ("status", Json::Int(i64::from(status))),
                            ("reason", text(reason)),
                            ("payload", payload),
                        ]);
                        let _ = write_frame(&mut stream, &response);
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(_) => break,
            }
        }
    });
    let (control_tx, control_rx) = mpsc::channel();
    crate::run::control::read_control(control_tx.clone());
    crate::run::control::watch_signals(control_tx)?;
    let control_host = tx.clone();
    std::thread::spawn(move || {
        for observation in control_rx {
            match observation {
                Observation::Interrupt => {
                    let _ = control_host.send(Message::Stop);
                }
                Observation::Control(command) if command == "stop" => {
                    let _ = control_host.send(Message::Stop);
                }
                Observation::Control(command) if command == "status" => {
                    let _ = control_host.send(Message::Status);
                }
                _ => {}
            }
        }
    });
    if root.join("revisions").is_dir() {
        for entry in std::fs::read_dir(root.join("revisions"))? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let metadata = read_json(&path.join("context.json"))?;
            if required(&metadata, "formatVersion")? != FORMAT
                || required(&metadata, "controllerAbi")? != super::plan::CONTROLLER_ABI
            {
                return Err(fail(3, "unsupported historical host/reducer version"));
            }
            let id = required(&metadata, "planId")?;
            let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
            let plan = Plan::parse(&value).map_err(|reason| fail(3, reason))?;
            let context = super::revision::Context::load(retained_plan(&root, &plan)?)?;
            super::revision::verify(context, &path)?;
        }
    }
    let resume = if host.recovery_reason.is_none() {
        host.active.clone().or_else(|| host.committed.clone())
    } else {
        None
    };
    if let Some(id) = resume {
        let live_owned = host.owned.values().any(|owned| {
            let pid = owned.get("pid").and_then(Json::as_i64);
            let instance = owned.get("instance").and_then(Json::as_str);
            pid.zip(instance)
                .is_some_and(|(pid, instance)| process_identity(pid).as_deref() == Some(instance))
        });
        if live_owned || generation_process_exists(&root)? {
            host.reject_operation(&id, "an earlier owned process instance still exists; listener adoption requires reconciliation")?;
        } else {
            host.owned.clear();
            host.append("reconciledRevision", object([
                ("planId", text(&id)), ("priorIntents", Json::Bool(intents.contains(&id))),
                ("observation", text("previous owned instances ended; no retained-generation process remains")),
            ]))?;
            let value = read_json(&root.join("plans").join(format!("{id}.json")))?;
            host.start(Plan::parse(&value).map_err(|reason| fail(3, reason))?)?;
        }
    }
    host.persist()?;
    eprintln!("ignite host: {} revision {}", host.identity, host.revision);
    let mut shutdown = false;
    while let Ok(message) = rx.recv() {
        match message {
            Message::Request(request, reply) => {
                let result = (|| {
                    if required(&request, "version")? != PROTOCOL {
                        return Err(fail(2, "unsupported control protocol"));
                    }
                    required(&request, "requestId")?;
                    let operation = required(&request, "operation")?;
                    let identity = required(&request, "stateIdentity")?;
                    if matches!(operation, "prepare" | "apply") && identity.is_empty() {
                        return Err(fail(2, "request requires state identity"));
                    }
                    if !identity.is_empty() && identity != host.identity {
                        return Err(fail(2, "request targets another host state"));
                    }
                    let payload = request.get("payload").cloned().unwrap_or(Json::Null);
                    match operation {
                        "snapshot" | "inspect" => {
                            let mut snapshot = host.snapshot(true);
                            if let Some(id) = payload.as_str() {
                                if !host.receipts.contains_key(id) {
                                    return Err(fail(2, "unknown operation"));
                                }
                                if let Json::Object(fields) = &mut snapshot {
                                    fields.insert("operation".into(), host.receipt(id));
                                }
                            }
                            let _ = reply.send(Ok(snapshot));
                        }
                        "prepare" if !shutdown => host.prepare(payload, reply.clone())?,
                        "apply" if !shutdown => host.apply(payload, reply.clone())?,
                        _ => return Err(fail(2, "unsupported operation or host is stopping")),
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = reply.send(Err(error));
                }
            }
            Message::Prepared(result, reply) => {
                host.preparing = false;
                let result = (*result).and_then(|plan| {
                    let value = plan.json();
                    let path = host.root.join("plans").join(format!("{}.json", plan.id));
                    if path.exists() {
                        if read_json(&path)? != value {
                            return Err(fail(3, "prepared plan identity collision"));
                        }
                    } else {
                        save_json(&path, &value)?;
                    }
                    Ok(value)
                });
                let _ = reply.send(result);
            }
            Message::Record(id, record, ack) => {
                let result = host.record(&id, record);
                let failed = result.as_ref().is_err_and(|error| error.status == 3);
                let _ = ack.send(result.map_err(|error| io::Error::other(error.to_string())));
                if failed {
                    stopping.store(true, Ordering::Relaxed);
                    return Err(fail(3, "host durable record failed; mutation stopped"));
                }
            }
            Message::Finished(id, result) => {
                host.runtime = None;
                host.owned.clear();
                if shutdown {
                    break;
                }
                if let Some(active) = host.active.clone() {
                    if active != id && matches!(result, Ok(0)) {
                        let value =
                            read_json(&host.root.join("plans").join(format!("{active}.json")))?;
                        host.start(Plan::parse(&value).map_err(|reason| fail(3, reason))?)?;
                    } else {
                        host.reject_operation(
                            &active,
                            &format!("revision controller ended before convergence: {result:?}"),
                        )?;
                    }
                } else if !shutdown && let Some(committed) = host.committed.clone() {
                    host.reject_operation(
                        &committed,
                        &format!("committed revision controller ended: {result:?}"),
                    )?;
                }
            }
            Message::Initialized(id) => {
                if host.active.as_deref() == Some(&id)
                    && host
                        .runtime
                        .as_ref()
                        .is_some_and(|runtime| runtime.expected.is_empty())
                {
                    host.complete(&id)?;
                }
            }
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
