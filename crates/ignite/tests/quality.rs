//! The quality ratchet (`docs/quality.md`, story `quality-ratchet`): the
//! metric list and its bars in `quality/baseline.json`, the per-merge
//! checks of the instrumented metrics, the history `task quality` appends
//! to `quality/history.jsonl`, and the pages generated from them
//! (`quality/trend.md`, the metric table of `docs/quality.md`, the book's
//! `site/book/src/quality.md`).
//!
//! A metric is measured by one source: `static` (counts over the tree,
//! [`static_metrics_hold_the_bar`]), `differential` (the runner of
//! `differential.rs`), `conformance` ([`conformance_claims_hold_the_bar`],
//! the claim lines of `ignite conformance`) or `perf` (`ignite-perf
//! quality`, timing and sizes, measured by `task quality` only). Each value
//! is held against its bar as the UI ratchet (`site::ux`) does: a value
//! worse than the bar beyond the metric's tolerance, a bar the run did not
//! measure, a measured value without a bar and an improvement not written
//! into the baseline are refused; and, against `main`'s baseline, a bar
//! made worse or a tolerance loosened without a `decision`.
//! `IGNITE_BLESS=1` writes improvements and new values into the baseline;
//! it never writes a worse one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_eval::Json;

use crate::differential;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace")
}

/// One measured value: metric id, target (`rust`, `llvm`, `eval` or
/// `all`, optionally followed by `/<part>`) and value.
pub type Measured = Vec<(String, String, i64)>;

/// A tolerance: an absolute count, or a percentage of the bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tolerance {
    Count(i64),
    Percent(i64),
}

impl Tolerance {
    fn parse(value: &Json) -> Result<Tolerance, String> {
        match value {
            Json::Int(n) if *n >= 0 => Ok(Tolerance::Count(*n)),
            Json::Str(text) => text
                .strip_suffix('%')
                .and_then(|n| n.parse::<i64>().ok())
                .filter(|n| *n >= 0)
                .map(Tolerance::Percent)
                .ok_or_else(|| format!("tolerance {text:?} is neither a count nor `N%`")),
            other => Err(format!(
                "tolerance {} is neither a count nor `N%`",
                other.compact()
            )),
        }
    }

    fn json(self) -> Json {
        match self {
            Tolerance::Count(n) => Json::Int(n),
            Tolerance::Percent(p) => Json::Str(format!("{p}%")),
        }
    }

    /// How far `value` may move from `bar` either way without counting.
    fn slack(self, bar: i64) -> i64 {
        match self {
            Tolerance::Count(n) => n,
            Tolerance::Percent(p) => bar.abs() * p / 100,
        }
    }

    fn looser_than(self, other: Tolerance) -> bool {
        match (self, other) {
            (Tolerance::Count(a), Tolerance::Count(b)) => a > b,
            (Tolerance::Percent(a), Tolerance::Percent(b)) => a > b,
            _ => true,
        }
    }
}

/// Whether a lower or a higher value is better.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Lower,
    Higher,
}

impl Direction {
    /// `value` is worse than `bar` beyond `tolerance`.
    fn worse(self, bar: i64, value: i64, tolerance: Tolerance) -> bool {
        let slack = tolerance.slack(bar);
        match self {
            Direction::Lower => value > bar + slack,
            Direction::Higher => value < bar - slack,
        }
    }

    /// `value` is better than `bar` beyond `tolerance` (any better value
    /// for a count tolerance).
    fn better(self, bar: i64, value: i64, tolerance: Tolerance) -> bool {
        let slack = match tolerance {
            Tolerance::Count(_) => 0,
            Tolerance::Percent(_) => tolerance.slack(bar),
        };
        match self {
            Direction::Lower => value < bar - slack,
            Direction::Higher => value > bar + slack,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Direction::Lower => "lower",
            Direction::Higher => "higher",
        }
    }
}

/// One bar: the value a target must hold, and the decision that let it
/// get worse than `main`'s.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub value: i64,
    pub decision: Option<String>,
}

/// One metric of `docs/quality.md`.
#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub area: String,
    pub name: String,
    /// `gate`, `ratchet` or `hill-climb`.
    pub kind: String,
    pub direction: Direction,
    pub unit: String,
    pub instrument: String,
    /// `static`, `differential`, `conformance` or `perf`; `None` for a
    /// metric without a number.
    pub source: Option<String>,
    pub tolerance: Tolerance,
    /// The decision that loosened `tolerance` against `main`'s; a later
    /// loosening needs a decision of its own.
    pub tolerance_decision: Option<String>,
    /// The hill-climbing target, where one is set.
    pub goal: Option<String>,
    /// Why the metric has no number, when it has none.
    pub unmeasured: Option<String>,
    pub bars: BTreeMap<String, Bar>,
}

/// A difference the differential runner found and the baseline accepts
/// until the story that fixes it lands.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub consumer: String,
    pub story: String,
    pub what: String,
    /// A difference that depends on the machine's load (a timing assertion):
    /// the unit may differ or agree in a run; it is not counted in 1.1 and
    /// not refused as gone.
    pub intermittent: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Baseline {
    pub about: String,
    pub areas: Vec<String>,
    pub metrics: BTreeMap<String, Metric>,
    pub known_differences: BTreeMap<String, Known>,
    pub eval_cannot_run: BTreeMap<String, String>,
    /// The units of `eval_cannot_run` listed as over the evaluator's time
    /// limit, which a merge run does not evaluate, each with the decision
    /// that listed it.
    pub eval_slow: BTreeMap<String, String>,
}

fn object<'a>(value: &'a Json, what: &str) -> Result<&'a BTreeMap<String, Json>, String> {
    match value {
        Json::Object(entries) => Ok(entries),
        other => Err(format!("{what} is not an object: {}", other.compact())),
    }
}

fn text(fields: &BTreeMap<String, Json>, key: &str, what: &str) -> Result<String, String> {
    fields
        .get(key)
        .and_then(Json::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{what}: no text `{key}`"))
}

fn optional(fields: &BTreeMap<String, Json>, key: &str) -> Option<String> {
    fields.get(key).and_then(Json::as_str).map(str::to_string)
}

const KINDS: [&str; 3] = ["gate", "ratchet", "hill-climb"];
const SOURCES: [&str; 4] = ["static", "differential", "conformance", "perf"];
const CONSUMERS: [&str; 4] = ["rust", "llvm", "eval", "all"];

pub fn read_baseline(json: &Json) -> Result<Baseline, String> {
    let root = object(json, "the baseline")?;
    for key in [
        "about",
        "areas",
        "metrics",
        "known_differences",
        "eval_cannot_run",
    ] {
        if !root.contains_key(key) {
            return Err(format!("the baseline has no `{key}`"));
        }
    }
    let areas: Vec<String> = root["areas"]
        .as_array()
        .ok_or("`areas` is no array")?
        .iter()
        .map(|a| a.as_str().map(str::to_string).ok_or("an area is no text"))
        .collect::<Result<_, _>>()?;
    let mut metrics = BTreeMap::new();
    for (id, entry) in object(&root["metrics"], "`metrics`")? {
        let fields = object(entry, id)?;
        let kind = text(fields, "kind", id)?;
        if !KINDS.contains(&kind.as_str()) {
            return Err(format!("{id}: kind {kind:?} is not one of {KINDS:?}"));
        }
        let direction = match text(fields, "direction", id)?.as_str() {
            "lower" => Direction::Lower,
            "higher" => Direction::Higher,
            other => return Err(format!("{id}: direction {other:?} is not lower or higher")),
        };
        let area = text(fields, "area", id)?;
        if !areas.contains(&area) {
            return Err(format!("{id}: area {area:?} is not in `areas`"));
        }
        let source = optional(fields, "source");
        if let Some(source) = &source
            && !SOURCES.contains(&source.as_str())
        {
            return Err(format!("{id}: source {source:?} is not one of {SOURCES:?}"));
        }
        let unmeasured = optional(fields, "unmeasured");
        if source.is_some() == unmeasured.is_some() {
            return Err(format!(
                "{id}: a metric has a `source` or says why it is `unmeasured`, one of the two"
            ));
        }
        let mut bars = BTreeMap::new();
        if let Some(entries) = fields.get("bars") {
            for (target, bar) in object(entries, &format!("{id} bars"))? {
                let consumer = target.split('/').next().unwrap_or("");
                if !CONSUMERS.contains(&consumer) {
                    return Err(format!(
                        "{id} {target}: the target is not one of {CONSUMERS:?}"
                    ));
                }
                let bar_fields = object(bar, &format!("{id} {target}"))?;
                let value = bar_fields
                    .get("value")
                    .and_then(Json::as_i64)
                    .ok_or_else(|| format!("{id} {target}: no integer `value`"))?;
                bars.insert(
                    target.clone(),
                    Bar {
                        value,
                        decision: optional(bar_fields, "decision"),
                    },
                );
            }
        }
        metrics.insert(
            id.clone(),
            Metric {
                area,
                name: text(fields, "name", id)?,
                kind,
                direction,
                unit: text(fields, "unit", id)?,
                instrument: text(fields, "instrument", id)?,
                source,
                tolerance: Tolerance::parse(
                    fields
                        .get("tolerance")
                        .ok_or_else(|| format!("{id}: no `tolerance`"))?,
                )
                .map_err(|e| format!("{id}: {e}"))?,
                tolerance_decision: optional(fields, "tolerance_decision"),
                goal: optional(fields, "goal"),
                unmeasured,
                bars,
            },
        );
    }
    let mut known_differences = BTreeMap::new();
    for (unit, entry) in object(&root["known_differences"], "`known_differences`")? {
        let fields = object(entry, unit)?;
        let known = Known {
            consumer: text(fields, "consumer", unit)?,
            story: text(fields, "story", unit)?,
            what: text(fields, "what", unit)?,
            intermittent: fields
                .get("intermittent")
                .and_then(Json::as_bool)
                .unwrap_or(false),
        };
        if !known.story.starts_with("story:") {
            return Err(format!(
                "{unit}: story {:?} is no `story:<id>`",
                known.story
            ));
        }
        known_differences.insert(unit.clone(), known);
    }
    let eval_cannot_run = object(&root["eval_cannot_run"], "`eval_cannot_run`")?
        .iter()
        .map(|(unit, reason)| {
            reason
                .as_str()
                .map(|r| (unit.clone(), r.to_string()))
                .ok_or_else(|| format!("{unit}: the reason is no text"))
        })
        .collect::<Result<_, _>>()?;
    let eval_slow = match root.get("eval_slow") {
        None => BTreeMap::new(),
        Some(slow) => object(slow, "`eval_slow`")?
            .iter()
            .map(|(unit, decision)| {
                decision
                    .as_str()
                    .map(|d| (unit.clone(), d.to_string()))
                    .ok_or_else(|| format!("{unit}: the decision is no text"))
            })
            .collect::<Result<_, _>>()?,
    };
    Ok(Baseline {
        about: text(root, "about", "the baseline")?,
        areas,
        metrics,
        known_differences,
        eval_cannot_run,
        eval_slow,
    })
}

pub fn write_baseline(baseline: &Baseline) -> String {
    let s = |t: &str| Json::Str(t.to_string());
    let metrics = baseline
        .metrics
        .iter()
        .map(|(id, m)| {
            let mut fields = BTreeMap::new();
            fields.insert("area".to_string(), s(&m.area));
            fields.insert("name".to_string(), s(&m.name));
            fields.insert("kind".to_string(), s(&m.kind));
            fields.insert("direction".to_string(), s(m.direction.word()));
            fields.insert("unit".to_string(), s(&m.unit));
            fields.insert("instrument".to_string(), s(&m.instrument));
            fields.insert("tolerance".to_string(), m.tolerance.json());
            if let Some(decision) = &m.tolerance_decision {
                fields.insert("tolerance_decision".to_string(), s(decision));
            }
            if let Some(source) = &m.source {
                fields.insert("source".to_string(), s(source));
            }
            if let Some(goal) = &m.goal {
                fields.insert("goal".to_string(), s(goal));
            }
            if let Some(reason) = &m.unmeasured {
                fields.insert("unmeasured".to_string(), s(reason));
            }
            if !m.bars.is_empty() {
                let bars = m
                    .bars
                    .iter()
                    .map(|(target, bar)| {
                        let mut b = BTreeMap::new();
                        b.insert("value".to_string(), Json::Int(bar.value));
                        if let Some(decision) = &bar.decision {
                            b.insert("decision".to_string(), s(decision));
                        }
                        (target.clone(), Json::Object(b))
                    })
                    .collect();
                fields.insert("bars".to_string(), Json::Object(bars));
            }
            (id.clone(), Json::Object(fields))
        })
        .collect();
    let known = baseline
        .known_differences
        .iter()
        .map(|(unit, k)| {
            let mut fields = BTreeMap::from([
                ("consumer".to_string(), s(&k.consumer)),
                ("story".to_string(), s(&k.story)),
                ("what".to_string(), s(&k.what)),
            ]);
            if k.intermittent {
                fields.insert("intermittent".to_string(), Json::Bool(true));
            }
            (unit.clone(), Json::Object(fields))
        })
        .collect();
    let cannot = baseline
        .eval_cannot_run
        .iter()
        .map(|(unit, reason)| (unit.clone(), s(reason)))
        .collect();
    let root = BTreeMap::from([
        ("about".to_string(), s(&baseline.about)),
        (
            "areas".to_string(),
            Json::Array(baseline.areas.iter().map(|a| s(a)).collect()),
        ),
        ("metrics".to_string(), Json::Object(metrics)),
        ("known_differences".to_string(), Json::Object(known)),
        ("eval_cannot_run".to_string(), Json::Object(cannot)),
        (
            "eval_slow".to_string(),
            Json::Object(
                baseline
                    .eval_slow
                    .iter()
                    .map(|(unit, decision)| (unit.clone(), s(decision)))
                    .collect(),
            ),
        ),
    ]);
    let mut out = String::new();
    Json::Object(root).write(&mut out, 0);
    out.push('\n');
    out
}

fn parse(text: &str, what: &str) -> Json {
    Json::parse(text).unwrap_or_else(|e| panic!("{what}: {e}"))
}

fn baseline_path() -> PathBuf {
    workspace().join("quality/baseline.json")
}

pub fn load_baseline() -> Baseline {
    let text = std::fs::read_to_string(baseline_path()).expect("quality/baseline.json");
    read_baseline(&parse(&text, "quality/baseline.json")).unwrap()
}

fn save_baseline(baseline: &Baseline) {
    std::fs::write(baseline_path(), write_baseline(baseline)).unwrap();
}

/// `main`'s `quality/baseline.json`, when `main` has one.
fn main_baseline() -> Option<Baseline> {
    let output = Command::new("git")
        .args(["show", "main:quality/baseline.json"])
        .current_dir(workspace())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(read_baseline(&parse(&text, "main:quality/baseline.json")).expect("main's baseline"))
}

fn blessing() -> bool {
    std::env::var_os("IGNITE_BLESS").is_some()
}

/// Every reason the values `source` measured do not hold against
/// `baseline`, and `baseline`'s bars against `main`'s.
pub fn check(
    baseline: &Baseline,
    source: &str,
    measured: &Measured,
    main: Option<&Baseline>,
) -> Vec<String> {
    let mut errors = Vec::new();
    let values: BTreeMap<(&str, &str), i64> = measured
        .iter()
        .map(|(id, target, value)| ((id.as_str(), target.as_str()), *value))
        .collect();
    for (id, metric) in &baseline.metrics {
        if metric.source.as_deref() != Some(source) || is_timing(id) {
            continue;
        }
        let climbing = metric.kind == "hill-climb";
        for (target, bar) in &metric.bars {
            match values.get(&(id.as_str(), target.as_str())) {
                None => errors.push(format!("{id} {target}: missing (bar {})", bar.value)),
                Some(_) if climbing => {}
                Some(&value) if metric.direction.worse(bar.value, value, metric.tolerance) => {
                    errors.push(format!(
                        "{id} {target}: worse, {value} against the bar {} ({}, {:?})",
                        bar.value,
                        metric.direction.word(),
                        metric.tolerance
                    ))
                }
                Some(&value) if metric.direction.better(bar.value, value, metric.tolerance) => {
                    errors.push(format!(
                        "{id} {target}: improved to {value} from {}; write it into quality/baseline.json (IGNITE_BLESS=1)",
                        bar.value
                    ))
                }
                Some(_) => {}
            }
        }
    }
    for ((id, target), value) in &values {
        match baseline.metrics.get(*id) {
            None => errors.push(format!("{id}: measured but not in quality/baseline.json")),
            Some(_) if is_timing(id) => {}
            Some(metric) if metric.source.as_deref() != Some(source) => errors.push(format!(
                "{id}: measured by `{source}`, the baseline names {:?}",
                metric.source
            )),
            Some(metric) if !metric.bars.contains_key(*target) => errors.push(format!(
                "{id} {target}: {value} is not in quality/baseline.json (IGNITE_BLESS=1)"
            )),
            Some(_) => {}
        }
    }
    if let Some(main) = main {
        errors.extend(against_main(baseline, source, main));
    }
    errors
}

/// Whether `decision` is a decision taken in this change: present, not
/// blank, and not the one `main` already holds in its place.
fn new_decision(decision: Option<&str>, on_main: Option<&str>) -> bool {
    decision.is_some_and(|d| !d.trim().is_empty() && Some(d) != on_main)
}

/// The refusals of `baseline` against `main`'s for the metrics `source`
/// measures (on either side): a bar made worse or a tolerance loosened
/// without a decision of its own, and a metric whose source, unit or
/// direction changed (a different measure is a new metric id).
fn against_main(baseline: &Baseline, source: &str, main: &Baseline) -> Vec<String> {
    let mut errors = Vec::new();
    for (id, old) in &main.metrics {
        let Some(metric) = baseline.metrics.get(id) else {
            continue;
        };
        let measured_here = metric.source.as_deref() == Some(source);
        if !measured_here && old.source.as_deref() != Some(source) {
            continue;
        }
        if metric.source != old.source
            || metric.unit != old.unit
            || metric.direction != old.direction
        {
            errors.push(format!(
                "{id}: source, unit or direction changed against main ({:?} {} {} to {:?} {} {}); a different measure is a new metric id",
                old.source,
                old.unit,
                old.direction.word(),
                metric.source,
                metric.unit,
                metric.direction.word()
            ));
            continue;
        }
        if is_timing(id) {
            continue;
        }
        for (target, bar) in &metric.bars {
            let Some(old_bar) = old.bars.get(target) else {
                continue;
            };
            if old
                .direction
                .worse(old_bar.value, bar.value, Tolerance::Count(0))
                && !new_decision(bar.decision.as_deref(), old_bar.decision.as_deref())
            {
                errors.push(format!(
                    "{id} {target}: the bar moved from {} on main to {} without a `decision` of its own",
                    old_bar.value, bar.value
                ));
            }
        }
        if metric.tolerance.looser_than(old.tolerance)
            && !new_decision(
                metric.tolerance_decision.as_deref(),
                old.tolerance_decision.as_deref(),
            )
        {
            errors.push(format!(
                "{id}: the tolerance loosened from {:?} on main to {:?} without a `tolerance_decision` of its own",
                old.tolerance, metric.tolerance
            ));
        }
    }
    errors
}

/// `baseline` with every improved and every new value of `source` taken
/// from `measured`; a worse value is left as it is.
pub fn bless(baseline: &Baseline, source: &str, measured: &Measured) -> Baseline {
    let mut out = baseline.clone();
    for (id, target, value) in measured {
        let Some(metric) = out.metrics.get_mut(id) else {
            continue;
        };
        if metric.source.as_deref() != Some(source) || is_timing(id) {
            continue;
        }
        let direction = metric.direction;
        let climbing = (metric.kind == "hill-climb").then_some(());
        match metric.bars.get_mut(target) {
            Some(bar)
                if climbing.is_none()
                    && direction.better(bar.value, *value, Tolerance::Count(0)) =>
            {
                bar.value = *value
            }
            Some(_) => {}
            None => {
                metric.bars.insert(
                    target.clone(),
                    Bar {
                        value: *value,
                        decision: None,
                    },
                );
            }
        }
    }
    out
}

/// Hold `measured` of `source` against the baseline (blessed first under
/// `IGNITE_BLESS=1`) and `main`'s; panics with every refusal.
pub fn hold(source: &str, measured: &Measured) {
    let mut baseline = load_baseline();
    if blessing() {
        baseline = bless(&baseline, source, measured);
        save_baseline(&baseline);
    }
    let errors = check(&baseline, source, measured, main_baseline().as_ref());
    assert!(
        errors.is_empty(),
        "the quality ratchet refuses `{source}`:\n  {}",
        errors.join("\n  ")
    );
}

/// Write `measured` for `task quality` to read.
pub fn write_measured(path: &Path, measured: &Measured) {
    write_measured_with_load(path, measured, None);
}

fn write_measured_with_load(path: &Path, measured: &Measured, load: Option<&str>) {
    let values = measured
        .iter()
        .map(|(id, target, value)| {
            let mut fields = BTreeMap::from([
                ("metric".to_string(), Json::Str(id.clone())),
                ("target".to_string(), Json::Str(target.clone())),
                ("value".to_string(), Json::Int(*value)),
            ]);
            if let Some(load) = load {
                fields.insert("load".to_string(), Json::Str(load.to_string()));
            }
            Json::Object(fields)
        })
        .collect();
    let mut out = String::new();
    Json::Object(BTreeMap::from([(
        "values".to_string(),
        Json::Array(values),
    )]))
    .write(&mut out, 0);
    out.push('\n');
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, out).unwrap();
}

/// One value read back from a measured file, with its load when timed.
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    pub metric: String,
    pub target: String,
    pub value: i64,
    pub load: Option<String>,
}

fn read_measured(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}; run `task quality`", path.display()));
    let json = parse(&text, &path.display().to_string());
    json.get("values")
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("{}: no `values`", path.display()))
        .iter()
        .map(|v| Value {
            metric: v.get("metric").and_then(Json::as_str).unwrap().to_string(),
            target: v.get("target").and_then(Json::as_str).unwrap().to_string(),
            value: v.get("value").and_then(Json::as_i64).unwrap(),
            load: v.get("load").and_then(Json::as_str).map(str::to_string),
        })
        .collect()
}

// ---- the differential runner's lists -----------------------------------

/// The runner's refusals: differences not known, known ones gone, units
/// the evaluator cannot run that the baseline does not list (or lists and
/// it now runs), and its metrics.
pub fn check_differential(outcome: &differential::Outcome) -> Vec<String> {
    let mut baseline = load_baseline();
    let metrics = steady(&baseline, outcome);
    if blessing() {
        baseline.eval_cannot_run = outcome.eval_cannot.clone();
        baseline = bless(&baseline, "differential", &metrics);
        save_baseline(&baseline);
    }
    let main = main_baseline();
    let mut errors = differential_lists(&baseline, outcome);
    if let Some(main) = &main {
        errors.extend(slow_against_main(&baseline, main));
    }
    errors.extend(check(&baseline, "differential", &metrics, main.as_ref()));
    errors
}

/// A unit listed as over the evaluator's time limit is not evaluated in a
/// merge run, so listing it is a decision: a unit slow here that `main` does
/// not list as slow needs an `eval_slow` decision that is not `main`'s.
fn slow_against_main(baseline: &Baseline, main: &Baseline) -> Vec<String> {
    let mut errors = Vec::new();
    for (unit, reason) in &baseline.eval_cannot_run {
        if !is_slow(reason) {
            continue;
        }
        let was_slow = main.eval_cannot_run.get(unit).is_some_and(|r| r == reason);
        if was_slow {
            continue;
        }
        if !new_decision(
            baseline.eval_slow.get(unit).map(String::as_str),
            main.eval_slow.get(unit).map(String::as_str),
        ) {
            errors.push(format!(
                "{unit}: listed as {reason:?}, which a merge run does not evaluate, without an `eval_slow` decision of its own"
            ));
        }
    }
    errors
}

/// A reason of `eval_cannot_run` that keeps the unit out of a merge run.
pub fn is_slow(reason: &str) -> bool {
    reason.starts_with("over the time limit")
}

/// Whether the planning store holds `story` (`story:<id>`).
fn story_exists(story: &str) -> bool {
    story.strip_prefix("story:").is_some_and(|id| {
        workspace()
            .join(".engineering/planning/story")
            .join(format!("{id}.md"))
            .is_file()
    })
}

/// The runner's metrics with 1.1 counting only the steady differences: a
/// known difference marked `intermittent` (a timing assertion under the
/// machine's load) is left out whether or not it showed in this run.
pub fn steady(baseline: &Baseline, outcome: &differential::Outcome) -> Measured {
    let count = |consumer: &str, differs: &BTreeMap<String, String>| {
        differs
            .keys()
            .filter(|unit| {
                !baseline
                    .known_differences
                    .get(*unit)
                    .is_some_and(|k| k.consumer == consumer && k.intermittent)
            })
            .count() as i64
    };
    let mut metrics: Measured = outcome
        .metrics
        .iter()
        .map(|(id, target, value)| {
            let value = match (id.as_str(), target.as_str()) {
                ("1.1", "llvm") => count("llvm", &outcome.llvm_differs),
                ("1.1", "eval") => count("eval", &outcome.eval_differs),
                _ => *value,
            };
            (id.clone(), target.clone(), value)
        })
        .collect();
    // 1.7: the intermittent entries themselves, a gate that may only go down,
    // so no regression hides under the mark for good.
    metrics.push((
        "1.7".to_string(),
        "all".to_string(),
        baseline
            .known_differences
            .values()
            .filter(|k| k.intermittent)
            .count() as i64,
    ));
    metrics
}

fn differential_lists(baseline: &Baseline, outcome: &differential::Outcome) -> Vec<String> {
    let mut errors = Vec::new();
    for (consumer, differs) in [
        ("llvm", &outcome.llvm_differs),
        ("eval", &outcome.eval_differs),
    ] {
        for (unit, what) in differs {
            match baseline.known_differences.get(unit) {
                Some(known) if known.consumer == consumer => {}
                _ => errors.push(format!(
                    "{unit}: {consumer} differs from rust ({what}); not a known difference"
                )),
            }
        }
    }
    for (unit, known) in &baseline.known_differences {
        let differs = match known.consumer.as_str() {
            "llvm" => outcome.llvm_differs.contains_key(unit),
            "eval" => outcome.eval_differs.contains_key(unit),
            _ => false,
        };
        if !differs && !known.intermittent {
            errors.push(format!(
                "{unit}: the known difference of {} ({}) is gone; remove it from quality/baseline.json",
                known.consumer, known.story
            ));
        }
    }
    for (unit, known) in &baseline.known_differences {
        if known.intermittent && !story_exists(&known.story) {
            errors.push(format!(
                "{unit}: an intermittent difference names {}, which is no story of .engineering/planning/story/",
                known.story
            ));
        }
    }
    for (unit, what) in &outcome.no_run {
        errors.push(format!("{unit}: {what}, and no `not run:` reason lists it"));
    }
    for (unit, reason) in &outcome.eval_cannot {
        match baseline.eval_cannot_run.get(unit) {
            None => errors.push(format!(
                "{unit}: the evaluator cannot run it ({reason}); not in `eval_cannot_run` (IGNITE_BLESS=1 after a decision)"
            )),
            Some(listed) if listed != reason => errors.push(format!(
                "{unit}: the evaluator cannot run it for another reason than listed: {reason:?}, listed {listed:?} (IGNITE_BLESS=1 after a decision)"
            )),
            Some(_) => {}
        }
    }
    for (unit, reason) in &baseline.eval_cannot_run {
        if is_slow(reason) != baseline.eval_slow.contains_key(unit) {
            errors.push(format!(
                "{unit}: `eval_slow` lists exactly the units of `eval_cannot_run` over the time limit"
            ));
        }
    }
    for unit in baseline.eval_slow.keys() {
        if !baseline.eval_cannot_run.contains_key(unit) {
            errors.push(format!("{unit}: in `eval_slow`, not in `eval_cannot_run`"));
        }
    }
    for unit in baseline.eval_cannot_run.keys() {
        if !outcome.eval_cannot.contains_key(unit) {
            errors.push(format!(
                "{unit}: listed in `eval_cannot_run`, and the evaluator runs it or it is gone; remove it (IGNITE_BLESS=1)"
            ));
        }
    }
    errors
}

// ---- static metrics -----------------------------------------------------

fn files_under(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != "target" && n != "node_modules")
            {
                files_under(&path, extension, out);
            }
        } else if path.extension().is_some_and(|e| e == extension) {
            out.push(path);
        }
    }
}

fn read_all(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .collect()
}

/// The crate directories under `crates/`.
fn crates(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// 2.1: diagnostic codes `docs/slice.md` names (`E` and three digits),
/// less the ones its table marks retired, that no file under a crate's
/// `tests/`, `std/` or `examples/` mentions.
fn codes_without_a_test(root: &Path) -> i64 {
    let slice = std::fs::read_to_string(root.join("docs/slice.md")).unwrap();
    let mut codes = BTreeSet::new();
    let mut retired = BTreeSet::new();
    for line in slice.lines() {
        let bytes = line.as_bytes();
        for (i, _) in line.match_indices('E') {
            let digits = &bytes[i + 1..(i + 4).min(bytes.len())];
            let before = i.checked_sub(1).map(|j| bytes[j]);
            let after = bytes.get(i + 4);
            if digits.len() == 3
                && digits.iter().all(u8::is_ascii_digit)
                && !before.is_some_and(|b| b.is_ascii_alphanumeric())
                && !after.is_some_and(|b| b.is_ascii_alphanumeric())
            {
                codes.insert(line[i..i + 4].to_string());
            }
        }
        if let Some(rest) = line.strip_prefix("| E")
            && let Some((code, meaning)) = rest.split_once(" | ")
            && meaning.starts_with("retired")
        {
            retired.insert(format!("E{code}"));
        }
    }
    let mut files = Vec::new();
    for dir in crates(root) {
        files_under(&dir.join("tests"), "rs", &mut files);
        files_under(&dir.join("tests"), "expected", &mut files);
        files_under(&dir.join("tests"), "ignite", &mut files);
    }
    files_under(&root.join("std"), "ignite", &mut files);
    files_under(&root.join("examples"), "ignite", &mut files);
    let texts = read_all(&files);
    codes
        .iter()
        .filter(|code| !retired.contains(*code))
        .filter(|code| !texts.iter().any(|t| t.contains(code.as_str())))
        .count() as i64
}

/// 1.4: generated-code golden outputs changed against the merge base with
/// `main` while `CODEGEN_REVISION` is the merge base's.
fn goldens_without_a_revision(root: &Path) -> i64 {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git");
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let Some(base) = git(&["merge-base", "main", "HEAD"]) else {
        return 0;
    };
    let base = base.trim();
    let plan = "crates/ignite/src/apply/plan.rs";
    let revision = |text: &str| {
        text.lines()
            .find(|l| l.contains("pub const CODEGEN_REVISION"))
            .map(str::to_string)
    };
    let now = revision(&std::fs::read_to_string(root.join(plan)).unwrap());
    let then = git(&["show", &format!("{base}:{plan}")]).and_then(|t| revision(&t));
    if now != then {
        return 0;
    }
    let changed = git(&[
        "diff",
        "--name-only",
        base,
        "--",
        "crates/ignite-rustgen/tests/golden",
        "crates/ignite-llvm/tests/golden",
    ])
    .unwrap_or_default();
    changed
        .lines()
        .filter(|path| path.ends_with(".rs") || path.ends_with(".ll"))
        .count() as i64
}

/// 3.5: `extern "C"` entries of `ignite-rt` whose body is not run through
/// `crate::entry` (the rule of `contain::tests::every_entry_is_guarded`,
/// `ignite_rt_init` excepted).
fn unguarded_entries(root: &Path) -> i64 {
    let mut files = Vec::new();
    files_under(&root.join("crates/ignite-rt/src"), "rs", &mut files);
    let mut unguarded = 0;
    for text in read_all(&files) {
        for (at, _) in text.match_indices("\npub extern \"C\" fn ") {
            let rest = &text[at + 1..];
            let body = &rest[..rest.find("\n}\n").unwrap_or(rest.len())];
            let guarded = body.contains("crate::entry(") || body.contains("crate::entry_never(");
            if !guarded && !body.starts_with("pub extern \"C\" fn ignite_rt_init(") {
                unguarded += 1;
            }
        }
    }
    unguarded
}

/// 3.6: `unsafe` blocks, fns, impls and extern blocks in `dir`.
fn unsafe_sites(dir: &Path) -> i64 {
    let mut files = Vec::new();
    files_under(dir, "rs", &mut files);
    read_all(&files)
        .iter()
        .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|line| {
            let code = line.split("//").next().unwrap_or("");
            ["unsafe {", "unsafe fn", "unsafe impl", "unsafe extern"]
                .iter()
                .any(|form| code.contains(form))
                || code.trim_end().ends_with("unsafe")
        })
        .count() as i64
}

/// Every static metric of the tree.
pub fn static_metrics() -> Measured {
    let root = workspace();
    let mut out = Measured::new();
    let mut push = |id: &str, target: &str, value: i64| {
        out.push((id.to_string(), target.to_string(), value));
    };
    for (target, krate) in [
        ("rust", "ignite-rustgen"),
        ("llvm", "ignite-llvm"),
        ("eval", "ignite-eval"),
    ] {
        let mut files = Vec::new();
        files_under(
            &root.join("crates").join(krate).join("src"),
            "rs",
            &mut files,
        );
        let texts = read_all(&files);
        let patterns: usize = texts
            .iter()
            .map(|t| t.matches("PatternKind::").count())
            .sum();
        let mut builtins = BTreeSet::new();
        for t in &texts {
            for (at, _) in t.match_indices("Builtin::") {
                let name: String = t[at + "Builtin::".len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    builtins.insert(name);
                }
            }
        }
        push("1.3", &format!("{target}/patterns"), patterns as i64);
        push("1.3", &format!("{target}/builtins"), builtins.len() as i64);
    }
    push("1.4", "all", goldens_without_a_revision(&root));
    push("2.1", "all", codes_without_a_test(&root));
    let mut expected = Vec::new();
    for dir in crates(&root) {
        files_under(&dir.join("tests"), "expected", &mut expected);
    }
    push("2.5", "all", expected.len() as i64);
    let limits: usize = crates(&root)
        .iter()
        .filter_map(|dir| std::fs::read_to_string(dir.join("tests/limits.rs")).ok())
        .map(|t| t.matches("#[test]").count())
        .sum();
    push("2.6", "all", limits as i64);
    let mut rust_goldens = Vec::new();
    files_under(
        &root.join("crates/ignite-rustgen/tests/golden"),
        "rs",
        &mut rust_goldens,
    );
    let clones: usize = read_all(&rust_goldens)
        .iter()
        .map(|t| t.matches(".clone()").count())
        .sum();
    push("3.1", "rust", clones as i64);
    let mut ll = Vec::new();
    files_under(&root.join("crates/ignite-llvm/tests/golden"), "ll", &mut ll);
    let copies: usize = read_all(&ll)
        .iter()
        .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|line| line.contains("call ") && line.contains("@\"copy."))
        .count();
    push("3.1", "llvm", copies as i64);
    push("3.5", "llvm", unguarded_entries(&root));
    let io = unsafe_sites(&root.join("crates/ignite-io/src"));
    let rt = unsafe_sites(&root.join("crates/ignite-rt/src"));
    push("3.6", "rust", io);
    push("3.6", "llvm", io + rt);
    let mut rust_files = Vec::new();
    files_under(&root.join("crates"), "rs", &mut rust_files);
    files_under(&root.join("tools"), "rs", &mut rust_files);
    let ignored = read_all(&rust_files)
        .iter()
        .flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|line| line.trim_start().starts_with("#[ignore"))
        .count();
    push("7.2", "all", ignored as i64);
    let mut sources = Vec::new();
    for dir in crates(&root) {
        files_under(&dir.join("src"), "rs", &mut sources);
    }
    files_under(&root.join("tools"), "rs", &mut sources);
    sources.retain(|p| p.components().any(|c| c.as_os_str() == "src"));
    files_under(&root.join("std"), "ignite", &mut sources);
    let long = read_all(&sources)
        .iter()
        .filter(|t| t.lines().count() > 3000)
        .count();
    push("7.3", "all", long as i64);
    let friction = std::fs::read_to_string(root.join("docs/friction.md")).unwrap();
    let open = friction
        .split("\n## ")
        .find(|section| section.starts_with("Open"))
        .map_or(0, |section| {
            section
                .lines()
                .filter(|line| {
                    line.strip_prefix("| ")
                        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
                })
                .count()
        });
    push("7.4", "all", open as i64);
    out
}

// ---- conformance --------------------------------------------------------

/// The protocol files whose claims 4.1 and 4.2 count.
const CONFORMING: [&str; 3] = [
    "std/protocol/telnet.ignite",
    "std/protocol/http.ignite",
    "std/protocol/jsonrpc.ignite",
];

/// 4.1 and 4.2 on `target`: claims that hold, and gaps per claimed spec,
/// read from the claim lines of `ignite conformance`'s matrix.
fn conformance_metrics(target: ignite::Target) -> Measured {
    let name = match target {
        ignite::Target::Rust => "rust",
        ignite::Target::Llvm => "llvm",
    };
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("quality-conformance");
    let mut holding = 0;
    let mut out = Measured::new();
    for (index, file) in CONFORMING.iter().enumerate() {
        let options = ignite::TestOptions {
            build: ignite::BuildOptions {
                target,
                out: Some(scratch.join(format!("{name}-{index}"))),
                target_dir: (target == ignite::Target::Rust).then(|| scratch.join("cargo")),
                runtime: (target == ignite::Target::Llvm).then(crate::common::runner_archive),
                ..Default::default()
            },
            runner: Vec::new(),
        };
        // A test executable just written can be held open for an instant by
        // another test thread's fork (ETXTBSY, `Text file busy`); the report
        // is then taken again, as `llvm_build::run_copied` retries.
        let mut attempts = 0;
        let report = loop {
            match ignite::conformance::report(&workspace().join(file), &options) {
                Err(e) if e.to_string().contains("os error 26") && attempts < 20 => {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                other => break other,
            }
        };
        let report = report
            .unwrap_or_else(|e| panic!("{file} on {name}: {e}"))
            .unwrap_or_else(|| panic!("{file}: no conforming protocol"));
        for line in report.text.lines() {
            let Some(rest) = line.strip_prefix("claim conformance of ") else {
                continue;
            };
            let (claim, verdict) = rest.split_once(": ").unwrap();
            let spec = claim.split(" to ").nth(1).unwrap();
            if verdict.starts_with("yes") {
                holding += 1;
            }
            let gaps: i64 = verdict
                .split('(')
                .nth(1)
                .and_then(|t| t.split(' ').next())
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("{line}: no gap count"));
            out.push(("4.2".to_string(), format!("{name}/{spec}"), gaps));
        }
    }
    out.push(("4.1".to_string(), name.to_string(), holding));
    out
}

fn quality_dir() -> PathBuf {
    workspace().join("target/quality")
}

/// The static metrics against their bars, every merge.
#[test]
fn static_metrics_hold_the_bar() {
    let measured = static_metrics();
    write_measured(&quality_dir().join("static-metrics.json"), &measured);
    hold("static", &measured);
}

/// 4.1 and 4.2 on both targets against their bars, every merge.
#[test]
fn conformance_claims_hold_the_bar() {
    let (rust, llvm) = std::thread::scope(|scope| {
        let rust = scope.spawn(|| conformance_metrics(ignite::Target::Rust));
        let llvm = scope.spawn(|| conformance_metrics(ignite::Target::Llvm));
        (rust.join().unwrap(), llvm.join().unwrap())
    });
    let mut measured = rust;
    measured.extend(llvm);
    write_measured(&quality_dir().join("conformance-metrics.json"), &measured);
    hold("conformance", &measured);
}

// ---- history, trend and pages -------------------------------------------

/// One record of `quality/history.jsonl`.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub commit: String,
    pub date: String,
    /// `bar` for the first assessment, `vX.Y.Z` for a release, `run`
    /// otherwise.
    pub run: String,
    pub metric: String,
    pub target: String,
    pub value: i64,
    /// Where the run was measured: `local` for this workstation (a timed
    /// row then carries its 1-minute `load`), `ec2:<type>` for the
    /// dedicated timing host (`ignite-perf quality-remote`).
    pub host: String,
    pub load: Option<String>,
    /// A dedicated-host record (`ignite-perf quality-remote`): the BASE
    /// commit measured beside it, BASE's median, and the machine with the
    /// run's cost.
    pub base: Option<String>,
    pub base_value: Option<i64>,
    pub machine: Option<Json>,
}

impl Record {
    fn line(&self) -> String {
        let mut fields = BTreeMap::from([
            ("commit".to_string(), Json::Str(self.commit.clone())),
            ("date".to_string(), Json::Str(self.date.clone())),
            ("host".to_string(), Json::Str(self.host.clone())),
            ("run".to_string(), Json::Str(self.run.clone())),
            ("metric".to_string(), Json::Str(self.metric.clone())),
            ("target".to_string(), Json::Str(self.target.clone())),
            ("value".to_string(), Json::Int(self.value)),
        ]);
        if let Some(load) = &self.load {
            fields.insert("load".to_string(), Json::Str(load.clone()));
        }
        if let Some(base) = &self.base {
            fields.insert("base".to_string(), Json::Str(base.clone()));
        }
        if let Some(value) = self.base_value {
            fields.insert("base_value".to_string(), Json::Int(value));
        }
        if let Some(machine) = &self.machine {
            fields.insert("machine".to_string(), machine.clone());
        }
        Json::Object(fields).compact()
    }

    fn parse(line: &str) -> Result<Record, String> {
        let json = Json::parse(line)?;
        let field = |key: &str| {
            json.get(key)
                .and_then(Json::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{line}: no `{key}`"))
        };
        Ok(Record {
            commit: field("commit")?,
            date: field("date")?,
            host: field("host")?,
            run: field("run")?,
            metric: field("metric")?,
            target: field("target")?,
            value: json
                .get("value")
                .and_then(Json::as_i64)
                .ok_or_else(|| format!("{line}: no `value`"))?,
            load: json.get("load").and_then(Json::as_str).map(str::to_string),
            base: json.get("base").and_then(Json::as_str).map(str::to_string),
            base_value: json.get("base_value").and_then(Json::as_i64),
            machine: json.get("machine").cloned(),
        })
    }

    /// A run that gets a row of the trend: the bar and every release.
    fn is_release(&self) -> bool {
        self.run == "bar" || self.run.starts_with('v')
    }
}

fn history_path() -> PathBuf {
    workspace().join("quality/history.jsonl")
}

pub fn read_history() -> Vec<Record> {
    std::fs::read_to_string(history_path())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Record::parse(l).unwrap())
        .collect()
}

/// The runs of `history` in order, each `(run, commit, date)` with its
/// values by `(metric, target)`.
type Runs = Vec<((String, String, String), BTreeMap<(String, String), i64>)>;

/// Whether `host` is the dedicated timing host (`ec2:<type>`), whose runs
/// `ignite-perf quality-remote` writes: timing rows only, kept apart from the
/// local runs.
fn dedicated(host: &str) -> bool {
    host.starts_with("ec2:")
}

/// The local runs of `history`; the dedicated host's are in [`host_runs`].
fn runs(history: &[Record]) -> Runs {
    let mut out: Runs = Vec::new();
    for record in history.iter().filter(|r| !dedicated(&r.host)) {
        let key = (
            record.run.clone(),
            record.commit.clone(),
            record.date.clone(),
        );
        if out.last().map(|(k, _)| k) != Some(&key) {
            out.push((key, BTreeMap::new()));
        }
        out.last_mut()
            .unwrap()
            .1
            .insert((record.metric.clone(), record.target.clone()), record.value);
    }
    out
}

/// The runs of the dedicated host in order, each `(run, commit, date, host)`
/// with its values by `(metric, target)`.
type HostRuns = Vec<(
    (String, String, String, String),
    BTreeMap<(String, String), i64>,
)>;

fn host_runs(history: &[Record]) -> HostRuns {
    let mut out: HostRuns = Vec::new();
    for record in history.iter().filter(|r| dedicated(&r.host)) {
        let key = (
            record.run.clone(),
            record.commit.clone(),
            record.date.clone(),
            record.host.clone(),
        );
        if out.last().map(|(k, _)| k) != Some(&key) {
            out.push((key, BTreeMap::new()));
        }
        out.last_mut()
            .unwrap()
            .1
            .insert((record.metric.clone(), record.target.clone()), record.value);
    }
    out
}

/// The timing rows: measured by `ignite-perf quality` in ms (the sizes it
/// measures, 3.2, are deterministic and held as any count). Recorded by
/// `task quality` with `host: local`, they gate nothing; measured on the
/// dedicated host by `ignite-perf quality-remote` (`host: ec2:<type>`), a
/// release run refuses a row worse than its BASE, the last release, in the
/// same run (`tools/ignite-perf/src/remote.rs`). The list is fixed here, not read from the baseline's labels, so a relabelled
/// metric is not exempted (`the_baseline_is_well_formed` holds the labels to
/// it, `check` refuses a relabel against `main`).
const TIMING: [&str; 3] = ["5.3", "6.1", "6.2"];

fn is_timing(id: &str) -> bool {
    TIMING.contains(&id)
}

/// A value with its unit.
fn shown(value: i64, unit: &str) -> String {
    match unit {
        "ms" => format!("{value} ms"),
        "bytes" => format!("{value} B"),
        _ => value.to_string(),
    }
}

/// The change from `from` to `to` of a metric, in words.
fn change(metric: &Metric, from: i64, to: i64) -> String {
    if from == to {
        return "=".to_string();
    }
    let delta = to - from;
    let sign = if delta > 0 { "+" } else { "−" };
    let verdict = if metric.direction.worse(from, to, metric.tolerance) {
        " worse"
    } else if metric.direction.better(from, to, Tolerance::Count(0)) {
        " better"
    } else {
        ""
    };
    format!("{sign}{}{verdict}", delta.abs())
}

/// `quality/trend.md`: one row per release (and the bar), with the change
/// of every value against the row before.
pub fn render_trend(baseline: &Baseline, history: &[Record]) -> String {
    let mut out = String::from(
        "# Quality trend\n\nGenerated from `quality/history.jsonl` by `task quality` (`quality::record`); do not edit.\nOne row per release and the first assessment (`bar`); the changes are against the row before\n(`docs/quality.md`). Timing rows (source `perf`) measured on `host: local`, the shared\nworkstation, are not comparable between runs: they are left out of `worse`, `better` and\n`changes` and counted under `timing`; the\ndedicated host's runs have their own table below the main one.\n\n| run | commit | date | worse | better | timing | changes |\n|---|---|---|---|---|---|---|\n",
    );
    let mut previous: Option<BTreeMap<(String, String), i64>> = None;
    for ((run, commit, date), values) in runs(history) {
        if !(run == "bar" || run.starts_with('v')) {
            continue;
        }
        let timing = values.keys().filter(|(id, _)| is_timing(id)).count();
        let timing = if timing == 0 {
            String::new()
        } else {
            format!("{timing} local, not comparable")
        };
        let (mut worse, mut better, mut changes) = (0, 0, Vec::new());
        if let Some(previous) = &previous {
            for ((id, target), value) in &values {
                let (Some(metric), Some(old)) = (
                    baseline.metrics.get(id),
                    previous.get(&(id.clone(), target.clone())),
                ) else {
                    continue;
                };
                if old == value || is_timing(id) {
                    continue;
                }
                if metric.direction.worse(*old, *value, metric.tolerance) {
                    worse += 1;
                } else if metric.direction.better(*old, *value, Tolerance::Count(0)) {
                    better += 1;
                }
                changes.push(format!(
                    "{id} {target} {}→{}",
                    shown(*old, &metric.unit),
                    shown(*value, &metric.unit)
                ));
            }
        }
        let changes = if previous.is_none() {
            format!("{} values, the bar", values.len())
        } else if changes.is_empty() {
            "none".to_string()
        } else {
            changes.join("; ")
        };
        out.push_str(&format!(
            "| {run} | {} | {date} | {worse} | {better} | {timing} | {changes} |\n",
            &commit[..commit.len().min(8)]
        ));
        previous = Some(values);
    }
    let hosts = host_runs(history);
    if !hosts.is_empty() {
        out.push_str(
            "\n## Timing on the dedicated host\n\nRuns of `ignite-perf quality-remote` (`task quality-remote`): the timing rows only, measured on a\nsingle-tenant EC2 host made for the run, HEAD's median over interleaved passes; the changes are\nagainst the run before on a host of the same type.\n\n| run | commit | date | host | values | changes |\n|---|---|---|---|---|---|\n",
        );
        for (index, ((run, commit, date, host), values)) in hosts.iter().enumerate() {
            let shown_values: Vec<String> = values
                .iter()
                .map(|((id, target), value)| {
                    let unit = baseline.metrics.get(id).map_or("", |m| m.unit.as_str());
                    format!("{id} {target} {}", shown(*value, unit))
                })
                .collect();
            let before = hosts[..index]
                .iter()
                .rev()
                .find(|((_, _, _, h), _)| h == host)
                .map(|(_, v)| v);
            let changes = match before {
                None => "first on this host".to_string(),
                Some(before) => {
                    let mut changes = Vec::new();
                    for ((id, target), value) in values {
                        let (Some(metric), Some(old)) = (
                            baseline.metrics.get(id),
                            before.get(&(id.clone(), target.clone())),
                        ) else {
                            continue;
                        };
                        if old == value {
                            continue;
                        }
                        let verdict = if metric.direction.worse(*old, *value, metric.tolerance) {
                            " worse"
                        } else if metric.direction.better(*old, *value, Tolerance::Count(0)) {
                            " better"
                        } else {
                            ""
                        };
                        changes.push(format!(
                            "{id} {target} {}→{}{verdict}",
                            shown(*old, &metric.unit),
                            shown(*value, &metric.unit)
                        ));
                    }
                    if changes.is_empty() {
                        "none".to_string()
                    } else {
                        changes.join("; ")
                    }
                }
            };
            out.push_str(&format!(
                "| {run} | {} | {date} | {host} | {} | {changes} |\n",
                &commit[..commit.len().min(8)],
                shown_values.join("; ")
            ));
        }
    }
    out
}

/// The text of a metric's bar: every target's value, or why it has none.
fn bar_text(metric: &Metric) -> String {
    if let Some(reason) = &metric.unmeasured {
        return format!("unmeasured: {reason}");
    }
    metric
        .bars
        .iter()
        .map(|(target, bar)| format!("{target} {}", shown(bar.value, &metric.unit)))
        .collect::<Vec<_>>()
        .join(", ")
}

const DOCS_START: &str = "<!-- generated by quality::record from quality/baseline.json -->";
const DOCS_END: &str = "<!-- end of the generated table -->";

/// The generated metric table of `docs/quality.md`.
pub fn render_docs_table(baseline: &Baseline) -> String {
    let mut out = String::new();
    for (index, area) in baseline.areas.iter().enumerate() {
        out.push_str(&format!(
            "\n### {} {area}\n\n| # | metric | kind | better | instrument | bar | target |\n|---|---|---|---|---|---|---|\n",
            index + 1
        ));
        for (id, m) in &baseline.metrics {
            if &m.area != area {
                continue;
            }
            out.push_str(&format!(
                "| {id} | {} | {} | {} | {} | {} | {} |\n",
                m.name,
                m.kind,
                m.direction.word(),
                m.instrument,
                bar_text(m),
                m.goal.as_deref().unwrap_or("")
            ));
        }
    }
    out
}

/// `docs` with its generated table replaced by `table`.
fn splice_docs(docs: &str, table: &str) -> Option<String> {
    let start = docs.find(DOCS_START)? + DOCS_START.len();
    let end = docs.find(DOCS_END)?;
    Some(format!("{}\n{table}\n{}", &docs[..start], &docs[end..]))
}

/// The book page `site/book/src/quality.md`: every metric per area and
/// target with its bar, its current value (the last run of the history)
/// and its trend (the change against the last release, or the bar).
pub fn render_site_page(baseline: &Baseline, history: &[Record]) -> String {
    let all = runs(history);
    let current = all.last().map(|(_, v)| v.clone()).unwrap_or_default();
    let current_run = all
        .last()
        .map(|((run, commit, date), _)| {
            format!(
                "`{run}` on {date}, commit `{}`",
                &commit[..commit.len().min(8)]
            )
        })
        .unwrap_or_else(|| "none yet".to_string());
    let reference = all
        .iter()
        .rev()
        .skip(1)
        .find(|((run, _, _), _)| run == "bar" || run.starts_with('v'))
        .map(|(_, v)| v.clone());
    let mut out = String::from(
        "# Quality\n\n\
<!-- Generated from quality/baseline.json and quality/history.jsonl by `task quality`; site::quality_page_is_generated checks it. -->\n\n\
Ignite keeps a written set of quality metrics. Today's values are the bar: a merge may not make a\n\
gated metric worse, a release may not be worse than the release before it, and an improvement is\n\
written into the bar in the same change, so the bar only moves one way. The list, its kinds and its\n\
instruments are in [`docs/quality.md`](https://github.com/ignite-lang/ignite-lang/blob/main/docs/quality.md);\n\
the bars are in `quality/baseline.json` and every measured value in `quality/history.jsonl`.\n\n\
- **gate**: a count of failures; the bar is never above today, usually 0.\n\
- **ratchet**: a value with a direction and a tolerance; it may not get worse than the bar.\n\
- **hill-climb**: tracked and aimed at a target; not refused when it moves.\n\n\
Targets: `rust` is the Rust target, `llvm` the LLVM target, `eval` the compile-time evaluator,\n\
`all` a value that does not differ by target. The first of these metrics, one result across the\n\
three, is checked on every merge by running one corpus of programs on all three.\n\n\
Timing rows (build, check and restart times) are measured on a single-tenant EC2 host made for the\n\
run (`ec2:<type>`, its trend against the run before on that host type); until such a run exists\n\
they show the shared workstation's values (`host: local`), recorded but not comparable.\n\n",
    );
    let hosts = host_runs(history);
    out.push_str(&format!("Current values: run {current_run}.\n"));
    for (index, area) in baseline.areas.iter().enumerate() {
        out.push_str(&format!(
            "\n## {} {area}\n\n| # | metric | kind | target | bar | current | trend |\n|---|---|---|---|---|---|---|\n",
            index + 1
        ));
        for (id, m) in &baseline.metrics {
            if &m.area != area {
                continue;
            }
            if m.bars.is_empty() {
                let reason = m
                    .unmeasured
                    .as_deref()
                    .map_or("unmeasured".to_string(), |r| format!("unmeasured: {r}"));
                out.push_str(&format!(
                    "| {id} | {} | {} | | {reason} | | |\n",
                    m.name, m.kind
                ));
                continue;
            }
            for (target, bar) in &m.bars {
                let key = (id.clone(), target.clone());
                let mut now = current.get(&key);
                let on_host = hosts.iter().rposition(|(_, v)| v.contains_key(&key));
                if let Some(at) = on_host.filter(|_| is_timing(id)) {
                    let ((_, _, _, host), values) = &hosts[at];
                    now = values.get(&key);
                    let before = hosts[..at]
                        .iter()
                        .rev()
                        .find(|((_, _, _, h), v)| h == host && v.contains_key(&key))
                        .and_then(|(_, v)| v.get(&key));
                    let trend = match (before, now) {
                        (Some(before), Some(now)) => {
                            format!("{host}, {}", change(m, *before, *now))
                        }
                        _ => host.clone(),
                    };
                    out.push_str(&format!(
                        "| {id} | {} | {} | {target} | {} | {} | {trend} |\n",
                        m.name,
                        m.kind,
                        shown(bar.value, &m.unit),
                        now.map_or(String::new(), |v| shown(*v, &m.unit))
                    ));
                    continue;
                }
                let trend = match (now, reference.as_ref().and_then(|r| r.get(&key))) {
                    _ if is_timing(id) => "local, not comparable".to_string(),
                    (Some(now), Some(then)) => change(m, *then, *now),
                    _ => String::new(),
                };
                out.push_str(&format!(
                    "| {id} | {} | {} | {target} | {} | {} | {trend} |\n",
                    m.name,
                    m.kind,
                    shown(bar.value, &m.unit),
                    now.map_or(String::new(), |v| shown(*v, &m.unit))
                ));
            }
        }
    }
    out
}

pub fn site_page_path() -> PathBuf {
    workspace().join("site/book/src/quality.md")
}

fn trend_path() -> PathBuf {
    workspace().join("quality/trend.md")
}

fn docs_path() -> PathBuf {
    workspace().join("docs/quality.md")
}

/// Write the three generated texts from the baseline and the history.
fn write_pages(baseline: &Baseline, history: &[Record]) {
    std::fs::write(trend_path(), render_trend(baseline, history)).unwrap();
    std::fs::write(site_page_path(), render_site_page(baseline, history)).unwrap();
    let docs = std::fs::read_to_string(docs_path()).unwrap();
    let spliced =
        splice_docs(&docs, &render_docs_table(baseline)).expect("docs/quality.md markers");
    std::fs::write(docs_path(), spliced).unwrap();
}

/// The civil date (UTC) of `secs` after the epoch.
fn civil_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Every refusal of a release: a count value worse than at the last release
/// (or the bar) beyond its metric's tolerance. Timing rows are left out here:
/// `ignite-perf quality-remote` refuses them against BASE on the dedicated host.
pub fn release_refusals(baseline: &Baseline, history: &[Record], values: &[Value]) -> Vec<String> {
    let Some((_, last)) = runs(history)
        .into_iter()
        .rev()
        .find(|((run, _, _), _)| run == "bar" || run.starts_with('v'))
    else {
        return Vec::new();
    };
    let mut errors = Vec::new();
    for v in values {
        let (Some(metric), Some(then)) = (
            baseline.metrics.get(&v.metric),
            last.get(&(v.metric.clone(), v.target.clone())),
        ) else {
            continue;
        };
        if metric.kind != "hill-climb"
            && !is_timing(&v.metric)
            && metric.direction.worse(*then, v.value, metric.tolerance)
        {
            errors.push(format!(
                "{} {}: {} at the last release, {} now ({}, {:?})",
                v.metric,
                v.target,
                then,
                v.value,
                metric.direction.word(),
                metric.tolerance
            ));
        }
    }
    errors
}

/// `task quality`: read every source's measured values from
/// `target/quality/` (the gate tests and `ignite-perf quality` wrote them),
/// refuse a release (`IGNITE_QUALITY_RUN=vX.Y.Z`) whose count rows are worse
/// than the last, append one record per metric and target to
/// `quality/history.jsonl` with its `host` (`IGNITE_QUALITY_HOST`, default
/// `local`) and regenerate `quality/trend.md`, the table of
/// `docs/quality.md` and the book page. The sizes of `ignite-perf quality`
/// are held against their bars; its timing rows are recorded and gate
/// nothing: on `host: local` they are not comparable. Under
/// `IGNITE_BLESS=1` a timing row without a bar gets one as its reference.
#[test]
#[ignore = "task quality runs it after the measurements"]
fn record() {
    let dir = quality_dir();
    let mut values = Vec::new();
    for source in SOURCES {
        values.extend(read_measured(&dir.join(format!("{source}-metrics.json"))));
    }
    let perf: Measured = values
        .iter()
        .filter(|v| v.load.is_some())
        .map(|v| (v.metric.clone(), v.target.clone(), v.value))
        .collect();
    let mut baseline = load_baseline();
    if blessing() {
        baseline = bless(&baseline, "perf", &perf);
        for v in values.iter().filter(|v| v.load.is_some()) {
            if let Some(metric) = baseline.metrics.get_mut(&v.metric)
                && is_timing(&v.metric)
            {
                metric.bars.entry(v.target.clone()).or_insert(Bar {
                    value: v.value,
                    decision: None,
                });
            }
        }
        save_baseline(&baseline);
    }
    let host = std::env::var("IGNITE_QUALITY_HOST").unwrap_or_else(|_| "local".to_string());
    let mut errors = check(&baseline, "perf", &perf, None);
    let run = std::env::var("IGNITE_QUALITY_RUN").unwrap_or_else(|_| "run".to_string());
    let history = read_history();
    let refused = if run.starts_with('v') {
        release_refusals(&baseline, &history, &values)
    } else {
        Vec::new()
    };
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap();
    let date = civil_date(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    let label = if refused.is_empty() {
        run.clone()
    } else {
        "run".to_string()
    };
    let mut text = std::fs::read_to_string(history_path()).unwrap_or_default();
    for v in &values {
        let record = Record {
            commit: commit.clone(),
            date: date.clone(),
            run: label.clone(),
            metric: v.metric.clone(),
            target: v.target.clone(),
            value: v.value,
            host: host.clone(),
            load: v.load.clone(),
            base: None,
            base_value: None,
            machine: None,
        };
        text.push_str(&record.line());
        text.push('\n');
    }
    std::fs::write(history_path(), text).unwrap();
    write_pages(&baseline, &read_history());
    errors.extend(
        refused
            .iter()
            .map(|e| format!("release {run} refused: {e}")),
    );
    assert!(
        errors.is_empty(),
        "task quality refuses this run:\n  {}",
        errors.join("\n  ")
    );
}

// ---- the files hold together -------------------------------------------

#[test]
fn the_baseline_is_well_formed() {
    let text = std::fs::read_to_string(baseline_path()).expect("quality/baseline.json");
    let baseline = read_baseline(&parse(&text, "quality/baseline.json")).unwrap();
    assert_eq!(
        write_baseline(&baseline),
        text,
        "quality/baseline.json is not in its canonical form"
    );
    for (id, metric) in &baseline.metrics {
        if metric.source.is_some() {
            assert!(!metric.bars.is_empty(), "{id}: measured, without a bar");
        }
        let timing_labels = metric.source.as_deref() == Some("perf") && metric.unit == "ms";
        assert_eq!(
            timing_labels,
            is_timing(id),
            "{id}: the timing rows are exactly {TIMING:?}, labelled `source: perf`, `unit: ms`"
        );
        if metric.kind == "hill-climb" {
            assert!(
                metric.goal.is_some(),
                "{id}: a hill-climb metric names its target"
            );
        }
    }
    for line in std::fs::read_to_string(history_path())
        .expect("quality/history.jsonl")
        .lines()
    {
        let record = Record::parse(line).unwrap();
        assert!(
            baseline.metrics.contains_key(&record.metric),
            "{line}: a metric the baseline does not list"
        );
        assert_eq!(record.line(), line, "a history line not in canonical form");
    }
}

/// The generated texts are the ones the baseline and history give.
#[test]
fn trend_and_docs_are_generated_from_quality() {
    let baseline = load_baseline();
    let history = read_history();
    assert!(
        history.iter().any(Record::is_release),
        "quality/history.jsonl holds no first assessment (`bar`)"
    );
    if blessing() {
        write_pages(&baseline, &history);
    }
    assert_eq!(
        std::fs::read_to_string(trend_path()).unwrap(),
        render_trend(&baseline, &history),
        "quality/trend.md is not generated from the history (IGNITE_BLESS=1)"
    );
    let docs = std::fs::read_to_string(docs_path()).unwrap();
    assert_eq!(
        splice_docs(&docs, &render_docs_table(&baseline)).as_deref(),
        Some(docs.as_str()),
        "docs/quality.md's table is not generated from quality/baseline.json (IGNITE_BLESS=1)"
    );
}

// ---- the ratchet's own break-tests --------------------------------------

fn metric(direction: Direction, tolerance: Tolerance, bars: &[(&str, i64)]) -> Metric {
    Metric {
        area: "A".to_string(),
        name: "m".to_string(),
        kind: "ratchet".to_string(),
        direction,
        unit: "count".to_string(),
        instrument: "i".to_string(),
        source: Some("static".to_string()),
        tolerance,
        tolerance_decision: None,
        goal: None,
        unmeasured: None,
        bars: bars
            .iter()
            .map(|(t, v)| {
                (
                    t.to_string(),
                    Bar {
                        value: *v,
                        decision: None,
                    },
                )
            })
            .collect(),
    }
}

fn baseline_of(metrics: &[(&str, Metric)]) -> Baseline {
    Baseline {
        about: String::new(),
        areas: vec!["A".to_string()],
        metrics: metrics
            .iter()
            .map(|(id, m)| (id.to_string(), m.clone()))
            .collect(),
        known_differences: BTreeMap::new(),
        eval_cannot_run: BTreeMap::new(),
        eval_slow: BTreeMap::new(),
    }
}

fn one(id: &str, target: &str, value: i64) -> Measured {
    vec![(id.to_string(), target.to_string(), value)]
}

/// The refusals of the ratchet, each on its own case: a worse value, an
/// improvement not taken, a missing value and a value without a bar; an
/// equal value and a timing inside its tolerance pass, in both directions.
#[test]
fn the_ratchet_refuses_worse_missing_new_and_untaken_values() {
    let lower = baseline_of(&[(
        "3.6",
        metric(Direction::Lower, Tolerance::Count(0), &[("llvm", 73)]),
    )]);
    assert_eq!(
        check(&lower, "static", &one("3.6", "llvm", 73), None),
        Vec::<String>::new()
    );
    let worse = check(&lower, "static", &one("3.6", "llvm", 74), None);
    assert!(worse.len() == 1 && worse[0].contains("worse"), "{worse:?}");
    let better = check(&lower, "static", &one("3.6", "llvm", 72), None);
    assert!(
        better.len() == 1 && better[0].contains("improved"),
        "{better:?}"
    );
    let missing = check(&lower, "static", &Measured::new(), None);
    assert!(
        missing.len() == 1 && missing[0].contains("missing"),
        "{missing:?}"
    );
    let new = check(&lower, "static", &one("3.6", "rust", 7), None);
    assert!(
        new.iter()
            .any(|e| e.contains("not in quality/baseline.json")),
        "{new:?}"
    );
    let unknown = check(&lower, "static", &one("9.9", "all", 1), None);
    assert!(
        unknown
            .iter()
            .any(|e| e.contains("not in quality/baseline.json")),
        "{unknown:?}"
    );

    let higher = baseline_of(&[(
        "4.3",
        metric(Direction::Higher, Tolerance::Count(0), &[("rust", 309)]),
    )]);
    let fewer = check(&higher, "static", &one("4.3", "rust", 308), None);
    assert!(fewer.len() == 1 && fewer[0].contains("worse"), "{fewer:?}");
    let more = check(&higher, "static", &one("4.3", "rust", 310), None);
    assert!(more.len() == 1 && more[0].contains("improved"), "{more:?}");

    let percent = baseline_of(&[(
        "6.9",
        metric(
            Direction::Lower,
            Tolerance::Percent(10),
            &[("all/check", 100)],
        ),
    )]);
    assert!(check(&percent, "static", &one("6.9", "all/check", 109), None).is_empty());
    assert!(check(&percent, "static", &one("6.9", "all/check", 91), None).is_empty());
    assert_eq!(
        check(&percent, "static", &one("6.9", "all/check", 111), None).len(),
        1
    );
    assert_eq!(
        check(&percent, "static", &one("6.9", "all/check", 89), None).len(),
        1
    );

    let mut climbing = metric(
        Direction::Lower,
        Tolerance::Count(0),
        &[("eval/builtins", 112)],
    );
    climbing.kind = "hill-climb".to_string();
    let climb = baseline_of(&[("1.3", climbing)]);
    assert!(check(&climb, "static", &one("1.3", "eval/builtins", 115), None).is_empty());
    assert!(check(&climb, "static", &one("1.3", "eval/builtins", 100), None).is_empty());
    assert_eq!(check(&climb, "static", &Measured::new(), None).len(), 1);
    assert_eq!(
        bless(&climb, "static", &one("1.3", "eval/builtins", 100)).metrics["1.3"].bars
            ["eval/builtins"]
            .value,
        112,
        "a hill-climb bar stays the first assessment's"
    );

    let other = check(&lower, "differential", &one("3.6", "llvm", 73), None);
    assert!(other.iter().any(|e| e.contains("measured by")), "{other:?}");

    let blessed = bless(&lower, "static", &one("3.6", "llvm", 70));
    assert_eq!(blessed.metrics["3.6"].bars["llvm"].value, 70);
    let kept = bless(&lower, "static", &one("3.6", "llvm", 80));
    assert_eq!(
        kept.metrics["3.6"].bars["llvm"].value, 73,
        "a worse value is never blessed"
    );
    let added = bless(&lower, "static", &one("3.6", "rust", 7));
    assert_eq!(added.metrics["3.6"].bars["rust"].value, 7);
}

#[test]
fn a_bar_made_worse_than_mains_needs_a_decision() {
    let main = baseline_of(&[(
        "7.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("all", 16)]),
    )]);
    let raised = baseline_of(&[(
        "7.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("all", 18)]),
    )]);
    let values = one("7.2", "all", 18);
    let errors = check(&raised, "static", &values, Some(&main));
    assert!(
        errors.len() == 1 && errors[0].contains("without a `decision`"),
        "{errors:?}"
    );
    let mut decided = raised.clone();
    decided
        .metrics
        .get_mut("7.2")
        .unwrap()
        .bars
        .get_mut("all")
        .unwrap()
        .decision = Some("story:x, 2026-11-07".to_string());
    assert!(check(&decided, "static", &values, Some(&main)).is_empty());
    let loose = baseline_of(&[(
        "7.2",
        metric(Direction::Lower, Tolerance::Count(5), &[("all", 16)]),
    )]);
    let loosened = check(&loose, "static", &one("7.2", "all", 16), Some(&main));
    assert!(
        loosened.len() == 1 && loosened[0].contains("tolerance loosened"),
        "{loosened:?}"
    );
}

#[test]
fn a_release_worse_than_the_last_is_refused() {
    let mut timing = metric(Direction::Lower, Tolerance::Percent(10), &[("llvm", 900)]);
    timing.source = Some("perf".to_string());
    timing.unit = "ms".to_string();
    let baseline = baseline_of(&[
        (
            "4.3",
            metric(Direction::Higher, Tolerance::Count(0), &[("rust", 309)]),
        ),
        ("6.2", timing),
    ]);
    let record = |run: &str, metric: &str, target: &str, value: i64| Record {
        commit: "c".to_string(),
        date: "2026-11-07".to_string(),
        run: run.to_string(),
        metric: metric.to_string(),
        target: target.to_string(),
        value,
        host: "local".to_string(),
        load: (metric == "6.2").then(|| "20.00".to_string()),
        base: None,
        base_value: None,
        machine: None,
    };
    let history = vec![
        record("bar", "4.3", "rust", 309),
        record("bar", "6.2", "llvm", 900),
        record("run", "4.3", "rust", 300),
        record("run", "6.2", "llvm", 950),
        record("v0.14.0", "4.3", "rust", 312),
        record("v0.14.0", "6.2", "llvm", 2000),
    ];
    let value = |metric: &str, target: &str, value| Value {
        metric: metric.to_string(),
        target: target.to_string(),
        value,
        load: (metric == "6.2").then(|| "20.00".to_string()),
    };
    assert!(release_refusals(&baseline, &history, &[value("4.3", "rust", 312)]).is_empty());
    let refused = release_refusals(&baseline, &history, &[value("4.3", "rust", 310)]);
    assert!(
        refused.len() == 1 && refused[0].contains("312 at the last release"),
        "{refused:?}"
    );
    let held = check(&baseline, "perf", &one("6.2", "llvm", 9000), None);
    assert!(
        held.is_empty(),
        "a timing row on host local was held: {held:?}"
    );
    let timed = release_refusals(&baseline, &history, &[value("6.2", "llvm", 9000)]);
    assert!(
        timed.is_empty(),
        "a timing row on host local refused a release: {timed:?}"
    );
    let trend = render_trend(&baseline, &history);
    assert!(
        trend.contains(
            "| bar | c | 2026-11-07 | 0 | 0 | 1 local, not comparable | 2 values, the bar |"
        ),
        "{trend}"
    );
    assert!(
        trend.contains(
            "| v0.14.0 | c | 2026-11-07 | 0 | 1 | 1 local, not comparable | 4.3 rust 309→312 |"
        ),
        "{trend}"
    );
    assert!(!trend.contains("| run | c |"), "{trend}");
    let line = history[1].line();
    assert!(
        line.contains(r#""host":"local""#) && line.contains(r#""load":"20.00""#),
        "{line}"
    );
    assert_eq!(Record::parse(&line).unwrap(), history[1]);
    let page = render_site_page(&baseline, &history);
    assert!(
        page.contains("| llvm | 900 ms | 2000 ms | local, not comparable |"),
        "{page}"
    );
}

/// Runs on the dedicated timing host (`host: ec2:<type>`, written by
/// `ignite-perf quality-remote`) get a timing table of their own in the
/// trend and give the page its timing values; the local runs keep the main
/// table, the count values and the release refusals.
#[test]
fn a_dedicated_host_run_has_its_own_timing_trend() {
    let mut timing = metric(Direction::Lower, Tolerance::Percent(10), &[("llvm", 900)]);
    timing.source = Some("perf".to_string());
    timing.unit = "ms".to_string();
    let baseline = baseline_of(&[
        (
            "4.3",
            metric(Direction::Higher, Tolerance::Count(0), &[("rust", 309)]),
        ),
        ("6.2", timing),
    ]);
    let record = |run: &str, commit: &str, metric: &str, value: i64, host: &str| Record {
        commit: commit.to_string(),
        date: "2026-11-08".to_string(),
        run: run.to_string(),
        metric: metric.to_string(),
        target: if metric == "4.3" { "rust" } else { "llvm" }.to_string(),
        value,
        host: host.to_string(),
        load: (metric == "6.2").then(|| "0.10".to_string()),
        base: None,
        base_value: None,
        machine: None,
    };
    let ec2 = "ec2:c7i.metal-24xl";
    let history = vec![
        record("bar", "c", "4.3", 309, "local"),
        record("bar", "c", "6.2", 900, "local"),
        record("run", "d", "6.2", 410, ec2),
        record("run", "e", "6.2", 380, ec2),
    ];
    let trend = render_trend(&baseline, &history);
    assert!(
        trend.contains(
            "| bar | c | 2026-11-08 | 0 | 0 | 1 local, not comparable | 2 values, the bar |"
        ),
        "{trend}"
    );
    assert!(trend.contains("## Timing on the dedicated host"), "{trend}");
    assert!(
        trend.contains(&format!(
            "| run | d | 2026-11-08 | {ec2} | 6.2 llvm 410 ms | first on this host |"
        )),
        "{trend}"
    );
    assert!(
        trend.contains(&format!(
            "| run | e | 2026-11-08 | {ec2} | 6.2 llvm 380 ms | 6.2 llvm 410 ms→380 ms better |"
        )),
        "{trend}"
    );
    let page = render_site_page(&baseline, &history);
    assert!(page.contains("| rust | 309 | 309 |"), "{page}");
    assert!(
        page.contains(&format!("| llvm | 900 ms | 380 ms | {ec2}, −30 better |")),
        "{page}"
    );
    let value = Value {
        metric: "4.3".to_string(),
        target: "rust".to_string(),
        value: 309,
        load: None,
    };
    assert!(release_refusals(&baseline, &history, &[value]).is_empty());
    let line = r#"{"base":"b","base_value":400,"commit":"e","date":"2026-11-08","host":"ec2:c7i.metal-24xl","load":"0.10","machine":{"cpu":"x","smt":"off"},"metric":"6.2","run":"run","target":"llvm","value":380}"#;
    let parsed = Record::parse(line).unwrap();
    assert_eq!(
        parsed.line(),
        line,
        "a dedicated-host record does not round-trip"
    );
    assert_eq!(
        (parsed.base.as_deref(), parsed.base_value, parsed.value),
        (Some("b"), Some(400), 380)
    );
}

#[test]
fn the_differential_lists_refuse_unknown_gone_and_unlisted_units() {
    let mut baseline = baseline_of(&[]);
    baseline.known_differences.insert(
        "a.ignite::t".to_string(),
        Known {
            consumer: "eval".to_string(),
            story: "story:x".to_string(),
            what: "w".to_string(),
            intermittent: false,
        },
    );
    baseline
        .eval_cannot_run
        .insert("b.ignite::u".to_string(), "IO".to_string());
    let outcome = |eval_differs: &[&str], cannot: &[&str]| differential::Outcome {
        units: BTreeMap::new(),
        llvm_differs: BTreeMap::new(),
        eval_differs: eval_differs
            .iter()
            .map(|u| (u.to_string(), "d".to_string()))
            .collect(),
        eval_cannot: cannot
            .iter()
            .map(|u| (u.to_string(), "IO".to_string()))
            .collect(),
        build_failed: BTreeMap::new(),
        unchecked: BTreeMap::new(),
        no_run: BTreeMap::new(),
        metrics: Measured::new(),
    };
    assert!(differential_lists(&baseline, &outcome(&["a.ignite::t"], &["b.ignite::u"])).is_empty());
    let unknown = differential_lists(
        &baseline,
        &outcome(&["a.ignite::t", "c::v"], &["b.ignite::u"]),
    );
    assert!(
        unknown.len() == 1 && unknown[0].contains("not a known difference"),
        "{unknown:?}"
    );
    let gone = differential_lists(&baseline, &outcome(&[], &["b.ignite::u"]));
    assert!(gone.len() == 1 && gone[0].contains("is gone"), "{gone:?}");
    let unlisted = differential_lists(
        &baseline,
        &outcome(&["a.ignite::t"], &["b.ignite::u", "d::w"]),
    );
    assert!(
        unlisted.len() == 1 && unlisted[0].contains("not in `eval_cannot_run`"),
        "{unlisted:?}"
    );
    let runs_now = differential_lists(&baseline, &outcome(&["a.ignite::t"], &[]));
    assert!(
        runs_now.len() == 1 && runs_now[0].contains("evaluator runs it"),
        "{runs_now:?}"
    );

    baseline.known_differences.insert(
        "load.ignite::timed".to_string(),
        Known {
            consumer: "llvm".to_string(),
            story: "story:llvm-store-update-off-lock".to_string(),
            what: "a timing assertion".to_string(),
            intermittent: true,
        },
    );
    let mut flaky = outcome(&["a.ignite::t", "x::steady"], &["b.ignite::u"]);
    flaky.metrics = vec![
        ("1.1".to_string(), "llvm".to_string(), 1),
        ("1.1".to_string(), "eval".to_string(), 2),
        ("1.2".to_string(), "llvm".to_string(), 7),
    ];
    let quiet = flaky.clone();
    flaky
        .llvm_differs
        .insert("load.ignite::timed".to_string(), "exit".to_string());
    let shown = differential_lists(&baseline, &flaky);
    assert!(
        shown.iter().all(|e| !e.contains("load.ignite::timed")),
        "{shown:?}"
    );
    let gone = differential_lists(&baseline, &quiet);
    assert!(
        gone.iter().all(|e| !e.contains("load.ignite::timed")),
        "an intermittent difference absent from a run was refused as gone: {gone:?}"
    );
    let expected: Measured = vec![
        ("1.1".to_string(), "llvm".to_string(), 0),
        ("1.1".to_string(), "eval".to_string(), 2),
        ("1.2".to_string(), "llvm".to_string(), 7),
        ("1.7".to_string(), "all".to_string(), 1),
    ];
    assert_eq!(steady(&baseline, &flaky), expected);
    assert_eq!(steady(&baseline, &quiet), expected);
}

#[test]
fn the_differential_refuses_a_missing_run_a_slow_listing_without_a_decision_and_an_unknown_story() {
    let empty = differential::Outcome {
        units: BTreeMap::new(),
        llvm_differs: BTreeMap::new(),
        eval_differs: BTreeMap::new(),
        eval_cannot: BTreeMap::new(),
        build_failed: BTreeMap::new(),
        unchecked: BTreeMap::new(),
        no_run: BTreeMap::new(),
        metrics: Measured::new(),
    };
    let baseline = baseline_of(&[]);
    assert!(differential_lists(&baseline, &empty).is_empty());

    let mut missing = empty.clone();
    missing
        .no_run
        .insert("a.ignite::t".to_string(), "no run on LLVM".to_string());
    let errors = differential_lists(&baseline, &missing);
    assert!(
        errors.len() == 1 && errors[0].contains("no run on LLVM"),
        "{errors:?}"
    );

    let slow_unit = "s.ignite::slow".to_string();
    let slow_reason = "over the time limit of 60 s".to_string();
    let mut slow = baseline.clone();
    slow.eval_cannot_run
        .insert(slow_unit.clone(), slow_reason.clone());
    let mut ran_slow = empty.clone();
    ran_slow
        .eval_cannot
        .insert(slow_unit.clone(), slow_reason.clone());
    let errors = differential_lists(&slow, &ran_slow);
    assert!(
        errors.len() == 1 && errors[0].contains("`eval_slow`"),
        "{errors:?}"
    );
    let main = baseline.clone();
    let errors = slow_against_main(&slow, &main);
    assert!(
        errors.len() == 1 && errors[0].contains("without an `eval_slow` decision"),
        "{errors:?}"
    );
    let mut decided = slow.clone();
    decided
        .eval_slow
        .insert(slow_unit.clone(), "story:x, measured 185 s".to_string());
    assert!(differential_lists(&decided, &ran_slow).is_empty());
    assert!(slow_against_main(&decided, &main).is_empty());
    let mut stale = decided.clone();
    stale
        .eval_cannot_run
        .insert("s.ignite::other".to_string(), slow_reason.clone());
    stale.eval_slow.insert(
        "s.ignite::other".to_string(),
        "story:x, measured 185 s".to_string(),
    );
    let mut main_with = main.clone();
    main_with.eval_slow.insert(
        "s.ignite::other".to_string(),
        "story:x, measured 185 s".to_string(),
    );
    let errors = slow_against_main(&stale, &main_with);
    assert!(
        errors.len() == 1 && errors[0].contains("s.ignite::other"),
        "main's decision for a unit main does not list as slow: {errors:?}"
    );

    let mut unknown = baseline.clone();
    unknown.known_differences.insert(
        "l.ignite::t".to_string(),
        Known {
            consumer: "llvm".to_string(),
            story: "story:no-such-story-anywhere".to_string(),
            what: "w".to_string(),
            intermittent: true,
        },
    );
    let errors = differential_lists(&unknown, &empty);
    assert!(
        errors.len() == 1 && errors[0].contains("no story"),
        "{errors:?}"
    );
    assert_eq!(
        steady(&unknown, &empty),
        vec![("1.7".to_string(), "all".to_string(), 1)]
    );
}

#[test]
fn civil_dates() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(1_794_009_600), "2026-11-07");
}

// ---- adversary, story quality-ratchet pass 1 ----------------------------

/// A decision is taken for one move of a bar. Once a bar carries one, the
/// bar must not get worse again against `main` on the same decision: the
/// check reads only that a decision is present, not that it is new.
#[test]
fn adversary_a_stale_decision_does_not_let_a_bar_worsen_again() {
    let decided = |value: i64| {
        let mut b = baseline_of(&[(
            "7.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("all", value)]),
        )]);
        b.metrics
            .get_mut("7.2")
            .unwrap()
            .bars
            .get_mut("all")
            .unwrap()
            .decision = Some("story:x, 2026-11-07".to_string());
        b
    };
    let main = decided(18);
    let worse_again = decided(30);
    let errors = check(&worse_again, "static", &one("7.2", "all", 30), Some(&main));
    assert!(
        errors.iter().any(|e| e.contains("without a `decision`")),
        "the bar moved 18 -> 30 on main's own decision and passed: {errors:?}"
    );
}

/// The tolerance of a metric loosened against `main` is accepted when any
/// bar of the metric carries any decision, also one taken earlier for
/// another target.
#[test]
fn adversary_a_tolerance_loosened_needs_its_own_decision() {
    let with_old_decision = |tolerance: Tolerance| {
        let mut b = baseline_of(&[(
            "3.1",
            metric(Direction::Lower, tolerance, &[("llvm", 90), ("rust", 168)]),
        )]);
        b.metrics
            .get_mut("3.1")
            .unwrap()
            .bars
            .get_mut("rust")
            .unwrap()
            .decision = Some("story:old, 2026-11-07".to_string());
        b
    };
    let main = with_old_decision(Tolerance::Count(0));
    let loose = with_old_decision(Tolerance::Count(50));
    let values = vec![
        ("3.1".to_string(), "llvm".to_string(), 90),
        ("3.1".to_string(), "rust".to_string(), 168),
    ];
    let errors = check(&loose, "static", &values, Some(&main));
    assert!(
        errors.iter().any(|e| e.contains("tolerance loosened")),
        "tolerance 0 -> 50 passed on main's old decision: {errors:?}"
    );
}

/// A count metric relabelled as a timing row (`source: perf`, `unit: ms`)
/// in the baseline is skipped by every check, so the instrument that still
/// measures it can report any value.
#[test]
fn adversary_a_metric_relabelled_as_timing_is_not_silently_ungated() {
    let main = baseline_of(&[(
        "7.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("all", 21)]),
    )]);
    let mut relabelled = main.clone();
    let m = relabelled.metrics.get_mut("7.2").unwrap();
    m.source = Some("perf".to_string());
    m.unit = "ms".to_string();
    let errors = check(&relabelled, "static", &one("7.2", "all", 500), Some(&main));
    assert!(
        !errors.is_empty(),
        "7.2 measured 500 against a bar of 21 and nothing refused it"
    );
}

/// docs/quality.md: "The units the evaluator cannot run are listed in
/// `eval_cannot_run` with the reason, exactly". The reason is not compared,
/// so a unit listed for an IO builtin that now fails for another reason
/// (a fuel overrun, an evaluator defect reported as `evaluation failed`)
/// passes.
#[test]
fn adversary_eval_cannot_run_reasons_are_held_exactly() {
    let mut baseline = baseline_of(&[]);
    baseline.eval_cannot_run.insert(
        "b.ignite::u".to_string(),
        "builtin `PipePair` has no value at compile time".to_string(),
    );
    let outcome = differential::Outcome {
        units: BTreeMap::new(),
        llvm_differs: BTreeMap::new(),
        eval_differs: BTreeMap::new(),
        eval_cannot: BTreeMap::from([(
            "b.ignite::u".to_string(),
            "`module(\"demo.ops\")` has no field `greeting`".to_string(),
        )]),
        build_failed: BTreeMap::new(),
        unchecked: BTreeMap::new(),
        no_run: BTreeMap::new(),
        metrics: Measured::new(),
    };
    let errors = differential_lists(&baseline, &outcome);
    assert!(
        !errors.is_empty(),
        "a changed reason for a unit the evaluator cannot run passed"
    );
}
