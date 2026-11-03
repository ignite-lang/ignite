//! Plan admission: preparing a plan against this host's state, admitting a
//! saved plan (identity, stale base, trusted payload, controller revisions,
//! input pins, ownership), the operation receipts, and the retained context of
//! an admitted plan.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use ignite_eval::Json;

use super::journal::{save_json, sync_directories};
use super::{Host, Message, Reply, read_json};
use crate::apply::plan::{InputKind, Plan};
use crate::apply::{Failure, fail, object, required, text};

/// Where a retained module of `plan` lives under `generation`.
fn retained_path(
    generation: &Path,
    plan: &Plan,
    input: &crate::apply::plan::Input,
) -> Option<PathBuf> {
    let (root, directory) = match input.kind {
        InputKind::Std => (plan.std.as_deref()?, "std"),
        InputKind::Source => (plan.source_root.as_path(), "source"),
        _ => return None,
    };
    let relative = input.path.strip_prefix(root).ok()?;
    Some(generation.join(directory).join(relative))
}

fn is_module(input: &crate::apply::plan::Input) -> bool {
    matches!(input.kind, InputKind::Source | InputKind::Std)
}

/// Preserve the admitted source graph independently of its workspace spelling.
/// Only these frozen modules and pinned artifacts may enter the live worker.
fn retain_context(root: &Path, plan: &Plan) -> Result<(), Failure> {
    let generation = root.join("retained").join(&plan.id);
    std::fs::create_dir_all(&generation)?;
    for input in plan.inputs.iter().filter(|input| is_module(input)) {
        let destination = retained_path(&generation, plan, input).ok_or_else(|| {
            fail(
                2,
                format!(
                    "source input is outside the retained graph: {}",
                    input.path.display()
                ),
            )
        })?;
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
/// checked against the pins.
///
/// Decision D2: a host binary other than the admitting one may open history.
/// The admitting compiler's bytes are an admission pin
/// ([`Plan::verify_inputs`]), not a startup condition; what a retained
/// revision needs from this host is its reducer revision, because the host's
/// evaluator re-steps the revision's `std.control` machines over the retained
/// standard library. Startup recovery neither re-steps nor drives a revision
/// whose reducer revision differs from this host's and reports it as
/// `reducer_revision_changed` (`recovery`); admission refuses such a plan
/// ([`Plan::verify_controller`]).
pub(super) fn retained_plan(root: &Path, plan: &Plan) -> Result<Plan, Failure> {
    let generation = root.join("retained").join(&plan.id);
    let mut frozen = plan.clone();
    frozen.source = generation.join("source").join(
        plan.source
            .file_name()
            .ok_or_else(|| fail(3, "retained source name missing"))?,
    );
    frozen.source_root = generation.join("source");
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
    for input in plan.inputs.iter().filter(|input| is_module(input)) {
        let path = retained_path(&generation, plan, input)
            .ok_or_else(|| fail(3, "retained input path mismatch"))?;
        if crate::apply::digest::file(&path)? != input.digest {
            return Err(fail(3, "retained source digest mismatch"));
        }
    }
    Ok(frozen)
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
        // Decision D3: an operation a machine gave up on (or whose new
        // listener could not be bound) is resolved by a corrective plan whose
        // base is the committed revision, while that failed operation is the
        // only one: no transition in flight, the active operation's receipt
        // `recovery_required`. A startup recovery block (`outcome_unknown`,
        // `foreign_listener`) admits no apply until it is lifted.
        if self.blocked() {
            return Err(fail(
                2,
                "host recovery is blocked; send `status` to the host or restart it once the evidence is resolved",
            ));
        }
        let corrective = self.transition.is_none()
            && self
                .runtime
                .as_ref()
                .is_none_or(|runtime| runtime.restore_to.is_none())
            && self
                .recovery_reason
                .as_deref()
                .is_some_and(super::controller::resolvable)
            && self
                .active
                .as_ref()
                .and_then(|id| self.receipts.get(id))
                .and_then(|receipt| receipt.get("state"))
                .and_then(Json::as_str)
                == Some("recovery_required")
            && plan.base_revision == self.revision;
        if self.active.is_some() && !corrective {
            return Err(fail(2, "host has an active or unresolved operation"));
        }
        if plan.base_revision != self.revision {
            return Err(fail(2, "stale base revision; prepare a fresh plan"));
        }
        let prepared = self.root.join("plans").join(format!("{}.json", plan.id));
        if !prepared.is_file() || read_json(&prepared)? != payload {
            return Err(fail(2, "plan was not prepared by this host"));
        }
        plan.verify_controller().map_err(|reason| fail(2, reason))?;
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
        // An operational identity change that affects no member (a codegen
        // revision change of a system without executables) is a no-op too.
        let every_member_unchanged = plan
            .actions
            .get("members")
            .and_then(Json::as_array)
            .is_some_and(|members| {
                members
                    .iter()
                    .all(|member| member.get("kind").and_then(Json::as_str) == Some("unchanged"))
            });
        let unchanged = previous_plan.as_ref().is_some_and(|old| {
            old.operational_identity == plan.operational_identity || every_member_unchanged
        });
        // Decision D2: while the committed revision is not driven because
        // its reducer revision differs from this host's, the replanned apply
        // starts its members afresh even when nothing else changed.
        let unchanged = unchanged
            && !self.recovery.as_ref().is_some_and(|recovery| {
                recovery.get("reason").and_then(Json::as_str)
                    == Some(super::recovery::REDUCER_CHANGED)
            });
        // A corrective plan back to the committed revision while the running
        // revision is still the failed operation's: a transition from that
        // revision back, which stops what the failed operation started.
        let reverting = unchanged
            && corrective
            && self
                .runtime
                .as_ref()
                .is_some_and(|runtime| Some(&runtime.desired) != self.committed.as_ref());
        let unchanged = unchanged && !reverting;
        if !unchanged {
            retain_context(&self.root, &plan)?;
            let context = crate::apply::revision::Context::load(retained_plan(&self.root, &plan)?)?;
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
            // A corrective plan back to the committed revision resolves the
            // failed operation without a transition.
            if corrective && let Some(failed) = self.active.take() {
                self.recovery_reason = None;
                self.append(
                    "recoveryResolved",
                    object([
                        ("operationId", text(&plan.id)),
                        ("resolvedOperationId", text(&failed)),
                    ]),
                )?;
            }
            self.persist()?;
            let _ = reply.send(Ok(self.receipt(&plan.id)));
            return Ok(());
        }
        // A corrective resolves the failed operation when it is admitted
        // (D3): the corrective is the active operation from here on, and a
        // host that restarts after this acceptance resumes it.
        let resolved = if corrective { self.active.take() } else { None };
        if let Some(failed) = &resolved {
            self.recovery_reason = None;
            self.append(
                "recoveryResolved",
                object([
                    ("operationId", text(&plan.id)),
                    ("resolvedOperationId", text(failed)),
                ]),
            )?;
        }
        self.active = Some(plan.id.clone());
        self.persist()?;
        self.waiters.entry(plan.id.clone()).or_default().push(reply);
        match resolved.filter(|_| reverting) {
            Some(failed) => self.revert(plan, &failed),
            None => self.dispatch(plan),
        }
    }
}
