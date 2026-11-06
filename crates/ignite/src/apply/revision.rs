use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::run::{
    control::{Adapters, Observation},
    engine::Core,
    journal::Journal,
    live::{Carried, Config, Ended, Live, RecordSink},
};

use super::plan::Plan;

/// Every table needed by a revision is loaded and checked before admission.
/// Moving this owned context to the worker preserves the checked input graph.
pub(super) struct Context {
    pub plan: Plan,
    pub loaded: crate::Loaded,
    pub model: ignite_eval::Model,
}

impl Context {
    pub fn load(plan: Plan) -> Result<Self, crate::Error> {
        let loaded = crate::load_for(&plan.source, plan.std.as_deref(), crate::RUN_ROOTS)?;
        let model = loaded.model(&crate::PlanOptions {
            assembly: plan.assembly.clone(),
            std: plan.std.clone(),
            fuel: plan.fuel,
        })?;
        let mut observed = ignite_eval::Json::parse(&model.to_json()).expect("model JSON");
        let mut expected = plan.model.clone();
        if let ignite_eval::Json::Object(fields) = &mut observed {
            fields.remove("sourceMap");
        }
        if let ignite_eval::Json::Object(fields) = &mut expected {
            fields.remove("sourceMap");
        }
        if observed != expected {
            return Err(crate::Error::Io {
                path: plan.source.clone(),
                error: std::io::Error::other("retained revision model differs from admitted model"),
            });
        }
        Ok(Context {
            plan,
            loaded,
            model,
        })
    }
}

/// Verify a historical revision under its retained source and reducer tables.
/// Replayed effects are compared, never dispatched.
pub(super) fn verify(context: Context, state: &Path) -> Result<(), crate::Error> {
    let Context {
        plan: _,
        loaded,
        model,
    } = context;
    let modules = loaded.modules();
    let types = modules
        .get(ignite_eval::CONTROL_MODULE)
        .expect("RUN_ROOTS includes control")
        .types;
    let mut evaluator = ignite_eval::Evaluator::new(&modules, ignite_eval::Fuel::default());
    let mut core = Core::new(&mut evaluator, &model);
    let journal = Journal::open(state, false).map_err(|error| crate::Error::Io {
        path: state.to_path_buf(),
        error: std::io::Error::other(error.to_string()),
    })?;
    if let Some(header) = journal.records.first() {
        let model_json = ignite_eval::Json::parse(&model.to_json()).expect("model JSON");
        crate::check_header(&loaded, header, &model_json)?;
        crate::run::replay::replay(&mut core, types, &journal.records[1..])
            .map_err(|error| crate::run_diagnostic(&loaded, "E127", error.to_string()))?;
    }
    Ok(())
}

/// How a revision's run starts under a transition driver: the instances a
/// previous revision's run handed over, and the members that start only on
/// the driver's `start <member>`. `None` for a resumed revision, which
/// starts every machine; its run accepts the driver's control lines too.
pub(super) struct Launch {
    pub carried: Option<Carried>,
    pub deferred: BTreeSet<String>,
}

/// The revision context owns every parsed/type table. The live driver borrows
/// it only inside this worker's stack; a new revision cannot replace borrowed
/// ASTs or silently change the reducer behind an existing journal.
/// With carried instances the run keeps the carried channel and `tx` and
/// `rx` are unused.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    context: Context,
    state: &Path,
    tx: mpsc::Sender<Observation>,
    rx: mpsc::Receiver<Observation>,
    sink: RecordSink,
    started: Box<dyn FnOnce() -> std::io::Result<()> + Send>,
    builder: crate::run::live::Builder,
    launch: Option<Launch>,
) -> Result<Ended, crate::Error> {
    let Context {
        plan,
        loaded,
        model,
    } = context;
    let modules = loaded.modules();
    let types = modules
        .get(ignite_eval::CONTROL_MODULE)
        .expect("RUN_ROOTS includes control")
        .types;
    let mut evaluator = ignite_eval::Evaluator::new(&modules, ignite_eval::Fuel::default());
    let mut core = Core::new(&mut evaluator, &model);
    let mut journal = Journal::open(state, false).map_err(|error| crate::Error::Io {
        path: state.to_path_buf(),
        error: std::io::Error::other(error.to_string()),
    })?;
    let epoch = crate::wall_now();
    let model_json = ignite_eval::Json::parse(&model.to_json()).expect("model JSON");
    if journal.records.is_empty() {
        journal
            .append(&crate::journal_header(&model_json, epoch), true)
            .map_err(crate::io_error(state))?;
    } else {
        crate::check_header(&loaded, &journal.records[0], &model_json)?;
        crate::run::replay::replay(&mut core, types, &journal.records[1..])
            .map_err(|error| crate::run_diagnostic(&loaded, "E127", error.to_string()))?;
    }
    core.set_next_seq(journal.next_seq);
    let built: BTreeMap<String, PathBuf> = plan
        .artifacts
        .iter()
        .map(|(member, artifact)| (member.clone(), artifact.path.clone()))
        .collect();
    let (carried, deferred) = match launch {
        Some(Launch { carried, deferred }) => (carried, deferred),
        None => (None, BTreeSet::new()),
    };
    let generation = carried
        .as_ref()
        .and_then(|carried| carried.instances.values().map(|i| i.generation).max())
        .unwrap_or(1)
        .max(1);
    let bin = state.join("bin").join(generation.to_string());
    crate::run::live::copy_into(&bin, &built).map_err(crate::io_error(&bin))?;
    let (executables, declared) = crate::run_members(&model);
    let (adapters, tx, rx, adopted, pinned) = match carried {
        Some(carried) => {
            for observation in carried.pending {
                let _ = carried.tx.send(observation);
            }
            (
                carried.adapters,
                carried.tx,
                carried.rx,
                carried.instances,
                carried.pinned,
            )
        }
        None => (
            Adapters::start(tx.clone()).map_err(crate::io_error(state))?,
            tx,
            rx,
            BTreeMap::new(),
            BTreeMap::new(),
        ),
    };
    let config = Config {
        state: state.to_path_buf(),
        manual: false,
        epoch,
        settle: Duration::from_secs(10),
        json: false,
        ports: BTreeMap::new(),
        declared,
        executables,
        first_listeners: crate::run::live::first_listeners(loaded.types()),
        generation,
        start_at: 0,
        builder,
    };
    Live::new(core, &model, config, journal, adapters, tx, rx)
        .with_record_sink(sink)
        .with_started_hook(started)
        .with_transitions(deferred, adopted, pinned)
        .run_host()
        .map_err(crate::io_error(state))
}
