//! Source-bound local plans; ordinary model JSON keeps its existing format.

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
pub const CODEGEN_REVISION: &str = "ignite-codegen/0.1.0-1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub path: PathBuf,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub path: PathBuf,
    pub digest: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub id: String,
    pub state_identity: String,
    pub base_revision: i64,
    pub source: PathBuf,
    pub assembly: Option<String>,
    pub target: crate::Target,
    pub release: bool,
    pub fuel: Option<u64>,
    pub std: Option<PathBuf>,
    pub runtime: Option<PathBuf>,
    pub model: Json,
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

fn plan_id(payload: &Json) -> String {
    digest::bytes(format!("{FORMAT}\0{}", payload.compact()).as_bytes())
}

impl Plan {
    fn payload(&self) -> Json {
        object([
            ("formatVersion", text(FORMAT)),
            ("controllerAbi", text(CONTROLLER_ABI)),
            ("codegenRevision", text(CODEGEN_REVISION)),
            ("stateIdentity", text(self.state_identity.clone())),
            ("baseRevision", Json::Int(self.base_revision)),
            ("source", path_json(&self.source)),
            ("assembly", optional_text(&self.assembly)),
            (
                "target",
                text(match self.target {
                    crate::Target::Rust => "rust",
                    crate::Target::Llvm => "llvm",
                }),
            ),
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
    pub fn parse(value: &Json) -> Result<Self, String> {
        for (key, expected) in [
            ("formatVersion", FORMAT),
            ("controllerAbi", CONTROLLER_ABI),
            ("codegenRevision", CODEGEN_REVISION),
        ] {
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
        let result = Plan {
            id: required_text(value, "id")?.into(),
            state_identity: required_text(value, "stateIdentity")?.into(),
            base_revision: value
                .get("baseRevision")
                .and_then(Json::as_i64)
                .filter(|v| *v >= 0)
                .ok_or("invalid baseRevision")?,
            source: required_path(value, "source")?,
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
        let paths: BTreeSet<_> = result.inputs.iter().map(|input| &input.path).collect();
        if paths.len() != result.inputs.len() || !paths.contains(&result.source) {
            return Err("invalid input manifest".into());
        }
        Ok(result)
    }

    /// Attach the host's observed base and static review manifest. This does
    /// not dispatch effects or replace the existing Ignite control reducers.
    pub fn annotate(&mut self, previous: Option<&Plan>, ownership: &Json) -> Result<(), String> {
        if let Some(previous) = previous
            && previous.state_identity != self.state_identity
        {
            return Err("base plan targets another state".into());
        }
        let old = previous
            .map(|plan| member_manifest(&plan.model))
            .transpose()?
            .unwrap_or_default();
        let new = member_manifest(&self.model)?;
        let implementation_changed = previous
            .is_some_and(|plan| plan.implementation_identity != self.implementation_identity);
        let mut changed = BTreeSet::new();
        let mut reasons = BTreeMap::new();
        for id in old.keys().chain(new.keys()) {
            let why = match (old.get(id), new.get(id)) {
                (None, Some(_)) => Some("member added"),
                (Some(_), None) => Some("member removed"),
                (Some(before), Some(after)) if before != after => {
                    Some("operational properties or binding changed")
                }
                (Some(_), Some(_))
                    if implementation_changed
                        && (self.artifacts.contains_key(id) || has_machine(&self.model, id)) =>
                {
                    Some("implementation, target or runtime semantics changed")
                }
                _ => None,
            };
            if let Some(why) = why {
                changed.insert(id.clone());
                reasons.insert(id.clone(), why.to_string());
            }
        }
        if previous.is_some_and(|plan| plan.operational_identity != self.operational_identity)
            && changed.is_empty()
        {
            return Err(
                "unsupported operational change outside the member action projection".into(),
            );
        }
        let dependencies = configuration_dependencies(&self.model)?;
        let directly_changed = changed.clone();
        loop {
            let mut added = false;
            for (consumer, providers) in &dependencies {
                if !changed.contains(consumer)
                    && providers.iter().any(|provider| changed.contains(provider))
                {
                    changed.insert(consumer.clone());
                    reasons.insert(
                        consumer.clone(),
                        "affected provider configuration or implementation".into(),
                    );
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        let observed = stable_ownership(ownership)?;
        let old_model = previous.map(|plan| &plan.model);
        let ids: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
        let entries = ids
            .iter()
            .map(|id| {
                let action = match (
                    old.contains_key(id),
                    new.contains_key(id),
                    changed.contains(id),
                ) {
                    (false, true, _) => "add",
                    (true, false, _)
                        if old_model.is_some_and(|model| external_binding(model, id)) =>
                    {
                        "detach"
                    }
                    (true, false, _) => "remove",
                    (true, true, true) => "replace",
                    _ => "unchanged",
                };
                let instance = observed.get(id).cloned().unwrap_or(Json::Null);
                let provider = old_model.and_then(|model| listener_provider(model, id));
                let listener_instance = provider
                    .and_then(|provider| observed.get(&provider))
                    .cloned()
                    .unwrap_or(Json::Null);
                object([
                    ("member", text(id.clone())),
                    ("action", text(action)),
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
                        "authority",
                        object([
                            (
                                "process",
                                text(if instance == Json::Null {
                                    "none"
                                } else {
                                    "host-created-instance-only"
                                }),
                            ),
                            ("instance", instance),
                            (
                                "listener",
                                text(if listener_instance == Json::Null {
                                    "none"
                                } else {
                                    "owned-listener-only"
                                }),
                            ),
                            ("listenerInstance", listener_instance),
                            ("history", text("retain")),
                            ("inactiveArtifacts", text("retain")),
                        ]),
                    ),
                ])
            })
            .collect();
        let start_nodes: BTreeSet<_> = new
            .keys()
            .filter(|id| changed.contains(*id))
            .cloned()
            .collect();
        let stop_nodes: BTreeSet<_> = old
            .keys()
            .filter(|id| changed.contains(*id))
            .cloned()
            .collect();
        let start = readiness_order(&self.model, &start_nodes)?;
        let mut stop = old_model
            .map(|model| readiness_order(model, &stop_nodes))
            .transpose()?
            .unwrap_or_default();
        stop.reverse();
        self.actions = object([
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
                Json::Array(
                    changed
                        .difference(&directly_changed)
                        .cloned()
                        .map(text)
                        .collect(),
                ),
            ),
            (
                "replacementPolicy",
                text("prepare-drain-release-start-ready"),
            ),
        ]);
        self.ownership = object([
            (
                "basePlanId",
                previous.map_or(Json::Null, |plan| text(plan.id.clone())),
            ),
            ("baseRevision", Json::Int(self.base_revision)),
            ("observed", Json::Object(observed)),
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
        if !self
            .inputs
            .iter()
            .any(|input| input.path == compiler && input.digest == compiler_digest)
        {
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

fn external_binding(model: &Json, member: &str) -> bool {
    let variant = binding(model, member)
        .and_then(|entry| entry.get("binding"))
        .and_then(|binding| binding.get("variant"))
        .and_then(Json::as_str);
    variant.is_some_and(|variant| {
        !matches!(
            variant,
            "NativeProcess" | "LocalSocket" | "ScheduledProbeHost"
        )
    })
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

/// Only readiness edges impose startup ordering. Signal/dataflow edges stay
/// out of this graph, and arbitrary user lists retain their declared order.
fn readiness_order(model: &Json, nodes: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut dependencies: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect();
    for edge in model
        .get("edges")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        if edge.get("kind").and_then(Json::as_str) != Some("readiness") {
            continue;
        }
        let consumer = required_text(edge, "from")?;
        let provider = required_text(edge, "to")?;
        if nodes.contains(provider)
            && let Some(required) = dependencies.get_mut(consumer)
        {
            required.insert(provider.into());
        }
    }
    let mut result = Vec::new();
    while !dependencies.is_empty() {
        let ready = dependencies
            .iter()
            .find(|(_, required)| required.is_empty())
            .map(|(id, _)| id.clone())
            .ok_or("required readiness action ordering contains a cycle")?;
        dependencies.remove(&ready);
        for required in dependencies.values_mut() {
            required.remove(&ready);
        }
        result.push(ready);
    }
    Ok(result)
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
fn canonical(path: &Path) -> Result<PathBuf, crate::Error> {
    path.canonicalize().map_err(|error| io_error(path, error))
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
    let source = canonical(file)?;
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
    let model_json = Json::parse(&model.to_json())
        .map_err(|why| io_error(&source, io::Error::new(io::ErrorKind::InvalidData, why)))?;
    let mut input_paths = BTreeSet::new();
    let mut source_pins = BTreeMap::new();
    for module in loaded.modules().iter() {
        let path = canonical(Path::new(&module.source.path))?;
        let pin = digest::bytes(module.source.text.as_bytes());
        verify_file(&path, &pin)
            .map_err(|why| io_error(&path, io::Error::new(io::ErrorKind::InvalidData, why)))?;
        source_pins.insert(path.clone(), pin);
        input_paths.insert(path);
    }
    input_paths.insert(canonical(
        &std::env::current_exe().map_err(|error| io_error(&source, error))?,
    )?);
    if build_options.target == crate::Target::Rust && !crate::run_members(&model).0.is_empty() {
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
                .map_err(|error| {
                    io_error(
                        Path::new(&compiler),
                        io::Error::new(io::ErrorKind::InvalidData, error),
                    )
                })?
                .trim(),
        );
        for tool in ["rustc", "cargo"] {
            let path = sysroot.join("bin").join(tool);
            input_paths.insert(canonical(&path)?);
        }
    }
    let substrate = canonical(&crate::default_substrate_path())?;
    collect_build_inputs(&substrate, &mut input_paths)?;
    if let Some(workspace) = substrate.parent().and_then(Path::parent) {
        for name in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain",
            "rust-toolchain.toml",
        ] {
            let path = workspace.join(name);
            if path.is_file() {
                input_paths.insert(canonical(&path)?);
            }
        }
    }
    for ancestor in source.ancestors().skip(1) {
        for name in [".cargo/config", ".cargo/config.toml"] {
            let path = ancestor.join(name);
            if path.is_file() {
                input_paths.insert(canonical(&path)?);
            }
        }
    }
    let runtime = if build_options.target == crate::Target::Llvm {
        let path = canonical(
            &build_options
                .runtime
                .clone()
                .unwrap_or_else(crate::default_runtime_path),
        )?;
        input_paths.insert(path.clone());
        Some(path)
    } else {
        None
    };
    let inputs = input_paths
        .into_iter()
        .map(|path| {
            source_pins
                .get(&path)
                .cloned()
                .map_or_else(|| digest::file(&path), Ok)
                .map(|digest| Input {
                    path: path.clone(),
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
    let (executables, _) = crate::run_members(&model);
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
            verify_file(&retained, &pin).map_err(|why| {
                io_error(&retained, io::Error::new(io::ErrorKind::InvalidData, why))
            })?;
        } else {
            static NEXT_TEMPORARY: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT_TEMPORARY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let temporary = parent.join(format!(".{name}.{}.{sequence}", std::process::id()));
            std::fs::copy(&path, &temporary).map_err(|error| io_error(&temporary, error))?;
            let mut permissions = std::fs::metadata(&temporary)
                .map_err(|error| io_error(&temporary, error))?
                .permissions();
            permissions.set_readonly(true);
            std::fs::set_permissions(&temporary, permissions)
                .map_err(|error| io_error(&temporary, error))?;
            std::fs::rename(&temporary, &retained).map_err(|error| io_error(&retained, error))?;
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
    let projection = implementation_projection(&loaded);
    let semantic_model = semantic_model(&model_json);
    let identity = object([
        ("model", semantic_model),
        ("implementation", projection),
        (
            "target",
            text(match options.target {
                crate::Target::Rust => "rust",
                crate::Target::Llvm => "llvm",
            }),
        ),
        ("release", Json::Bool(options.release)),
        ("controllerAbi", text(CONTROLLER_ABI)),
        ("codegenRevision", text(CODEGEN_REVISION)),
        (
            "substrate",
            Json::Array(
                inputs
                    .iter()
                    .filter(|input| input.path.starts_with(&substrate))
                    .map(|input| {
                        object([
                            (
                                "path",
                                text(
                                    input
                                        .path
                                        .strip_prefix(&substrate)
                                        .unwrap()
                                        .to_string_lossy(),
                                ),
                            ),
                            ("digest", text(input.digest.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "runtime",
            runtime
                .as_ref()
                .and_then(|path| inputs.iter().find(|input| input.path == *path))
                .map_or(Json::Null, |input| text(input.digest.clone())),
        ),
    ]);
    let mut implementation_identity = identity.clone();
    if let Json::Object(fields) = &mut implementation_identity {
        fields.remove("model");
    }
    let mut result = Plan {
        id: String::new(),
        state_identity: state_identity.into(),
        base_revision,
        source,
        assembly: model.assembly.as_ref().map(|assembly| assembly.id.clone()),
        target: options.target,
        release: options.release,
        fuel: planning_fuel,
        std: Some(std_root),
        runtime,
        model: model_json,
        implementation_identity: digest::bytes(
            format!(
                "ignite-implementation/0.1\0{}",
                implementation_identity.compact()
            )
            .as_bytes(),
        ),
        operational_identity: digest::bytes(
            format!("ignite-operational/0.1\0{}", identity.compact()).as_bytes(),
        ),
        inputs,
        artifacts,
        configuration: configuration(),
        actions: Json::Object(BTreeMap::new()),
        ownership: Json::Object(BTreeMap::new()),
    };
    result.id = plan_id(&result.payload());
    result.verify_inputs().map_err(|why| {
        io_error(
            &result.source,
            io::Error::new(io::ErrorKind::InvalidData, why),
        )
    })?;
    Ok(result)
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

fn collect_build_inputs(root: &Path, paths: &mut BTreeSet<PathBuf>) -> Result<(), crate::Error> {
    for entry in std::fs::read_dir(root).map_err(|error| io_error(root, error))? {
        let entry = entry.map_err(|error| io_error(root, error))?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name != "target" && name != ".git")
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
            (
                decl.name().name.clone(),
                text(without_spans(&format!("{decl:?}"))),
            )
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
        let mut plan = Plan {
            id: String::new(),
            state_identity: "state-1".into(),
            base_revision: 0,
            source: PathBuf::from("/source.ignite"),
            assembly: Some("local".into()),
            target: crate::Target::Rust,
            release: false,
            fuel: Some(5_000_000),
            std: None,
            runtime: None,
            model: Json::Object(BTreeMap::new()),
            operational_identity: digest::bytes(b"operation"),
            implementation_identity: digest::bytes(b"implementation"),
            inputs: vec![Input {
                path: PathBuf::from("/source.ignite"),
                digest: digest::bytes(b"source"),
            }],
            artifacts: BTreeMap::new(),
            configuration: BTreeMap::new(),
            actions: Json::Object(BTreeMap::new()),
            ownership: Json::Object(BTreeMap::new()),
        };
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
        let Json::Object(mut unknown) = original else {
            unreachable!()
        };
        unknown.insert("executable".into(), text("/arbitrary"));
        assert!(
            Plan::parse(&Json::Object(unknown))
                .unwrap_err()
                .contains("payload/id")
        );
    }

    fn action_plan(model: Json) -> Plan {
        Plan {
            id: "previous-plan".into(),
            state_identity: "state-1".into(),
            base_revision: 1,
            source: PathBuf::from("/source.ignite"),
            assembly: Some("local".into()),
            target: crate::Target::Rust,
            release: false,
            fuel: None,
            std: None,
            runtime: None,
            model,
            operational_identity: "operational".into(),
            implementation_identity: "implementation".into(),
            inputs: vec![Input {
                path: PathBuf::from("/source.ignite"),
                digest: digest::bytes(b"source"),
            }],
            artifacts: BTreeMap::new(),
            configuration: BTreeMap::new(),
            actions: Json::Object(BTreeMap::new()),
            ownership: Json::Object(BTreeMap::new()),
        }
    }

    #[test]
    fn annotated_actions_include_dependent_closure_and_preserve_foreign_resources() {
        let old=action_plan(Json::parse(r#"{
            "components":[{"id":"provider","value":1},{"id":"consumer"},{"id":"unrelated"},{"id":"external"}],
            "edges":[{"from":"consumer","kind":"readiness","to":"provider"}],
            "assembly":{"resolution":[{"member":"external","binding":{"variant":"External"}}]}
        }"#).unwrap());
        let mut new = action_plan(
            Json::parse(
                r#"{
            "components":[{"id":"provider","value":2},{"id":"consumer"},{"id":"unrelated"}],
            "edges":[{"from":"consumer","kind":"readiness","to":"provider"}],
            "assembly":{"resolution":[]}
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
        let members = new.actions.get("members").unwrap().as_array().unwrap();
        let action = |name: &str| {
            members
                .iter()
                .find(|entry| entry.get("member").and_then(Json::as_str) == Some(name))
                .unwrap()
        };
        assert_eq!(
            action("provider").get("action").and_then(Json::as_str),
            Some("replace")
        );
        assert_eq!(
            action("consumer").get("action").and_then(Json::as_str),
            Some("replace")
        );
        assert_eq!(
            action("unrelated").get("action").and_then(Json::as_str),
            Some("unchanged")
        );
        assert_eq!(
            action("external").get("action").and_then(Json::as_str),
            Some("detach")
        );
        assert_eq!(
            action("external")
                .get("authority")
                .unwrap()
                .get("process")
                .and_then(Json::as_str),
            Some("none")
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
        assert_eq!(Plan::parse(&new.json()).unwrap().json(), new.json());
    }

    #[test]
    fn readiness_cycles_reject_but_signal_cycles_do_not_impose_start_order() {
        let model=Json::parse(r#"{"components":[{"id":"a"},{"id":"b"}],"edges":[{"from":"a","kind":"readiness","to":"b"},{"from":"b","kind":"readiness","to":"a"}]}"#).unwrap();
        let mut plan = action_plan(model);
        assert!(
            plan.annotate(None, &Json::Object(BTreeMap::new()))
                .unwrap_err()
                .contains("cycle")
        );
        plan.model=Json::parse(r#"{"components":[{"id":"a"},{"id":"b"}],"edges":[{"from":"a","kind":"observes","to":"b"},{"from":"b","kind":"observes","to":"a"}]}"#).unwrap();
        plan.annotate(None, &Json::Object(BTreeMap::new())).unwrap();
    }
}
