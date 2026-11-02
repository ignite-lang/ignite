//! Plan admission: preparing a plan against this host's state, admitting a
//! saved plan (identity, stale base, trusted payload, input pins, ownership),
//! the operation receipts, and the retained context of an admitted plan.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use ignite_eval::Json;

use super::journal::{save_json, sync_directories};
use super::{Host, Message, Reply, read_json};
use crate::apply::{Failure, fail, object, plan::Plan, required, text};

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
        if crate::apply::digest::file(&destination)? != input.digest {
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
        if crate::apply::digest::file(&destination)? != artifact.digest {
            return Err(fail(2, "artifact changed while retaining admitted context"));
        }
        File::open(&destination)?.sync_all()?;
    }
    sync_directories(&generation)?;
    File::open(generation.parent().unwrap())?.sync_all()?;
    Ok(())
}

/// The admitted plan with its sources and artifacts at their retained paths,
/// checked against the pins and the admitting compiler's bytes.
pub(super) fn retained_plan(root: &Path, plan: &Plan) -> Result<Plan, Failure> {
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
    if crate::apply::digest::file(&executable)? != compiler.digest {
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
        if crate::apply::digest::file(&artifact.path)? != artifact.digest {
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
        if crate::apply::digest::file(&path)? != input.digest {
            return Err(fail(3, "retained source digest mismatch"));
        }
    }
    Ok(frozen)
}

fn reducer_inputs(plan: &Plan) -> BTreeMap<PathBuf, String> {
    let root = plan.std.clone().unwrap_or_else(crate::default_std_path);
    plan.inputs
        .iter()
        .filter_map(|input| {
            input
                .path
                .strip_prefix(&root)
                .ok()
                .map(|path| (path.to_path_buf(), input.digest.clone()))
        })
        .collect()
}

impl Host {
    pub(super) fn receipt(&self, id: &str) -> Json {
        let mut receipt = self.receipts.get(id).cloned().unwrap_or(Json::Null);
        if let Json::Object(fields) = &mut receipt {
            fields.insert("currentRevision".into(), Json::Int(self.revision));
        }
        receipt
    }

    pub(super) fn prepare(&mut self, payload: Json, reply: Reply) -> Result<(), Failure> {
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
            build: Some(self.root.join("build").join(format!(
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
                ))),
            ..crate::RunOptions::default()
        };
        let root = self.root.join("preparations").join(format!(
            "{}-{}",
            std::process::id(),
            crate::apply::NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
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
                crate::apply::plan::prepare(&source, &options, &identity, revision, &build, &root)
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

    /// Record a finished preparation as a plan this host prepared.
    pub(super) fn prepared(&mut self, result: Result<Plan, Failure>) -> Result<Json, Failure> {
        self.preparing = false;
        result.and_then(|plan| {
            let value = plan.json();
            let path = self.root.join("plans").join(format!("{}.json", plan.id));
            if path.exists() {
                if read_json(&path)? != value {
                    return Err(fail(3, "prepared plan identity collision"));
                }
            } else {
                save_json(&path, &value)?;
            }
            Ok(value)
        })
    }

    pub(super) fn apply(&mut self, payload: Json, reply: Reply) -> Result<(), Failure> {
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
        let previous_plan = self
            .committed
            .as_ref()
            .map(|id| {
                read_json(&self.root.join("plans").join(format!("{id}.json")))
                    .and_then(|value| Plan::parse(&value).map_err(|reason| fail(3, reason)))
            })
            .transpose()?;
        let unchanged = previous_plan
            .as_ref()
            .is_some_and(|old| old.operational_identity == plan.operational_identity);
        let mut rollout_member = None;
        if !unchanged {
            let actions = plan
                .actions
                .get("members")
                .and_then(Json::as_array)
                .ok_or_else(|| fail(2, "plan has no reviewed member actions"))?;
            if self.owned.len() > 1
                || self.owned.keys().any(|member| {
                    actions.iter().any(|action| {
                        action.get("member").and_then(Json::as_str) == Some(member)
                            && action.get("action").and_then(Json::as_str) == Some("unchanged")
                    })
                })
            {
                return Err(fail(
                    2,
                    "unsupported partial revision: preserving unaffected owned members requires the differential transition driver",
                ));
            }
        }
        if !unchanged {
            retain_context(&self.root, &plan)?;
            let context = crate::apply::revision::Context::load(retained_plan(&self.root, &plan)?)?;
            let supervised: BTreeSet<_> = context
                .model
                .machines
                .iter()
                .filter(|machine| machine.variant() == "Supervise")
                .map(|machine| machine.member.as_str())
                .collect();
            if context.model.edges.iter().any(|edge| {
                supervised.contains(edge.from.as_str())
                    && supervised.contains(edge.to.as_str())
                    && matches!(
                        edge.kind,
                        ignite_eval::EdgeKind::Readiness
                            | ignite_eval::EdgeKind::References
                            | ignite_eval::EdgeKind::Uses
                    )
            }) {
                return Err(fail(
                    2,
                    "unsupported required startup order between native processes",
                ));
            }
            if self.owned.len() == 1 && self.runtime.is_some() {
                let (member, owner) = self.owned.first_key_value().unwrap();
                let same_controller = previous_plan.as_ref().is_some_and(|old| {
                    crate::apply::plan::semantic_model(&old.model)
                        == crate::apply::plan::semantic_model(&plan.model)
                        && reducer_inputs(old) == reducer_inputs(&plan)
                });
                let ready_instance = owner.get("ready").and_then(Json::as_bool) == Some(true)
                    && owner
                        .get("pid")
                        .and_then(Json::as_i64)
                        .zip(owner.get("instance").and_then(Json::as_str))
                        .is_some_and(|(pid, instance)| {
                            super::controller::process_identity(pid).as_deref() == Some(instance)
                        });
                if same_controller
                    && ready_instance
                    && context
                        .model
                        .machines
                        .iter()
                        .any(|machine| machine.member == *member && machine.variant() == "Roll")
                {
                    rollout_member = Some(member.clone());
                } else if !supervised.is_empty() {
                    return Err(fail(
                        2,
                        "unsupported model/controller replacement; this host currently rolls implementation-only changes through the declared drain policy",
                    ));
                }
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
        self.dispatch(plan, rollout_member)
    }
}
