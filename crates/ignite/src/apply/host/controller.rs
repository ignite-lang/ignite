//! Driving admitted revisions: the bridge to the run driver, the records it
//! sends back, ownership and readiness of the owned instances, and the
//! completion or rejection of an operation.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::Ordering, mpsc};

use ignite_eval::Json;

use super::admission::retained_plan;
use super::journal::{FORMAT, save_json};
use super::{Host, Message, read_json};
use crate::apply::{Failure, fail, object, plan::Plan, text};
use crate::run::control::Observation;

/// The revision controller running on the run driver's thread.
pub(super) struct Runtime {
    pub(super) operation: String,
    pub(super) controller_plan: String,
    pub(super) operation_sink: Arc<Mutex<String>>,
    pub(super) artifacts: Arc<Mutex<BTreeMap<String, PathBuf>>>,
    pub(super) minimum_generation: Option<i64>,
    pub(super) control: mpsc::Sender<Observation>,
    pub(super) ready: BTreeSet<String>,
    pub(super) expected: BTreeSet<String>,
}

/// The start time of a live process, `None` when it is gone or a zombie.
pub(super) fn process_identity(pid: i64) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat.rsplit_once(") ")?.1;
    if matches!(fields.split_whitespace().next(), Some("Z" | "X")) {
        return None;
    }
    Some(fields.split_whitespace().nth(19)?.to_string())
}

impl Host {
    pub(super) fn complete(&mut self, id: &str) -> Result<(), Failure> {
        self.revision += 1;
        self.committed = Some(id.to_string());
        self.contexts.remove(id);
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

    pub(super) fn reject_operation(&mut self, id: &str, reason: &str) -> Result<(), Failure> {
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

    /// Hand an admitted, journaled operation to the controller: roll
    /// `rollout_member` in the running controller, stop the old controller
    /// (its end starts the new one), or start the first one.
    pub(super) fn dispatch(
        &mut self,
        plan: Plan,
        rollout_member: Option<String>,
    ) -> Result<(), Failure> {
        if let Some(member) = rollout_member {
            let frozen = &self.contexts.get(&plan.id).expect("admitted context").plan;
            let artifacts: BTreeMap<_, _> = frozen
                .artifacts
                .iter()
                .map(|(member, artifact)| (member.clone(), artifact.path.clone()))
                .collect();
            let controller_plan = self.runtime.as_ref().unwrap().controller_plan.clone();
            self.append(
                "revisionTransition",
                object([
                    ("operationId", text(&plan.id)),
                    ("controllerPlanId", text(controller_plan)),
                    ("member", text(&member)),
                    ("artifacts", plan.json().get("artifacts").cloned().unwrap()),
                ]),
            )?;
            let runtime = self.runtime.as_mut().unwrap();
            *runtime
                .artifacts
                .lock()
                .map_err(|_| fail(3, "artifact manifest lock failed"))? = artifacts;
            *runtime
                .operation_sink
                .lock()
                .map_err(|_| fail(3, "operation identity lock failed"))? = plan.id.clone();
            runtime.operation = plan.id.clone();
            runtime.minimum_generation = Some(
                self.owned
                    .get(&member)
                    .and_then(|owner| owner.get("generation"))
                    .and_then(Json::as_i64)
                    .unwrap_or(1)
                    + 1,
            );
            runtime.ready.clear();
            runtime
                .control
                .send(Observation::Control(format!("rollout {member}")))
                .map_err(|_| fail(1, "revision controller is unavailable"))?;
        } else if let Some(runtime) = &self.runtime {
            runtime
                .control
                .send(Observation::Control("stop".into()))
                .map_err(|_| fail(1, "old revision controller is unavailable"))?;
        } else {
            self.start(plan)?;
        }
        Ok(())
    }

    pub(super) fn start(&mut self, plan: Plan) -> Result<(), Failure> {
        let id = plan.id.clone();
        let context = match self.contexts.remove(&id) {
            Some(context) => context,
            None => crate::apply::revision::Context::load(retained_plan(&self.root, &plan)?)?,
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
        let operation_sink = Arc::new(Mutex::new(id.clone()));
        let operation = Arc::clone(&operation_sink);
        let controller_plan = id.clone();
        let sink: crate::run::live::RecordSink = Box::new(move |record| {
            let (ack, wait) = mpsc::channel();
            let value = Json::parse(&record.line()).map_err(io::Error::other)?;
            let operation = operation
                .lock()
                .map_err(|_| io::Error::other("operation identity lock failed"))?
                .clone();
            tx.send(Message::Record(
                operation,
                controller_plan.clone(),
                value,
                ack,
            ))
            .map_err(io::Error::other)?;
            wait.recv().map_err(io::Error::other)?
        });
        let tx = self.tx.clone();
        let thread_control = control.clone();
        let runtime_state = self.root.join("revisions").join(format!(
            "{}-{}-{}",
            self.revision + 1,
            std::process::id(),
            crate::apply::NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&runtime_state)?;
        save_json(
            &runtime_state.join("context.json"),
            &object([
                ("formatVersion", text(FORMAT)),
                ("planId", text(&id)),
                ("controllerAbi", text(crate::apply::plan::CONTROLLER_ABI)),
            ]),
        )?;
        let artifacts = Arc::new(Mutex::new(
            context
                .plan
                .artifacts
                .iter()
                .map(|(member, artifact)| (member.clone(), artifact.path.clone()))
                .collect::<BTreeMap<_, _>>(),
        ));
        let builder_artifacts = Arc::clone(&artifacts);
        let builder: crate::run::live::Builder = Arc::new(move || {
            builder_artifacts
                .lock()
                .map(|artifacts| artifacts.clone())
                .map_err(|_| "artifact manifest lock failed".to_string())
        });
        self.runtime = Some(Runtime {
            operation: id.clone(),
            controller_plan: id.clone(),
            operation_sink,
            artifacts,
            minimum_generation: None,
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
            let result = crate::apply::revision::run(
                context,
                &runtime_state,
                thread_control,
                rx,
                sink,
                started,
                builder,
            )
            .map_err(|error| error.to_string());
            let _ = tx.send(Message::Finished(id, result));
        });
        Ok(())
    }

    /// The controller of `id` initialized; a revision without supervised
    /// processes is complete now.
    pub(super) fn initialized(&mut self, id: &str) -> Result<(), Failure> {
        if self.active.as_deref() == Some(id)
            && self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.expected.is_empty())
        {
            self.complete(id)?;
        }
        Ok(())
    }

    /// The controller of `id` ended. Unless the host is shutting down, an
    /// active operation of another plan starts after a clean stop; any other
    /// end requires recovery.
    pub(super) fn finished(
        &mut self,
        id: &str,
        result: Result<i32, String>,
        shutdown: bool,
    ) -> Result<(), Failure> {
        self.runtime = None;
        self.owned.clear();
        self.retiring.clear();
        if shutdown {
            return Ok(());
        }
        if let Some(active) = self.active.clone() {
            if active != id && matches!(result, Ok(0)) {
                let value = read_json(&self.root.join("plans").join(format!("{active}.json")))?;
                self.start(Plan::parse(&value).map_err(|reason| fail(3, reason))?)?;
            } else {
                self.reject_operation(
                    &active,
                    &format!("revision controller ended before convergence: {result:?}"),
                )?;
            }
        } else if let Some(committed) = self.committed.clone() {
            self.reject_operation(
                &committed,
                &format!("committed revision controller ended: {result:?}"),
            )?;
        }
        Ok(())
    }

    pub(super) fn record(
        &mut self,
        operation: &str,
        controller: &str,
        record: Json,
    ) -> Result<(), Failure> {
        self.append(
            "revisionRecord",
            object([
                ("operationId", text(operation)),
                ("controllerPlanId", text(controller)),
                ("record", record.clone()),
            ]),
        )?;
        let kind = record.get("kind").and_then(Json::as_str);
        if let Some(Json::Object(receipt)) = self.receipts.get_mut(operation)
            && matches!(
                receipt.get("state").and_then(Json::as_str),
                Some("accepted" | "applying")
            )
        {
            receipt.insert("progress".into(), record.clone());
        }
        if kind == Some("standing")
            && let Some(machine) = record.get("machine").and_then(Json::as_str)
        {
            self.standings.insert(machine.to_string(), record.clone());
        }
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
                    if let Some(previous) = self.owned.get(member)
                        && let Some(old_pid) = previous.get("pid").and_then(Json::as_i64)
                        && old_pid != pid
                        && previous
                            .get("instance")
                            .and_then(Json::as_str)
                            .is_some_and(|instance| {
                                process_identity(old_pid).as_deref() == Some(instance)
                            })
                    {
                        self.retiring.insert(old_pid.to_string(), previous.clone());
                    }
                    let generation = event
                        .and_then(|value| value.get("fields"))
                        .and_then(Json::as_array)
                        .and_then(|fields| fields.get(1))
                        .and_then(Json::as_i64)
                        .unwrap_or(1);
                    self.owned.insert(
                        member.to_string(),
                        object([
                            ("pid", Json::Int(pid)),
                            ("ready", Json::Bool(false)),
                            ("operationId", text(operation)),
                            ("instance", process_identity(pid).map_or(Json::Null, text)),
                            ("generation", Json::Int(generation)),
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
            let desired_generation = self
                .owned
                .get(member)
                .and_then(|owner| owner.get("generation"))
                .and_then(Json::as_i64)
                .is_some_and(|generation| {
                    self.runtime
                        .as_ref()
                        .and_then(|runtime| runtime.minimum_generation)
                        .is_none_or(|minimum| generation >= minimum)
                });
            if variant == Some("Ready") && matches_instance && desired_generation {
                if let Some(id) = self.active.as_ref().or(self.committed.as_ref()) {
                    let manifest = read_json(&self.root.join("plans").join(format!("{id}.json")))?;
                    let expected = manifest
                        .get("artifacts")
                        .and_then(|artifacts| artifacts.get(member))
                        .and_then(|artifact| artifact.get("digest"))
                        .and_then(Json::as_str);
                    if let Some(pid) = pid {
                        let actual = match crate::apply::digest::file(Path::new(&format!(
                            "/proc/{pid}/exe"
                        ))) {
                            Ok(digest) => digest,
                            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                                return Ok(());
                            }
                            Err(error) => {
                                return Err(fail(
                                    1,
                                    format!("cannot prove ready process artifact: {error}"),
                                ));
                            }
                        };
                        if expected != Some(actual.as_str()) {
                            let id = id.clone();
                            self.reject_operation(
                                &id,
                                "ready process artifact differs from the admitted desired artifact",
                            )?;
                            return Err(fail(1, "ready artifact identity mismatch"));
                        }
                    }
                }
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
            if matches!(variant, Some("Exited" | "Signalled"))
                && let Some(pid) = pid
            {
                self.retiring.remove(&pid.to_string());
            }
            if variant == Some("SpawnFailed") && self.active.as_deref() == Some(operation) {
                if let Some(runtime) = &mut self.runtime {
                    runtime.ready.remove(machine);
                }
                self.reject_operation(operation, "a required owned process failed to start")?;
            }
        }
        if self.recovery_reason.is_none()
            && self.retiring.is_empty()
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
