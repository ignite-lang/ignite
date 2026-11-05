//! Source-bound local plans (`docs/slice.md` 17.3, 17.4); ordinary model JSON
//! keeps its existing format.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use ignite_eval::Json;
use ignite_syntax::ast::{Decl, Program};

use super::digest;

pub const FORMAT: &str = "ignite-apply-plan/0.1";
pub const CONTROLLER_ABI: &str = "ignite-local-host/0.1";
/// Increment when compiler/backend generation semantics change. The admitted
/// compiler bytes are a separate raw input, so a rebuild never changes a plan.
pub const CODEGEN_REVISION: &str = "ignite-codegen/0.1.0-18";
/// Increment when the evaluator's stepping of `std.control` reducers may
/// change. With the digests of the standard-library inputs it forms a plan's
/// reducer revision.
pub const REDUCER_EVALUATOR: &str = "ignite-eval-reducers/0.1.0-1";
/// Schema of [`Plan::actions`].
pub const ACTIONS_SCHEMA: &str = "ignite-apply-actions/0.1";
/// Read only by debug builds: lets one test binary play two compilers
/// (decision D2, `docs/slice.md` 17.7 row 6).
pub const CODEGEN_OVERRIDE: &str = "IGNITE_DEBUG_CODEGEN_REVISION";

const PLAN_ID: &str = "ignite-apply-plan-id/0.1";
const OPERATIONAL: &str = "ignite-operational/0.1";
const IMPLEMENTATION: &str = "ignite-implementation/0.1";
const REDUCER: &str = "ignite-reducer/0.1";
const OBSERVED_RESOURCE: &str = "ignite-observed-resource/0.1";

/// The codegen revision this binary generates under.
pub fn codegen_revision() -> String {
    #[cfg(debug_assertions)]
    let overridden = std::env::var_os(CODEGEN_OVERRIDE);
    #[cfg(not(debug_assertions))]
    let overridden = None;
    codegen_revision_or(overridden)
}

fn codegen_revision_or(overridden: Option<std::ffi::OsString>) -> String {
    overridden.filter(|value| !value.is_empty()).map_or_else(
        || CODEGEN_REVISION.to_string(),
        |value| value.to_string_lossy().into_owned(),
    )
}

/// The versioned contract between generated code and its substrate: the
/// `ignite-io` API for Rust, the `ignite-rt` C ABI over it for LLVM. Part of
/// operational identity; the substrate's and the runtime archive's bytes are
/// admission pins only.
pub fn runtime_contract(target: crate::Target) -> &'static str {
    match target {
        crate::Target::Rust => "ignite-io/0.1",
        crate::Target::Llvm => "ignite-rt-c-abi/0.1+ignite-io/0.1",
    }
}

/// What a pinned input is. Only `Source` and `Std` feed operational identity
/// (through the implementation projection); the rest are admission pins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputKind {
    /// A reached `.ignite` module under the program's source root.
    Source,
    /// A reached standard-library module, reducers included.
    Std,
    /// The exact bytes of the admitting `ignite` executable.
    Compiler,
    /// `rustc` and `cargo` of the Rust target's sysroot.
    Toolchain,
    /// `clang` of the LLVM target.
    Clang,
    /// A build input of the `ignite-io` substrate (tests excluded).
    Substrate,
    /// The workspace manifest, lock file and toolchain file.
    Workspace,
    /// A `.cargo/config` above the source.
    Configuration,
    /// The LLVM runtime archive.
    Runtime,
}

impl InputKind {
    const ALL: [InputKind; 9] = [
        InputKind::Source,
        InputKind::Std,
        InputKind::Compiler,
        InputKind::Toolchain,
        InputKind::Clang,
        InputKind::Substrate,
        InputKind::Workspace,
        InputKind::Configuration,
        InputKind::Runtime,
    ];

    pub fn name(self) -> &'static str {
        match self {
            InputKind::Source => "source",
            InputKind::Std => "std",
            InputKind::Compiler => "compiler",
            InputKind::Toolchain => "toolchain",
            InputKind::Clang => "clang",
            InputKind::Substrate => "substrate",
            InputKind::Workspace => "workspace",
            InputKind::Configuration => "configuration",
            InputKind::Runtime => "runtime",
        }
    }

    fn parse(name: &str) -> Option<InputKind> {
        InputKind::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub path: PathBuf,
    pub kind: InputKind,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub path: PathBuf,
    pub digest: String,
    pub name: String,
}

/// The `ignite-apply-plan/0.1` envelope (`docs/slice.md` 17.3).
///
/// `actions` is frozen as `ignite-apply-actions/0.1`, the input of the
/// transition driver:
///
/// - `schema`: `"ignite-apply-actions/0.1"`.
/// - `members[*]`, one per stable member id of the base and desired models,
///   sorted by id:
///   - `id`: the member id;
///   - `kind`: `add`, `replace`, `remove`, `detach` or `unchanged`;
///   - `ownership`: `owned` (the host creates its process, listener or
///     machine), `included` (runs inside another member's process),
///     `external` (bound outside the host: observed, never started, stopped,
///     replaced or deleted) or `declared` (no instance of its own);
///     A `remove` carries the base's ownership and authority: a member that
///     was `owned` and is rebound to an external binding is one `remove`
///     entry of the host-created instance (ownership `owned`, its instance
///     and stop authority); its new external binding is only observed and
///     gets no second entry;
///   - `reason`: why the member has this kind;
///   - `authority`: `process` (`host-created-instance-only`,
///     `with-host-process` or `none`), `instance` (the observed instance or
///     null), `listener` (`owned-listener-only` or `none`),
///     `listenerInstance`, `external` (`observe-only` or `none`), `history`
///     and `inactiveArtifacts` (`retain`);
///   - `dependsOn`: the members it requires (readiness, references, uses);
///   - `dependents`: the exact affected dependent closure, the members this
///     member's change replaces transitively (empty when `unchanged`).
/// - `startOrder`: the `owned` members to start (`add`, `replace`), every
///   provider before the members that depend on it, over the same edges as
///   `dependsOn`.
/// - `stopOrder`: the `owned` members of the base to stop (`remove`,
///   `replace`), dependents before their providers, over the same edges.
/// - A change of operational identity that affects no member (a codegen
///   revision change of a system without executables) plans every member
///   `unchanged` with fresh pins; admission treats it as a no-op.
/// - `affectedDependents`: members replaced only through a provider.
/// - `replacementPolicy`: `prepare-drain-release-start-ready`.
#[derive(Clone, Debug)]
pub struct Plan {
    pub id: String,
    pub state_identity: String,
    pub base_revision: i64,
    pub source: PathBuf,
    /// The program's source root: the directory of `source`.
    pub source_root: PathBuf,
    pub assembly: Option<String>,
    pub target: crate::Target,
    pub release: bool,
    pub fuel: Option<u64>,
    pub std: Option<PathBuf>,
    pub runtime: Option<PathBuf>,
    pub model: Json,
    pub codegen_revision: String,
    pub reducer_revision: String,
    pub runtime_contract: String,
    pub operational_identity: String,
    pub implementation_identity: String,
    pub inputs: Vec<Input>,
    pub artifacts: BTreeMap<String, Artifact>,
    pub configuration: BTreeMap<String, Option<String>>,
    pub actions: Json,
    pub ownership: Json,
}

fn object(items: impl IntoIterator<Item = (&'static str, Json)>) -> Json {
    Json::Object(
        items
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}

fn text(value: impl Into<String>) -> Json {
    Json::Str(value.into())
}
fn path_json(value: &Path) -> Json {
    text(value.to_string_lossy())
}
fn optional_text(value: &Option<String>) -> Json {
    value.as_ref().map_or(Json::Null, |s| text(s.clone()))
}
fn optional_path(value: &Option<PathBuf>) -> Json {
    value.as_ref().map_or(Json::Null, |p| path_json(p))
}
fn target_name(target: crate::Target) -> &'static str {
    match target {
        crate::Target::Rust => "rust",
        crate::Target::Llvm => "llvm",
    }
}

fn plan_id(payload: &Json) -> String {
    digest::tagged(PLAN_ID, payload.compact().as_bytes())
}

/// The reducer revision of standard-library pins under `evaluator`: what
/// re-stepping a revision's `std.control` machines depends on.
fn reducer_revision(evaluator: &str, std_root: Option<&Path>, inputs: &[Input]) -> String {
    let pins: Vec<Json> = inputs
        .iter()
        .filter(|input| input.kind == InputKind::Std)
        .map(|input| {
            let relative = std_root
                .and_then(|root| input.path.strip_prefix(root).ok())
                .unwrap_or(&input.path);
            object([
                ("path", path_json(relative)),
                ("digest", text(input.digest.clone())),
            ])
        })
        .collect();
    let value = object([
        ("evaluator", text(evaluator)),
        ("controllerAbi", text(CONTROLLER_ABI)),
        ("std", Json::Array(pins)),
    ]);
    digest::tagged(REDUCER, value.compact().as_bytes())
}

impl Plan {
    fn payload(&self) -> Json {
        object([
            ("formatVersion", text(FORMAT)),
            ("controllerAbi", text(CONTROLLER_ABI)),
            ("codegenRevision", text(self.codegen_revision.clone())),
            ("reducerRevision", text(self.reducer_revision.clone())),
            ("runtimeContract", text(self.runtime_contract.clone())),
            ("stateIdentity", text(self.state_identity.clone())),
            ("baseRevision", Json::Int(self.base_revision)),
            ("source", path_json(&self.source)),
            (
                "sourceRoots",
                object([
                    ("program", path_json(&self.source_root)),
                    ("std", optional_path(&self.std)),
                ]),
            ),
            ("assembly", optional_text(&self.assembly)),
            ("target", text(target_name(self.target))),
            ("release", Json::Bool(self.release)),
            (
                "fuel",
                self.fuel.map_or(Json::Null, |fuel| Json::Int(fuel as i64)),
            ),
            ("std", optional_path(&self.std)),
            ("runtime", optional_path(&self.runtime)),
            ("model", self.model.clone()),
            ("actions", self.actions.clone()),
            ("ownership", self.ownership.clone()),
            (
                "implementationIdentity",
                text(self.implementation_identity.clone()),
            ),
            (
                "operationalIdentity",
                text(self.operational_identity.clone()),
            ),
            (
                "inputs",
                Json::Array(
                    self.inputs
                        .iter()
                        .map(|input| {
                            object([
                                ("path", path_json(&input.path)),
                                ("kind", text(input.kind.name())),
                                ("digest", text(input.digest.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "artifacts",
                Json::Object(
                    self.artifacts
                        .iter()
                        .map(|(member, artifact)| {
                            (
                                member.clone(),
                                object([
                                    ("path", path_json(&artifact.path)),
                                    ("digest", text(artifact.digest.clone())),
                                    ("name", text(artifact.name.clone())),
                                ]),
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "configuration",
                Json::Object(
                    self.configuration
                        .iter()
                        .map(|(key, value)| (key.clone(), optional_text(value)))
                        .collect(),
                ),
            ),
        ])
    }

    pub fn json(&self) -> Json {
        let Json::Object(mut value) = self.payload() else {
            unreachable!()
        };
        value.insert("id".into(), text(self.id.clone()));
        Json::Object(value)
    }

    /// Integrity is checked before receipt lookup or stale-revision admission.
    /// Codegen, reducer and runtime revisions are recorded provenance here, so
    /// a later host can read history; admission compares them with the host
    /// ([`Plan::verify_controller`]).
    pub fn parse(value: &Json) -> Result<Self, String> {
        for (key, expected) in [("formatVersion", FORMAT), ("controllerAbi", CONTROLLER_ABI)] {
            if required_text(value, key)? != expected {
                return Err(format!("unsupported {key}"));
            }
        }
        let inputs = value
            .get("inputs")
            .and_then(Json::as_array)
            .ok_or("missing inputs")?
            .iter()
            .map(|item| {
                Ok(Input {
                    path: required_path(item, "path")?,
                    kind: InputKind::parse(required_text(item, "kind")?)
                        .ok_or("unsupported input kind")?,
                    digest: required_text(item, "digest")?.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let artifacts = value
            .get("artifacts")
            .and_then(Json::as_object)
            .ok_or("missing artifacts")?
            .iter()
            .map(|(member, item)| {
                let name = required_text(item, "name")?;
                if Path::new(name).components().count() != 1
                    || name == "."
                    || name == ".."
                    || name.is_empty()
                {
                    return Err("invalid artifact executable name".into());
                }
                Ok((
                    member.clone(),
                    Artifact {
                        path: required_path(item, "path")?,
                        digest: required_text(item, "digest")?.into(),
                        name: name.into(),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let configuration = value
            .get("configuration")
            .and_then(Json::as_object)
            .ok_or("missing configuration")?
            .iter()
            .map(|(key, value)| Ok((key.clone(), nullable_text(value)?)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let roots = value.get("sourceRoots").ok_or("missing sourceRoots")?;
        let result = Plan {
            id: required_text(value, "id")?.into(),
            state_identity: required_text(value, "stateIdentity")?.into(),
            base_revision: value
                .get("baseRevision")
                .and_then(Json::as_i64)
                .filter(|v| *v >= 0)
                .ok_or("invalid baseRevision")?,
            source: required_path(value, "source")?,
            source_root: required_path(roots, "program")?,
            assembly: nullable_text(value.get("assembly").ok_or("missing assembly")?)?,
            target: match required_text(value, "target")? {
                "rust" => crate::Target::Rust,
                "llvm" => crate::Target::Llvm,
                _ => return Err("unsupported target".into()),
            },
            release: value
                .get("release")
                .and_then(Json::as_bool)
                .ok_or("invalid release")?,
            fuel: match value.get("fuel") {
                Some(Json::Null) => None,
                Some(Json::Int(value)) if *value >= 0 => Some(*value as u64),
                _ => return Err("invalid fuel".into()),
            },
            std: nullable_text(value.get("std").ok_or("missing std")?)?.map(PathBuf::from),
            runtime: nullable_text(value.get("runtime").ok_or("missing runtime")?)?
                .map(PathBuf::from),
            model: value
                .get("model")
                .filter(|v| v.as_object().is_some())
                .ok_or("invalid model")?
                .clone(),
            codegen_revision: required_text(value, "codegenRevision")?.into(),
            reducer_revision: required_text(value, "reducerRevision")?.into(),
            runtime_contract: required_text(value, "runtimeContract")?.into(),
            operational_identity: required_text(value, "operationalIdentity")?.into(),
            implementation_identity: required_text(value, "implementationIdentity")?.into(),
            inputs,
            artifacts,
            configuration,
            actions: value
                .get("actions")
                .filter(|value| value.as_object().is_some())
                .ok_or("invalid actions")?
                .clone(),
            ownership: value
                .get("ownership")
                .filter(|value| value.as_object().is_some())
                .ok_or("invalid ownership")?
                .clone(),
        };
        if result.json() != *value || plan_id(&result.payload()) != result.id {
            return Err("plan payload/id mismatch".into());
        }
        result.check_manifest()?;
        Ok(result)
    }

    /// Every input path once, the source among the program inputs, exactly
    /// one compiler, program and standard-library modules under their roots.
    fn check_manifest(&self) -> Result<(), String> {
        let paths: BTreeSet<_> = self.inputs.iter().map(|input| &input.path).collect();
        let invalid = || Err("invalid input manifest".to_string());
        if paths.len() != self.inputs.len()
            || self.source.parent() != Some(self.source_root.as_path())
            || !self
                .inputs
                .iter()
                .any(|input| input.kind == InputKind::Source && input.path == self.source)
            || self
                .inputs
                .iter()
                .filter(|input| input.kind == InputKind::Compiler)
                .count()
                != 1
        {
            return invalid();
        }
        for input in &self.inputs {
            let root = match input.kind {
                InputKind::Source => Some(self.source_root.as_path()),
                InputKind::Std => Some(self.std.as_deref().ok_or("std inputs without std root")?),
                _ => None,
            };
            if root.is_some_and(|root| !input.path.starts_with(root)) {
                return invalid();
            }
        }
        Ok(())
    }

    /// This host's reducer revision for the plan's retained standard library.
    pub fn host_reducer_revision(&self) -> String {
        reducer_revision(REDUCER_EVALUATOR, self.std.as_deref(), &self.inputs)
    }

    /// Admission: the plan was prepared under this host's codegen revision,
    /// runtime contract and reducer revision.
    pub fn verify_controller(&self) -> Result<(), String> {
        let codegen = codegen_revision();
        if self.codegen_revision != codegen {
            return Err(format!(
                "plan was prepared under codegen revision {}; this host generates {codegen}; replan",
                self.codegen_revision
            ));
        }
        let runtime = runtime_contract(self.target);
        if self.runtime_contract != runtime {
            return Err(format!(
                "plan was prepared for runtime contract {}; this host provides {runtime}; replan",
                self.runtime_contract
            ));
        }
        if self.reducer_revision != self.host_reducer_revision() {
            return Err("plan was prepared under another reducer revision; replan".into());
        }
        Ok(())
    }

    /// Attach the host's observed base and the reviewed actions. This does not
    /// dispatch effects or replace the existing Ignite control reducers.
    /// Rejects stateful members that would need migration and cycles in the
    /// required ordering.
    pub fn annotate(&mut self, previous: Option<&Plan>, ownership: &Json) -> Result<(), String> {
        if let Some(previous) = previous
            && previous.state_identity != self.state_identity
        {
            return Err("base plan targets another state".into());
        }
        let old_model = previous.map(|plan| &plan.model);
        let old = old_model
            .map(member_manifest)
            .transpose()?
            .unwrap_or_default();
        let new = member_manifest(&self.model)?;
        let implementation_changed = previous
            .is_some_and(|plan| plan.implementation_identity != self.implementation_identity);
        let reducers_changed =
            previous.is_some_and(|plan| plan.reducer_revision != self.reducer_revision);
        let ids: BTreeSet<String> = old.keys().chain(new.keys()).cloned().collect();
        let mut reasons = BTreeMap::new();
        let mut configured = BTreeSet::new();
        for id in &ids {
            // The flag: the member's own configuration changed, so its
            // consumers are affected; an implementation-only replacement keeps
            // its endpoint and configuration.
            let why = match (old.get(id), new.get(id)) {
                (None, Some(_)) => Some(("member added", true)),
                (Some(_), None) => Some(("member removed", true)),
                (Some(before), Some(after)) if before != after => {
                    Some(("operational properties or binding changed", true))
                }
                (Some(_), Some(_)) if implementation_changed && self.artifacts.contains_key(id) => {
                    Some((
                        "implementation, target, runtime contract or codegen revision changed",
                        false,
                    ))
                }
                (Some(_), Some(_)) if reducers_changed && has_machine(&self.model, id) => {
                    Some(("reducer revision changed", false))
                }
                _ => None,
            };
            if let Some((why, configuration)) = why {
                if configuration {
                    configured.insert(id.clone());
                }
                reasons.insert(id.clone(), why.to_string());
            }
        }
        let mut dependencies = configuration_dependencies(&self.model)?;
        if let Some(model) = old_model {
            for (consumer, providers) in configuration_dependencies(model)? {
                dependencies.entry(consumer).or_default().extend(providers);
            }
        }
        let mut consumers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (consumer, providers) in &dependencies {
            for provider in providers {
                consumers
                    .entry(provider.clone())
                    .or_default()
                    .insert(consumer.clone());
            }
        }
        let directly: BTreeSet<String> = reasons.keys().cloned().collect();
        let mut affected = directly.clone();
        let mut frontier: Vec<String> = configured.iter().cloned().collect();
        while let Some(provider) = frontier.pop() {
            for consumer in consumers.get(&provider).into_iter().flatten() {
                if new.contains_key(consumer) && affected.insert(consumer.clone()) {
                    reasons.insert(
                        consumer.clone(),
                        format!("affected by the configuration of {provider}"),
                    );
                    frontier.push(consumer.clone());
                }
            }
        }
        let old_class = |id: &str| old_model.map(|model| ownership_class(model, id));
        let new_class = |id: &str| ownership_class(&self.model, id);
        let mut kinds = BTreeMap::new();
        for id in &ids {
            let kind = match (
                old.contains_key(id),
                new.contains_key(id),
                affected.contains(id),
            ) {
                (false, true, _) => "add",
                (true, false, _) if old_class(id) == Some(Class::External) => "detach",
                (true, false, _) => "remove",
                (true, true, true) => match (old_class(id), new_class(id)) {
                    (Some(Class::External), Class::External) => "detach",
                    (_, Class::External) => "remove",
                    (Some(Class::External), _) => "add",
                    _ => "replace",
                },
                (true, true, false) => "unchanged",
                (false, false, _) => unreachable!(),
            };
            if kind == "replace"
                && let Some(kind_name) = [old.get(id), new.get(id)]
                    .into_iter()
                    .flatten()
                    .filter_map(|manifest| manifest.get("member")?.get("kind")?.as_str())
                    .find(|kind| stateful_kind(kind))
            {
                return Err(format!(
                    "unsupported migration: stateful member {id} ({kind_name}) would be replaced ({}); this slice admits no stateful migration",
                    reasons[id]
                ));
            }
            kinds.insert(id.clone(), kind);
        }
        let observed = stable_ownership(ownership)?;
        let entries = ids
            .iter()
            .map(|id| {
                let kind = kinds[id];
                // A `remove` acts on the base's instance, so it carries the
                // base's ownership and authority, also when the member stays
                // in the desired model rebound to an external binding.
                let class = if new.contains_key(id) && kind != "remove" {
                    new_class(id)
                } else {
                    old_class(id).expect("removed member has a base")
                };
                let instance = observed.get(id).cloned().unwrap_or(Json::Null);
                let provider = old_model.and_then(|model| listener_provider(model, id));
                let listener_instance = provider
                    .and_then(|provider| observed.get(&provider))
                    .cloned()
                    .unwrap_or(Json::Null);
                let mut closure = BTreeSet::new();
                if kind != "unchanged" {
                    let mut frontier = vec![id.clone()];
                    while let Some(provider) = frontier.pop() {
                        for consumer in consumers.get(&provider).into_iter().flatten() {
                            if consumer != id
                                && affected.contains(consumer)
                                && closure.insert(consumer.clone())
                            {
                                frontier.push(consumer.clone());
                            }
                        }
                    }
                }
                let external = class == Class::External;
                object([
                    ("id", text(id.clone())),
                    ("kind", text(kind)),
                    ("ownership", text(class.name())),
                    (
                        "reason",
                        text(
                            reasons
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| "operationally unchanged".into()),
                        ),
                    ),
                    (
                        "authority",
                        object([
                            (
                                "process",
                                text(match class {
                                    Class::Included => "with-host-process",
                                    Class::Owned if instance != Json::Null => {
                                        "host-created-instance-only"
                                    }
                                    _ => "none",
                                }),
                            ),
                            ("instance", if external { Json::Null } else { instance }),
                            (
                                "listener",
                                text(if listener_instance == Json::Null || external {
                                    "none"
                                } else {
                                    "owned-listener-only"
                                }),
                            ),
                            (
                                "listenerInstance",
                                if external {
                                    Json::Null
                                } else {
                                    listener_instance
                                },
                            ),
                            (
                                "external",
                                text(if external { "observe-only" } else { "none" }),
                            ),
                            ("history", text("retain")),
                            ("inactiveArtifacts", text("retain")),
                        ]),
                    ),
                    (
                        "dependsOn",
                        Json::Array(
                            dependencies
                                .get(id)
                                .into_iter()
                                .flatten()
                                .cloned()
                                .map(text)
                                .collect(),
                        ),
                    ),
                    (
                        "dependents",
                        Json::Array(closure.into_iter().map(text).collect()),
                    ),
                ])
            })
            .collect();
        // Cycles are rejected over every non-external member that takes an
        // action; the orders list only the members the host starts or stops.
        let start_nodes: BTreeSet<_> = ids
            .iter()
            .filter(|id| {
                matches!(kinds[*id], "add" | "replace")
                    && new.contains_key(*id)
                    && new_class(id) != Class::External
            })
            .cloned()
            .collect();
        let stop_nodes: BTreeSet<_> = ids
            .iter()
            .filter(|id| {
                matches!(kinds[*id], "remove" | "replace")
                    && old_class(id).is_some_and(|class| class != Class::External)
            })
            .cloned()
            .collect();
        let start: Vec<String> = dependency_order(&self.model, &start_nodes)?
            .into_iter()
            .filter(|id| new_class(id) == Class::Owned)
            .collect();
        let mut stop: Vec<String> = old_model
            .map(|model| dependency_order(model, &stop_nodes))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .filter(|id| old_class(id) == Some(Class::Owned))
            .collect();
        stop.reverse();
        self.actions = object([
            ("schema", text(ACTIONS_SCHEMA)),
            ("members", Json::Array(entries)),
            (
                "startOrder",
                Json::Array(start.into_iter().map(text).collect()),
            ),
            (
                "stopOrder",
                Json::Array(stop.into_iter().map(text).collect()),
            ),
            (
                "affectedDependents",
                Json::Array(affected.difference(&directly).cloned().map(text).collect()),
            ),
            (
                "replacementPolicy",
                text("prepare-drain-release-start-ready"),
            ),
        ]);
        let resource_revisions = observed
            .iter()
            .map(|(member, observation)| {
                (
                    member.clone(),
                    text(digest::tagged(
                        OBSERVED_RESOURCE,
                        format!("{member}\0{}", observation.compact()).as_bytes(),
                    )),
                )
            })
            .collect();
        self.ownership = object([
            (
                "basePlanId",
                previous.map_or(Json::Null, |plan| text(plan.id.clone())),
            ),
            ("baseRevision", Json::Int(self.base_revision)),
            ("observed", Json::Object(observed)),
            ("resourceRevisions", Json::Object(resource_revisions)),
            (
                "removal",
                object([
                    ("history", text("retain")),
                    ("inactiveArtifacts", text("retain")),
                    ("externalResources", text("never")),
                ]),
            ),
        ]);
        self.id = plan_id(&self.payload());
        Ok(())
    }

    pub fn verify_ownership(&self, observed: &Json) -> Result<(), String> {
        let expected = self
            .ownership
            .get("observed")
            .and_then(Json::as_object)
            .ok_or("missing observed ownership assumptions")?;
        if *expected != stable_ownership(observed)? {
            return Err(
                "owned process instances changed since preparation; replan required".into(),
            );
        }
        Ok(())
    }

    /// Admission pins: the payload agrees with its id, every input and
    /// artifact still has its pinned bytes, the build configuration is the
    /// same and this process is the pinned compiler.
    pub fn verify_inputs(&self) -> Result<(), String> {
        if plan_id(&self.payload()) != self.id {
            return Err("plan payload/id mismatch".into());
        }
        for input in &self.inputs {
            verify_file(&input.path, &input.digest)?;
        }
        for artifact in self.artifacts.values() {
            verify_file(&artifact.path, &artifact.digest)?;
        }
        for (key, expected) in &self.configuration {
            if std::env::var(key).ok().as_ref() != expected.as_ref() {
                return Err(format!("build configuration changed: {key}"));
            }
        }
        let compiler = std::env::current_exe().map_err(|error| error.to_string())?;
        let compiler = compiler.canonicalize().map_err(|error| error.to_string())?;
        let compiler_digest = digest::file(&compiler).map_err(|error| error.to_string())?;
        if !self.inputs.iter().any(|input| {
            input.kind == InputKind::Compiler
                && input.path == compiler
                && input.digest == compiler_digest
        }) {
            return Err("plan was prepared by a different compiler executable".into());
        }
        Ok(())
    }
}

fn stable_ownership(ownership: &Json) -> Result<BTreeMap<String, Json>, String> {
    let mut observed = ownership
        .as_object()
        .ok_or("invalid ownership snapshot")?
        .clone();
    for observation in observed.values_mut() {
        if let Json::Object(fields) = observation {
            fields.remove("ready");
        }
    }
    Ok(observed)
}

/// Who holds a member's instance (`docs/slice.md` 17.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Owned,
    Included,
    External,
    Declared,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::Owned => "owned",
            Class::Included => "included",
            Class::External => "external",
            Class::Declared => "declared",
        }
    }
}

/// Bindings whose instance the host creates; every other binding is
/// external.
const HOST_BINDINGS: [&str; 3] = ["NativeProcess", "LocalSocket", "ScheduledProbeHost"];

fn ownership_class(model: &Json, member: &str) -> Class {
    match binding(model, member) {
        Some(entry) => {
            let variant = entry
                .get("binding")
                .and_then(|binding| binding.get("variant"))
                .and_then(Json::as_str);
            if !variant.is_some_and(|variant| HOST_BINDINGS.contains(&variant)) {
                Class::External
            } else if entry
                .get("via")
                .and_then(Json::as_str)
                .is_some_and(|via| via != member)
            {
                Class::Included
            } else {
                Class::Owned
            }
        }
        None if has_machine(model, member) => Class::Owned,
        None => Class::Declared,
    }
}

/// A member whose durable contents a replacement would have to migrate.
fn stateful_kind(kind: &str) -> bool {
    kind.starts_with("Store<")
}

fn has_machine(model: &Json, member: &str) -> bool {
    model
        .get("machines")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .any(|machine| machine.get("member").and_then(Json::as_str) == Some(member))
}

fn member_manifest(model: &Json) -> Result<BTreeMap<String, Json>, String> {
    let mut members = BTreeMap::new();
    for key in ["components", "contracts", "policies", "resources"] {
        for member in model
            .get(key)
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let id = required_text(member, "id")?;
            if members
                .insert(
                    id.to_string(),
                    object([
                        ("member", member.clone()),
                        ("binding", binding(model, id).cloned().unwrap_or(Json::Null)),
                        (
                            "machines",
                            Json::Array({
                                let mut machines: Vec<_> = model
                                    .get("machines")
                                    .and_then(Json::as_array)
                                    .into_iter()
                                    .flatten()
                                    .filter(|machine| {
                                        machine.get("member").and_then(Json::as_str) == Some(id)
                                    })
                                    .cloned()
                                    .collect();
                                machines.sort_by_key(Json::compact);
                                machines
                            }),
                        ),
                    ]),
                )
                .is_some()
            {
                return Err("duplicate stable member id".into());
            }
        }
    }
    Ok(members)
}

fn binding<'a>(model: &'a Json, member: &str) -> Option<&'a Json> {
    model
        .get("assembly")?
        .get("resolution")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("member").and_then(Json::as_str) == Some(member))
}

fn listener_provider(model: &Json, member: &str) -> Option<String> {
    model
        .get("edges")?
        .as_array()?
        .iter()
        .find(|edge| {
            edge.get("kind").and_then(Json::as_str) == Some("provisioning")
                && edge.get("from").and_then(Json::as_str) == Some(member)
        })?
        .get("to")?
        .as_str()
        .map(str::to_string)
}

fn configuration_dependencies(model: &Json) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut dependencies: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in model
        .get("edges")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        if matches!(
            edge.get("kind").and_then(Json::as_str),
            Some("readiness" | "references" | "uses")
        ) {
            dependencies
                .entry(required_text(edge, "from")?.into())
                .or_default()
                .insert(required_text(edge, "to")?.into());
        }
    }
    Ok(dependencies)
}

/// `nodes` with every provider before its consumers, over the edges that
/// give `dependsOn` and `dependents` ([`configuration_dependencies`]:
/// readiness, references, uses). Signal/dataflow (`observes`), hosting and
/// provisioning edges stay out of this graph, and arbitrary user lists retain
/// their declared order. A cycle is rejected naming its members.
fn dependency_order(model: &Json, nodes: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut dependencies: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect();
    for (consumer, providers) in configuration_dependencies(model)? {
        if let Some(required) = dependencies.get_mut(&consumer) {
            required.extend(
                providers
                    .into_iter()
                    .filter(|provider| nodes.contains(provider)),
            );
        }
    }
    let mut result = Vec::new();
    while !dependencies.is_empty() {
        let Some(ready) = dependencies
            .iter()
            .find(|(_, required)| required.is_empty())
            .map(|(id, _)| id.clone())
        else {
            return Err(format!(
                "required action ordering contains a cycle: {} (each requires the next)",
                cycle(&dependencies).join(" -> ")
            ));
        };
        dependencies.remove(&ready);
        for required in dependencies.values_mut() {
            required.remove(&ready);
        }
        result.push(ready);
    }
    Ok(result)
}

/// A cycle among `dependencies`, every one of which still requires another.
fn cycle(dependencies: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let mut path: Vec<String> = vec![dependencies.keys().next().expect("non-empty").clone()];
    loop {
        let next = dependencies[path.last().unwrap()]
            .iter()
            .next()
            .expect("every remaining node requires another")
            .clone();
        if let Some(start) = path.iter().position(|id| *id == next) {
            let mut found = path.split_off(start);
            found.push(next);
            return found;
        }
        path.push(next);
    }
}

fn verify_file(path: &Path, expected: &str) -> Result<(), String> {
    let found = digest::file(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if found == expected {
        Ok(())
    } else {
        Err(format!("input content changed: {}", path.display()))
    }
}

fn required_text<'a>(value: &'a Json, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("invalid {key}"))
}
fn required_path(value: &Json, key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(required_text(value, key)?);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}
fn nullable_text(value: &Json) -> Result<Option<String>, String> {
    match value {
        Json::Null => Ok(None),
        Json::Str(value) => Ok(Some(value.clone())),
        _ => Err("expected text or null".into()),
    }
}

fn io_error(path: &Path, error: io::Error) -> crate::Error {
    crate::Error::Io {
        path: path.to_path_buf(),
        error,
    }
}
fn invalid(path: &Path, why: impl Into<String>) -> crate::Error {
    io_error(path, io::Error::new(io::ErrorKind::InvalidData, why.into()))
}
fn canonical(path: &Path) -> Result<PathBuf, crate::Error> {
    path.canonicalize().map_err(|error| io_error(path, error))
}

/// The first `name` on `PATH`, canonical.
fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .and_then(|path| path.canonicalize().ok())
}

/// The operational identity's inputs (`docs/slice.md` 17.3): canonical model
/// semantics, the implementation projection, target/profile, controller ABI,
/// runtime contract and codegen revision. Raw substrate files, the runtime
/// archive's and the compiler's bytes are admission pins, not identity.
fn identity(
    model: &Json,
    implementation: Json,
    target: crate::Target,
    release: bool,
    codegen: &str,
) -> Json {
    object([
        ("model", semantic_model(model)),
        ("implementation", implementation),
        ("target", text(target_name(target))),
        ("release", Json::Bool(release)),
        ("controllerAbi", text(CONTROLLER_ABI)),
        ("runtimeContract", text(runtime_contract(target))),
        ("codegenRevision", text(codegen)),
    ])
}

/// E142 for `ignite plan --for-apply` before the host is contacted
/// (`docs/slice.md` 21.6, row 26): a program whose plan would start a
/// member with a `Setting` without `default`. Any other failure to load or
/// model the program is left to the host's [`prepare`], which reports it
/// as before.
pub fn refuse_settings_without_default(
    file: &Path,
    options: &crate::PlanOptions,
) -> Result<(), crate::Error> {
    let Ok(loaded) = crate::load_for(file, options.std.as_deref(), crate::RUN_ROOTS) else {
        return Ok(());
    };
    let Ok(model) = loaded.model(options) else {
        return Ok(());
    };
    crate::run::refuse_settings_without_default(&loaded, &model)
}

/// Prepare before any destructive progress; all files are checked again after
/// the build so a concurrent workspace edit cannot produce mixed input pins.
pub fn prepare(
    file: &Path,
    options: &crate::PlanOptions,
    state_identity: &str,
    base_revision: i64,
    build_options: &crate::RunOptions,
    build_root: &Path,
) -> Result<Plan, crate::Error> {
    let planning_fuel = options.fuel;
    if options.fuel.is_some_and(|fuel| fuel > i64::MAX as u64) {
        return Err(io_error(
            file,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "apply planning fuel exceeds supported JSON integer range",
            ),
        ));
    }
    let codegen = codegen_revision();
    let source = canonical(file)?;
    let source_root = source
        .parent()
        .expect("canonical file has a parent")
        .to_path_buf();
    let std_root = canonical(&options.std.clone().unwrap_or_else(crate::default_std_path))?;
    let loaded = crate::load_for(&source, Some(&std_root), crate::RUN_ROOTS)?;
    let model = loaded.model(options)?;
    if model.assembly.is_none() {
        return Err(io_error(
            &source,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "apply requires a selected assembly",
            ),
        ));
    }
    crate::run::refuse_settings_without_default(&loaded, &model)?;
    let model_json = Json::parse(&model.to_json()).map_err(|why| invalid(&source, why))?;
    let mut kinds: BTreeMap<PathBuf, InputKind> = BTreeMap::new();
    let mut source_pins = BTreeMap::new();
    for module in loaded.modules().iter() {
        let path = canonical(Path::new(&module.source.path))?;
        let kind = if path.starts_with(&std_root) {
            InputKind::Std
        } else if path.starts_with(&source_root) {
            InputKind::Source
        } else {
            return Err(invalid(
                &path,
                "reached module is outside the program and standard-library roots",
            ));
        };
        let pin = digest::content(module.source.text.as_bytes());
        verify_file(&path, &pin).map_err(|why| invalid(&path, why))?;
        source_pins.insert(path.clone(), pin);
        kinds.insert(path, kind);
    }
    kinds.insert(
        canonical(&std::env::current_exe().map_err(|error| io_error(&source, error))?)?,
        InputKind::Compiler,
    );
    let (executables, _) = crate::run_members(&model);
    if build_options.target == crate::Target::Rust && !executables.is_empty() {
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let output = std::process::Command::new(&compiler)
            .args(["--print", "sysroot"])
            .output()
            .map_err(|error| io_error(Path::new(&compiler), error))?;
        if !output.status.success() {
            return Err(io_error(
                Path::new(&compiler),
                io::Error::other("cannot identify Rust toolchain"),
            ));
        }
        let sysroot = PathBuf::from(
            String::from_utf8(output.stdout)
                .map_err(|error| invalid(Path::new(&compiler), error.to_string()))?
                .trim(),
        );
        for tool in ["rustc", "cargo"] {
            kinds.insert(
                canonical(&sysroot.join("bin").join(tool))?,
                InputKind::Toolchain,
            );
        }
        let substrate = canonical(&crate::default_substrate_path())?;
        let mut paths = BTreeSet::new();
        collect_build_inputs(&substrate, &mut paths)?;
        kinds.extend(paths.into_iter().map(|path| (path, InputKind::Substrate)));
        if let Some(workspace) = substrate.parent().and_then(Path::parent) {
            for name in [
                "Cargo.toml",
                "Cargo.lock",
                "rust-toolchain",
                "rust-toolchain.toml",
            ] {
                let path = workspace.join(name);
                if path.is_file() {
                    kinds.insert(canonical(&path)?, InputKind::Workspace);
                }
            }
        }
    }
    for ancestor in source.ancestors().skip(1) {
        for name in [".cargo/config", ".cargo/config.toml"] {
            let path = ancestor.join(name);
            if path.is_file() {
                kinds.insert(canonical(&path)?, InputKind::Configuration);
            }
        }
    }
    let runtime = if build_options.target == crate::Target::Llvm {
        let runtime = crate::runtime_or_default(build_options.runtime.as_ref())?;
        if !runtime.is_file() {
            return Err(crate::llvm_build::missing_runtime(runtime));
        }
        let path = canonical(&runtime)?;
        kinds.insert(path.clone(), InputKind::Runtime);
        match on_path("clang") {
            Some(clang) => {
                kinds.insert(clang, InputKind::Clang);
            }
            None if !executables.is_empty() => {
                return Err(invalid(&source, "cannot identify clang on PATH"));
            }
            None => {}
        }
        Some(path)
    } else {
        None
    };
    let inputs = kinds
        .into_iter()
        .map(|(path, kind)| {
            source_pins
                .get(&path)
                .cloned()
                .map_or_else(|| digest::file(&path), Ok)
                .map(|digest| Input {
                    path: path.clone(),
                    kind,
                    digest,
                })
                .map_err(|error| io_error(&path, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut options = build_options.clone();
    options.assembly = options
        .assembly
        .clone()
        .or_else(|| model.assembly.as_ref().map(|assembly| assembly.id.clone()));
    options.std = Some(std_root.clone());
    options.runtime = runtime.clone();
    let built = if executables.is_empty() {
        BTreeMap::new()
    } else {
        crate::build_members(
            &source,
            &options,
            options.build.as_deref().unwrap_or(build_root),
            &executables,
        )?
    };
    let artifact_root = build_root.join("artifacts");
    let mut artifacts = BTreeMap::new();
    for (member, path) in built {
        let pin = digest::file(&path).map_err(|error| io_error(&path, error))?;
        let name = executables
            .get(&member)
            .expect("built member is declared")
            .clone();
        let retained = artifact_root
            .join(pin.trim_start_matches("sha256:"))
            .join(&name);
        let parent = retained.parent().expect("artifact parent");
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        if retained.exists() {
            verify_file(&retained, &pin).map_err(|why| invalid(&retained, why))?;
        } else {
            static NEXT_TEMPORARY: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT_TEMPORARY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            retain_artifact(&path, &retained, sequence)?;
        }
        artifacts.insert(
            member,
            Artifact {
                path: canonical(&retained)?,
                digest: pin,
                name,
            },
        );
    }
    let identity = identity(
        &model_json,
        implementation_projection(&loaded),
        options.target,
        options.release,
        &codegen,
    );
    let mut implementation_identity = identity.clone();
    if let Json::Object(fields) = &mut implementation_identity {
        fields.remove("model");
    }
    let reducer = reducer_revision(REDUCER_EVALUATOR, Some(&std_root), &inputs);
    let mut result = Plan {
        id: String::new(),
        state_identity: state_identity.into(),
        base_revision,
        source,
        source_root,
        assembly: model.assembly.as_ref().map(|assembly| assembly.id.clone()),
        target: options.target,
        release: options.release,
        fuel: planning_fuel,
        std: Some(std_root),
        runtime,
        model: model_json,
        codegen_revision: codegen,
        reducer_revision: reducer,
        runtime_contract: runtime_contract(options.target).into(),
        implementation_identity: digest::tagged(
            IMPLEMENTATION,
            implementation_identity.compact().as_bytes(),
        ),
        operational_identity: digest::tagged(OPERATIONAL, identity.compact().as_bytes()),
        inputs,
        artifacts,
        configuration: configuration(),
        actions: Json::Object(BTreeMap::new()),
        ownership: Json::Object(BTreeMap::new()),
    };
    result.id = plan_id(&result.payload());
    result
        .check_manifest()
        .map_err(|why| invalid(&result.source, why))?;
    result
        .verify_inputs()
        .map_err(|why| invalid(&result.source, why))?;
    Ok(result)
}

/// Copy the built `path` to the read-only `retained` through the temporary
/// `.<name>.<pid>.<sequence>` beside it and a rename. A temporary of that
/// name that exists already is the leftover of a crashed earlier process with
/// this pid (a container's pid 1), possibly read-only, which a copy could not
/// write over: it is removed first.
fn retain_artifact(path: &Path, retained: &Path, sequence: u64) -> Result<(), crate::Error> {
    let parent = retained.parent().expect("artifact parent");
    let name = retained
        .file_name()
        .expect("artifact name")
        .to_string_lossy();
    let temporary = parent.join(format!(".{name}.{}.{sequence}", std::process::id()));
    std::fs::remove_file(&temporary)
        .or_else(|error| match error.kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        })
        .map_err(|error| io_error(&temporary, error))?;
    std::fs::copy(path, &temporary).map_err(|error| io_error(&temporary, error))?;
    let mut permissions = std::fs::metadata(&temporary)
        .map_err(|error| io_error(&temporary, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&temporary, permissions)
        .map_err(|error| io_error(&temporary, error))?;
    std::fs::rename(&temporary, retained).map_err(|error| io_error(retained, error))?;
    Ok(())
}

fn configuration() -> BTreeMap<String, Option<String>> {
    [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTUP_TOOLCHAIN",
        "CC",
        "CFLAGS",
        "LDFLAGS",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_INCREMENTAL",
    ]
    .into_iter()
    .map(|key| (key.into(), std::env::var(key).ok()))
    .collect()
}

/// Directories of a crate that do not enter its library build.
const NOT_BUILT: [&str; 5] = ["target", ".git", "tests", "benches", "examples"];

/// The substrate's library build inputs: `.rs` files and manifests, without
/// its tests, benches and examples.
fn collect_build_inputs(root: &Path, paths: &mut BTreeSet<PathBuf>) -> Result<(), crate::Error> {
    for entry in std::fs::read_dir(root).map_err(|error| io_error(root, error))? {
        let entry = entry.map_err(|error| io_error(root, error))?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| !NOT_BUILT.iter().any(|skip| name == *skip))
            {
                collect_build_inputs(&path, paths)?;
            }
        } else if path.extension().is_some_and(|extension| extension == "rs")
            || path
                .file_name()
                .is_some_and(|name| name == "Cargo.toml" || name == "Cargo.lock")
        {
            paths.insert(canonical(&path)?);
        }
    }
    Ok(())
}

/// Only model collections whose contract defines a set are normalized.
pub fn semantic_model(model: &Json) -> Json {
    let mut result = model.clone();
    let Json::Object(top) = &mut result else {
        return result;
    };
    top.remove("sourceMap");
    for key in [
        "components",
        "contracts",
        "policies",
        "resources",
        "machines",
        "observations",
        "edges",
    ] {
        if let Some(Json::Array(values)) = top.get_mut(key) {
            values.sort_by_key(Json::compact);
        }
    }
    if let Some(Json::Object(assembly)) = top.get_mut("assembly")
        && let Some(Json::Array(values)) = assembly.get_mut("resolution")
    {
        values.sort_by_key(Json::compact);
    }
    result
}

/// Parsed structure, rather than token concatenation: newlines can change a
/// return statement's meaning even when their removal leaves identical tokens.
fn implementation_projection(loaded: &crate::Loaded) -> Json {
    Json::Object(
        loaded
            .modules()
            .iter()
            .map(|module| (module.path.into(), program_projection(module.program)))
            .collect(),
    )
}

fn program_projection(program: &Program) -> Json {
    let mut imports: Vec<_> = program
        .imports
        .iter()
        .map(|import| without_spans(&format!("{import:?}")))
        .collect();
    imports.sort();
    let declarations = program
        .decls
        .iter()
        .filter(|decl| !decl.is_test_only() && !matches!(decl, Decl::System(_) | Decl::Assembly(_)))
        .map(|decl| {
            // Associated fns are keyed `Owner.name`, so `A.run` and `B.run`
            // (or `Server.main` and `Client.main`) are both kept.
            let name = match decl {
                Decl::Fn(function) => function.qualified_name(),
                _ => decl.name().name.clone(),
            };
            (name, text(without_spans(&format!("{decl:?}"))))
        })
        .collect();
    object([
        (
            "imports",
            Json::Array(imports.into_iter().map(text).collect()),
        ),
        ("declarations", Json::Object(declarations)),
    ])
}

/// Debug structure is versioned by CODEGEN_REVISION. Strip only actual Span
/// nodes outside string literals, retaining escaped literal content verbatim.
fn without_spans(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut remaining = input;
    let mut quoted = false;
    let mut escaped = false;
    while !remaining.is_empty() {
        if !quoted
            && remaining.starts_with("Span { start: ")
            && let Some(end) = remaining.find('}')
        {
            out.push_str("Span");
            remaining = &remaining[end + 1..];
            continue;
        }
        let character = remaining.chars().next().unwrap();
        out.push(character);
        remaining = &remaining[character.len_utf8()..];
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(source: &str) -> Program {
        let file = ignite_syntax::SourceFile::new("projection.ignite", source);
        let tokens = ignite_syntax::lexer::lex(&file.text).unwrap();
        ignite_syntax::parser::parse(&tokens).unwrap()
    }

    /// Only a `TcpListener` resource has a listener provider: the studio's
    /// capability resources (`Studio.entropy` sorts before the listener)
    /// carry no listener authority in an action.
    #[test]
    fn a_capability_resource_has_no_listener_provider() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/slice17/studio_defaults.ignite");
        let loaded = crate::load_for(&file, None, crate::RUN_ROOTS).expect("the studio loads");
        let model = loaded
            .model(&crate::PlanOptions::default())
            .expect("the studio models");
        let model = Json::parse(&model.to_json()).expect("the model is JSON");
        assert_eq!(
            listener_provider(&model, "Studio.ingress").as_deref(),
            Some("Studio.server")
        );
        for capability in [
            "Studio.entropy",
            "Studio.files",
            "Studio.tool",
            "Studio.tools",
        ] {
            assert_eq!(listener_provider(&model, capability), None, "{capability}");
        }
    }

    #[test]
    fn projection_ignores_locations_and_declaration_order_but_keeps_statement_structure() {
        let first = parsed("fn one() -> Int { return 1 }\nfn two() -> Int { return 2 }\n");
        let reformatted = parsed(
            "// shifted\nfn two() -> Int {\n return 2\n}\n\nfn one() -> Int {\n return 1\n}\n",
        );
        assert_eq!(program_projection(&first), program_projection(&reformatted));
        let changed = parsed("fn one() -> Int { return 3 }\nfn two() -> Int { return 2 }\n");
        assert_ne!(program_projection(&first), program_projection(&changed));
    }

    #[test]
    fn literal_span_text_is_not_erased() {
        let source = r#"Fn { span: Span { start: 1, end: 2 }, text: "Span { start: 3, end: 4 }" }"#;
        assert_eq!(
            without_spans(source),
            r#"Fn { span: Span, text: "Span { start: 3, end: 4 }" }"#
        );
    }

    #[test]
    fn semantic_model_keeps_arbitrary_list_order() {
        let first=Json::parse(r#"{"sourceMap":{"x":"a:1"},"components":[{"id":"b"},{"id":"a"}],"properties":{"xs":[1,2]}}"#).unwrap();
        let reordered=Json::parse(r#"{"sourceMap":{"x":"a:2"},"components":[{"id":"a"},{"id":"b"}],"properties":{"xs":[1,2]}}"#).unwrap();
        let changed =
            Json::parse(r#"{"components":[{"id":"a"},{"id":"b"}],"properties":{"xs":[2,1]}}"#)
                .unwrap();
        assert_eq!(semantic_model(&first), semantic_model(&reordered));
        assert_ne!(semantic_model(&first), semantic_model(&changed));
    }

    #[test]
    fn envelope_rejects_changed_payload_and_unknown_fields_before_admission() {
        let mut plan = action_plan(Json::Object(BTreeMap::new()));
        plan.base_revision = 0;
        plan.fuel = Some(5_000_000);
        plan.id = plan_id(&plan.payload());
        let original = plan.json();
        assert_eq!(Plan::parse(&original).unwrap().fuel, Some(5_000_000));
        assert_eq!(Plan::parse(&original).unwrap().json(), original);
        let Json::Object(mut changed) = original.clone() else {
            unreachable!()
        };
        changed.insert("baseRevision".into(), Json::Int(1));
        assert!(
            Plan::parse(&Json::Object(changed))
                .unwrap_err()
                .contains("payload/id")
        );
        let Json::Object(mut unknown) = original.clone() else {
            unreachable!()
        };
        unknown.insert("executable".into(), text("/arbitrary"));
        assert!(
            Plan::parse(&Json::Object(unknown))
                .unwrap_err()
                .contains("payload/id")
        );
        let Json::Object(mut version) = original else {
            unreachable!()
        };
        version.insert("formatVersion".into(), text("ignite-apply-plan/0.2"));
        assert!(
            Plan::parse(&Json::Object(version))
                .unwrap_err()
                .contains("unsupported formatVersion")
        );
    }

    #[test]
    fn envelope_carries_every_saved_plan_field() {
        let mut plan = action_plan(Json::Object(BTreeMap::new()));
        plan.annotate(None, &Json::Object(BTreeMap::new())).unwrap();
        let value = plan.json();
        for key in [
            "formatVersion",
            "stateIdentity",
            "baseRevision",
            "assembly",
            "model",
            "actions",
            "sourceRoots",
            "inputs",
            "target",
            "release",
            "artifacts",
            "controllerAbi",
            "codegenRevision",
            "reducerRevision",
            "runtimeContract",
            "operationalIdentity",
            "implementationIdentity",
            "configuration",
            "id",
        ] {
            assert!(value.get(key).is_some(), "envelope lacks {key}");
        }
        for key in ["observed", "resourceRevisions", "removal", "baseRevision"] {
            assert!(plan.ownership.get(key).is_some(), "ownership lacks {key}");
        }
        for key in ["schema", "members", "startOrder", "stopOrder"] {
            assert!(plan.actions.get(key).is_some(), "actions lack {key}");
        }
        let Json::Object(mut without_id) = value.clone() else {
            unreachable!()
        };
        without_id.remove("id");
        assert_eq!(
            plan.id,
            digest::tagged(PLAN_ID, Json::Object(without_id).compact().as_bytes())
        );
        let roots = value.get("sourceRoots").unwrap();
        assert_eq!(roots.get("program").and_then(Json::as_str), Some("/src"));
        assert_eq!(roots.get("std").and_then(Json::as_str), Some("/std"));
        let kinds: BTreeSet<_> = plan.inputs.iter().map(|input| input.kind).collect();
        assert!(kinds.contains(&InputKind::Compiler) && kinds.contains(&InputKind::Std));
    }

    #[test]
    fn manifest_rejects_inputs_outside_their_roots_and_a_missing_compiler() {
        let mut plan = action_plan(Json::Object(BTreeMap::new()));
        plan.inputs
            .retain(|input| input.kind != InputKind::Compiler);
        plan.id = plan_id(&plan.payload());
        assert!(Plan::parse(&plan.json()).unwrap_err().contains("manifest"));
        let mut plan = action_plan(Json::Object(BTreeMap::new()));
        plan.inputs.push(Input {
            path: PathBuf::from("/elsewhere/std/extra.ignite"),
            kind: InputKind::Std,
            digest: digest::content(b"x"),
        });
        plan.id = plan_id(&plan.payload());
        assert!(Plan::parse(&plan.json()).unwrap_err().contains("manifest"));
    }

    #[test]
    fn operational_identity_excludes_raw_substrate_runtime_and_compiler_bytes() {
        let model = Json::parse(r#"{"components":[{"id":"a"}]}"#).unwrap();
        let rust = identity(&model, Json::Null, crate::Target::Rust, false, "c1");
        let Json::Object(fields) = &rust else {
            unreachable!()
        };
        assert_eq!(
            fields.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "codegenRevision",
                "controllerAbi",
                "implementation",
                "model",
                "release",
                "runtimeContract",
                "target"
            ]
        );
        assert_ne!(
            rust,
            identity(&model, Json::Null, crate::Target::Rust, false, "c2"),
            "codegen revision is operational"
        );
        assert_ne!(
            identity(&model, Json::Null, crate::Target::Llvm, false, "c1").get("runtimeContract"),
            rust.get("runtimeContract"),
            "the runtime contract is operational and per target"
        );
        let mut paths = BTreeSet::new();
        collect_build_inputs(&crate::default_substrate_path(), &mut paths).unwrap();
        assert!(paths.iter().any(|path| path.ends_with("src/lib.rs")));
        assert!(
            paths.iter().all(|path| !path
                .components()
                .any(|component| component.as_os_str() == "tests")),
            "substrate tests are no build input: {paths:?}"
        );
    }

    #[test]
    fn an_override_replaces_the_codegen_revision() {
        assert_eq!(
            codegen_revision_or(Some("ignite-codegen/test-2".into())),
            "ignite-codegen/test-2"
        );
        assert_eq!(codegen_revision_or(Some("".into())), CODEGEN_REVISION);
        assert_eq!(codegen_revision_or(None), CODEGEN_REVISION);
    }

    #[test]
    fn controller_mismatch_is_rejected_at_admission() {
        let mut plan = action_plan(Json::Object(BTreeMap::new()));
        plan.codegen_revision = CODEGEN_REVISION.into();
        plan.reducer_revision = plan.host_reducer_revision();
        plan.verify_controller().unwrap();
        let mut other = plan.clone();
        other.codegen_revision = "ignite-codegen/older".into();
        assert!(other.verify_controller().unwrap_err().contains("codegen"));
        let mut other = plan.clone();
        other.reducer_revision = digest::tagged(REDUCER, b"older");
        assert!(other.verify_controller().unwrap_err().contains("reducer"));
        let mut other = plan;
        other.runtime_contract = "ignite-io/0.0".into();
        assert!(other.verify_controller().unwrap_err().contains("runtime"));
    }

    fn action_plan(model: Json) -> Plan {
        let mut plan = Plan {
            id: "previous-plan".into(),
            state_identity: "state-1".into(),
            base_revision: 1,
            source: PathBuf::from("/src/source.ignite"),
            source_root: PathBuf::from("/src"),
            assembly: Some("local".into()),
            target: crate::Target::Rust,
            release: false,
            fuel: None,
            std: Some(PathBuf::from("/std")),
            runtime: None,
            model,
            codegen_revision: CODEGEN_REVISION.into(),
            reducer_revision: String::new(),
            runtime_contract: runtime_contract(crate::Target::Rust).into(),
            operational_identity: "operational".into(),
            implementation_identity: "implementation".into(),
            inputs: vec![
                Input {
                    path: PathBuf::from("/src/source.ignite"),
                    kind: InputKind::Source,
                    digest: digest::content(b"source"),
                },
                Input {
                    path: PathBuf::from("/std/control.ignite"),
                    kind: InputKind::Std,
                    digest: digest::content(b"reducers"),
                },
                Input {
                    path: PathBuf::from("/bin/ignite"),
                    kind: InputKind::Compiler,
                    digest: digest::content(b"compiler"),
                },
            ],
            artifacts: BTreeMap::new(),
            configuration: BTreeMap::new(),
            actions: Json::Object(BTreeMap::new()),
            ownership: Json::Object(BTreeMap::new()),
        };
        plan.reducer_revision = plan.host_reducer_revision();
        plan
    }

    fn artifact(name: &str) -> Artifact {
        Artifact {
            path: PathBuf::from(format!("/artifacts/{name}")),
            digest: digest::content(name.as_bytes()),
            name: name.into(),
        }
    }

    #[test]
    fn annotated_actions_include_dependent_closure_and_preserve_foreign_resources() {
        let old=action_plan(Json::parse(r#"{
            "components":[{"id":"provider","value":1},{"id":"consumer"},{"id":"unrelated"},{"id":"external"}],
            "edges":[{"from":"consumer","kind":"readiness","to":"provider"}],
            "assembly":{"resolution":[
                {"member":"provider","via":"provider","binding":{"variant":"NativeProcess"}},
                {"member":"consumer","via":"consumer","binding":{"variant":"ScheduledProbeHost"}},
                {"member":"unrelated","via":"unrelated","binding":{"variant":"NativeProcess"}},
                {"member":"external","binding":{"variant":"External"}}
            ]}
        }"#).unwrap());
        let mut new = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"provider","value":2},{"id":"consumer"},{"id":"unrelated"}],
            "edges":[{"from":"consumer","kind":"readiness","to":"provider"}],
            "assembly":{"resolution":[
                {"member":"provider","via":"provider","binding":{"variant":"NativeProcess"}},
                {"member":"consumer","via":"consumer","binding":{"variant":"ScheduledProbeHost"}},
                {"member":"unrelated","via":"unrelated","binding":{"variant":"NativeProcess"}}
            ]}
        }"#,
            )
            .unwrap(),
        );
        let ownership =
            Json::parse(r#"{"provider":{"pid":123,"operationId":"accepted","ready":true}}"#)
                .unwrap();
        new.annotate(Some(&old), &ownership).unwrap();
        new.verify_ownership(
            &Json::parse(r#"{"provider":{"pid":123,"operationId":"accepted","ready":false}}"#)
                .unwrap(),
        )
        .unwrap();
        assert!(
            new.verify_ownership(
                &Json::parse(r#"{"provider":{"pid":124,"operationId":"accepted","ready":true}}"#)
                    .unwrap()
            )
            .unwrap_err()
            .contains("instances changed")
        );
        assert_eq!(kind(&new, "provider"), "replace");
        assert_eq!(kind(&new, "consumer"), "replace");
        assert_eq!(kind(&new, "unrelated"), "unchanged");
        assert_eq!(kind(&new, "external"), "detach");
        assert_eq!(
            entry(&new, "provider").get("dependents"),
            Some(&Json::Array(vec![text("consumer")]))
        );
        assert_eq!(
            entry(&new, "external")
                .get("authority")
                .unwrap()
                .get("process")
                .and_then(Json::as_str),
            Some("none")
        );
        assert_eq!(
            new.actions.get("schema").and_then(Json::as_str),
            Some(ACTIONS_SCHEMA)
        );
        let start = new.actions.get("startOrder").unwrap().as_array().unwrap();
        assert_eq!(start, &[text("provider"), text("consumer")]);
        let stop = new.actions.get("stopOrder").unwrap().as_array().unwrap();
        assert!(
            stop.iter().position(|id| id.as_str() == Some("consumer"))
                < stop.iter().position(|id| id.as_str() == Some("provider"))
        );
        assert!(
            new.ownership
                .get("observed")
                .unwrap()
                .get("provider")
                .unwrap()
                .get("ready")
                .is_none()
        );
        assert!(
            new.ownership
                .get("resourceRevisions")
                .unwrap()
                .get("provider")
                .and_then(Json::as_str)
                .is_some_and(|revision| revision.starts_with("sha256:"))
        );
        assert_eq!(Plan::parse(&new.json()).unwrap().json(), new.json());
    }

    fn entry<'a>(plan: &'a Plan, id: &str) -> &'a Json {
        plan.actions
            .get("members")
            .and_then(Json::as_array)
            .expect("action members")
            .iter()
            .find(|entry| entry.get("id").and_then(Json::as_str) == Some(id))
            .unwrap_or_else(|| panic!("no action entry for {id}: {:?}", plan.actions))
    }

    fn kind<'a>(plan: &'a Plan, id: &str) -> &'a str {
        entry(plan, id)
            .get("kind")
            .and_then(Json::as_str)
            .expect("action kind")
    }

    const HOSTED: &str = r#"{
        "components":[{"id":"server","port":1},{"id":"client","target":"a"}],
        "contracts":[{"id":"reachable"}],
        "machines":[{"id":"reachable.evaluator","member":"reachable"},{"id":"server.supervisor","member":"server"}],
        "edges":[{"from":"reachable","kind":"readiness","to":"server"}],
        "assembly":{"resolution":[
            {"member":"server","via":"server","binding":{"variant":"NativeProcess"}},
            {"member":"client","via":"client","binding":{"variant":"WorkstationExecutable"}}
        ]}
    }"#;

    #[test]
    fn changed_external_member_is_detach_never_replace() {
        let old = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"server","port":1},{"id":"client","target":"a"}],
            "assembly":{"resolution":[
                {"member":"server","via":"server","binding":{"variant":"NativeProcess"}},
                {"member":"client","via":"client","binding":{"variant":"WorkstationExecutable"}}
            ]}
        }"#,
            )
            .unwrap(),
        );
        let mut new = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"server","port":1},{"id":"client","target":"b"}],
            "assembly":{"resolution":[
                {"member":"server","via":"server","binding":{"variant":"NativeProcess"}},
                {"member":"client","via":"client","binding":{"variant":"WorkstationExecutable"}}
            ]}
        }"#,
            )
            .unwrap(),
        );
        let ownership = Json::parse(r#"{"client":{"pid":7,"instance":"9"}}"#).unwrap();
        new.annotate(Some(&old), &ownership).unwrap();
        assert_eq!(kind(&new, "client"), "detach");
        assert_eq!(kind(&new, "server"), "unchanged");
        let authority = entry(&new, "client").get("authority").unwrap();
        assert_eq!(
            authority.get("process").and_then(Json::as_str),
            Some("none")
        );
        assert_eq!(authority.get("instance"), Some(&Json::Null));
        assert_eq!(
            authority.get("external").and_then(Json::as_str),
            Some("observe-only")
        );
        for order in ["startOrder", "stopOrder"] {
            assert!(
                !new.actions
                    .get(order)
                    .and_then(Json::as_array)
                    .unwrap()
                    .contains(&text("client")),
                "{order} holds an external member"
            );
        }
    }

    #[test]
    fn changed_codegen_revision_with_an_unchanged_model_replaces_owned_executables() {
        let model = Json::parse(HOSTED).unwrap();
        let mut old = action_plan(model.clone());
        old.artifacts.insert("server".into(), artifact("terminald"));
        old.artifacts.insert("client".into(), artifact("tn"));
        let mut new = old.clone();
        new.codegen_revision = "ignite-codegen/next".into();
        new.implementation_identity = "implementation-under-next".into();
        new.operational_identity = "operational-under-next".into();
        new.artifacts
            .insert("server".into(), artifact("terminald-next"));
        new.annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .unwrap();
        assert_eq!(kind(&new, "server"), "replace");
        assert!(
            entry(&new, "server")
                .get("reason")
                .and_then(Json::as_str)
                .unwrap()
                .contains("codegen")
        );
        assert_eq!(kind(&new, "client"), "detach");
        assert_eq!(
            kind(&new, "reachable"),
            "unchanged",
            "an implementation-only replacement keeps its consumers' configuration"
        );
        assert_eq!(
            new.actions.get("startOrder"),
            Some(&Json::Array(vec![text("server")]))
        );
    }

    #[test]
    fn changed_reducer_revision_replaces_machine_members() {
        let model = Json::parse(HOSTED).unwrap();
        let old = action_plan(model);
        let mut new = old.clone();
        new.reducer_revision = digest::tagged(REDUCER, b"next");
        new.implementation_identity = "implementation-next".into();
        new.operational_identity = "operational-next".into();
        new.annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .unwrap();
        assert_eq!(kind(&new, "reachable"), "replace");
        assert_eq!(kind(&new, "server"), "replace");
        assert_eq!(kind(&new, "client"), "unchanged");
    }

    #[test]
    fn replacing_a_stateful_member_is_an_unsupported_migration() {
        let old = action_plan(
            Json::parse(r#"{"resources":[{"id":"todos","kind":"Store<examples.todo.Todo>","properties":{}}]}"#)
                .unwrap(),
        );
        let mut new = action_plan(
            Json::parse(r#"{"resources":[{"id":"todos","kind":"Store<examples.todo.Task>","properties":{}}]}"#)
                .unwrap(),
        );
        let error = new
            .annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .unwrap_err();
        assert!(
            error.contains("unsupported migration") && error.contains("todos"),
            "{error}"
        );
        let mut unchanged = old.clone();
        unchanged
            .annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .unwrap();
        assert_eq!(kind(&unchanged, "todos"), "unchanged");
    }

    #[test]
    fn readiness_cycles_reject_but_signal_cycles_do_not_impose_start_order() {
        let model=Json::parse(r#"{"components":[{"id":"a"},{"id":"b"}],"edges":[{"from":"a","kind":"readiness","to":"b"},{"from":"b","kind":"readiness","to":"a"}]}"#).unwrap();
        let mut plan = action_plan(model);
        let error = plan
            .annotate(None, &Json::Object(BTreeMap::new()))
            .unwrap_err();
        assert!(
            error.contains("cycle: a -> b -> a") || error.contains("cycle: b -> a -> b"),
            "{error}"
        );
        plan.model=Json::parse(r#"{"components":[{"id":"a"},{"id":"b"}],"edges":[{"from":"a","kind":"observes","to":"b"},{"from":"b","kind":"observes","to":"a"}]}"#).unwrap();
        plan.annotate(None, &Json::Object(BTreeMap::new())).unwrap();
    }

    #[test]
    fn adversary_projection_keys_associated_fns_by_qualified_name() {
        let first = parsed("fn A.run() -> Int = 1\nfn B.run() -> Int = 2\n");
        let changed = parsed("fn A.run() -> Int = 3\nfn B.run() -> Int = 2\n");
        assert_ne!(
            program_projection(&first),
            program_projection(&changed),
            "a changed body of A.run is invisible to the implementation projection"
        );
    }

    #[test]
    fn adversary_owned_member_rebound_external_keeps_stop_authority() {
        let old = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"server","port":1}],
            "assembly":{"resolution":[
                {"member":"server","via":"server","binding":{"variant":"NativeProcess"}}
            ]}
        }"#,
            )
            .unwrap(),
        );
        let mut new = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"server","port":1}],
            "assembly":{"resolution":[
                {"member":"server","via":"server","binding":{"variant":"WorkstationExecutable"}}
            ]}
        }"#,
            )
            .unwrap(),
        );
        let ownership = Json::parse(r#"{"server":{"pid":7,"instance":"9"}}"#).unwrap();
        new.annotate(Some(&old), &ownership).unwrap();
        let stop = new
            .actions
            .get("stopOrder")
            .and_then(Json::as_array)
            .unwrap();
        assert!(
            stop.contains(&text("server")),
            "the host-created process must be stopped"
        );
        let server = entry(&new, "server");
        let authority = server.get("authority").unwrap();
        assert_eq!(
            (
                server.get("ownership").and_then(Json::as_str),
                authority.get("process").and_then(Json::as_str),
                authority.get("instance").cloned(),
            ),
            (
                Some("owned"),
                Some("host-created-instance-only"),
                Some(Json::parse(r#"{"pid":7,"instance":"9"}"#).unwrap())
            ),
            "stopOrder holds a member whose entry grants no authority over the instance to stop: {server:?}"
        );
    }

    const USES: &str = r#"{
        "components":[{"id":"api","listener":"listener"}],
        "resources":[{"id":"listener","port":PORT}],
        "edges":[{"from":"api","kind":"uses","to":"listener"}],
        "assembly":{"resolution":[
            {"member":"api","via":"api","binding":{"variant":"NativeProcess"}},
            {"member":"listener","via":"listener","binding":{"variant":"LocalSocket"}}
        ]}
    }"#;

    fn position(order: &[Json], id: &str) -> Option<usize> {
        order.iter().position(|value| value.as_str() == Some(id))
    }

    #[test]
    fn adversary_start_order_respects_depends_on() {
        let mut plan = action_plan(Json::parse(&USES.replace("PORT", "1")).unwrap());
        plan.annotate(None, &Json::Object(BTreeMap::new())).unwrap();
        assert_eq!(
            entry(&plan, "api").get("dependsOn"),
            Some(&Json::Array(vec![text("listener")]))
        );
        let start = plan
            .actions
            .get("startOrder")
            .and_then(Json::as_array)
            .unwrap();
        assert!(
            position(start, "listener") < position(start, "api"),
            "api dependsOn listener but starts first: {start:?}"
        );
    }

    #[test]
    fn adversary_stop_order_stops_dependents_before_providers() {
        let old = action_plan(Json::parse(&USES.replace("PORT", "1")).unwrap());
        let mut new = action_plan(Json::parse(&USES.replace("PORT", "2")).unwrap());
        new.annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .unwrap();
        assert_eq!(kind(&new, "listener"), "replace");
        assert_eq!(kind(&new, "api"), "replace");
        assert_eq!(
            entry(&new, "listener").get("dependents"),
            Some(&Json::Array(vec![text("api")]))
        );
        let stop = new
            .actions
            .get("stopOrder")
            .and_then(Json::as_array)
            .unwrap();
        assert!(
            position(stop, "api") < position(stop, "listener"),
            "listener's dependent api is stopped after its provider: {stop:?}"
        );
    }

    #[test]
    fn adversary_codegen_change_without_affected_members_plans_unchanged() {
        let model = Json::parse(
            r#"{
            "resources":[{"id":"first","port":1},{"id":"second","port":2}],
            "assembly":{"resolution":[
                {"member":"first","via":"first","binding":{"variant":"LocalSocket"}},
                {"member":"second","via":"second","binding":{"variant":"LocalSocket"}}
            ]}
        }"#,
        )
        .unwrap();
        let old = action_plan(model);
        let mut new = old.clone();
        new.codegen_revision = "ignite-codegen/next".into();
        new.implementation_identity = "implementation-under-next".into();
        new.operational_identity = "operational-under-next".into();
        new.annotate(Some(&old), &Json::Object(BTreeMap::new()))
            .expect(
                "a compiler upgrade must not make an unchanged listener-only state unplannable",
            );
        assert_eq!(kind(&new, "first"), "unchanged");
        assert_eq!(kind(&new, "second"), "unchanged");
    }

    #[test]
    fn adversary_self_and_indirect_readiness_cycles_are_rejected() {
        let mut plan = action_plan(
            Json::parse(
                r#"{"components":[{"id":"a"}],"edges":[{"from":"a","kind":"readiness","to":"a"}]}"#,
            )
            .unwrap(),
        );
        let error = plan
            .annotate(None, &Json::Object(BTreeMap::new()))
            .unwrap_err();
        assert!(error.contains("cycle: a -> a"), "{error}");
        plan.model = Json::parse(r#"{"components":[{"id":"a"},{"id":"b"},{"id":"c"}],"edges":[{"from":"a","kind":"readiness","to":"b"},{"from":"b","kind":"readiness","to":"c"},{"from":"c","kind":"readiness","to":"a"}]}"#).unwrap();
        let error = plan
            .annotate(None, &Json::Object(BTreeMap::new()))
            .unwrap_err();
        assert!(error.contains("cycle: a -> b -> c -> a"), "{error}");
    }

    /// A preparation killed between `set_readonly` and the rename leaves a
    /// read-only temporary; a later process with the same pid (a container's
    /// pid 1) and the same sequence must still retain the artifact.
    #[test]
    fn a_read_only_leftover_temporary_does_not_block_retaining_an_artifact() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/plan-leftover")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let store = dir.join("store");
        std::fs::create_dir_all(&store).unwrap();
        let built = dir.join("built");
        std::fs::write(&built, b"artifact").unwrap();
        let leftover = store.join(format!(".terminald.{}.7", std::process::id()));
        std::fs::write(&leftover, b"torn").unwrap();
        let mut permissions = std::fs::metadata(&leftover).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&leftover, permissions).unwrap();
        let retained = store.join("terminald");

        let result = retain_artifact(&built, &retained, 7);
        let content = std::fs::read(&retained).ok();
        let read_only = std::fs::metadata(&retained)
            .map(|metadata| metadata.permissions().readonly())
            .ok();
        let left = leftover.exists();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            result.is_ok(),
            "a crash leftover blocks retaining: {result:?}"
        );
        assert_eq!(content.as_deref(), Some(&b"artifact"[..]));
        assert_eq!(read_only, Some(true));
        assert!(!left, "the leftover is gone");
    }
}
