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
//! the claim lines of `ignite conformance`), `perf` (`ignite-perf
//! quality`, timing and sizes, measured by `task quality` only) or `bench`
//! (the native benchmark of `ignite-perf quality-remote`, [`BENCH`]). Each value
//! is held against its bar as the UI ratchet (`site::ux`) does: a value
//! worse than the bar beyond the metric's tolerance, a bar the run did not
//! measure, a measured value without a bar and an improvement not written
//! into the baseline are refused; and, against `main`'s baseline, a bar
//! made worse or a tolerance loosened without a `decision`.
//! `IGNITE_BLESS=1` writes improvements and new values into the baseline;
//! it never writes a worse one.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
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
    /// `static`, `differential`, `conformance`, `perf` or `bench`; `None` for a
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
    /// `"release_growth": "decision"` in the baseline: a binary-size row
    /// ([`SIZE`]). A release may leave a cell of it above the last release
    /// when the cell's bar carries a new `decision` naming the code that
    /// added the bytes, one the baseline at the last release does not hold
    /// ([`release_judged_against`]); a merge reads nothing from it.
    pub grows_by_decision: bool,
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
    /// The decision that listed a unit of `known_differences` as
    /// intermittent; a unit `main` does not list so needs one of its own.
    pub intermittent_decisions: BTreeMap<String, String>,
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
const SOURCES: [&str; 5] = ["static", "differential", "conformance", "perf", "bench"];
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
        let grows_by_decision = match fields.get("release_growth") {
            None => false,
            Some(Json::Str(rule)) if rule == "decision" => {
                if !SIZE.contains(&id.as_str()) {
                    return Err(format!(
                        "{id}: `release_growth` is for the binary-size rows {SIZE:?} only; no decision holds another row for a release"
                    ));
                }
                true
            }
            Some(other) => {
                return Err(format!(
                    "{id}: `release_growth` is {}, and \"decision\" is its only value",
                    other.compact()
                ));
            }
        };
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
                grows_by_decision,
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
    let intermittent_decisions = match root.get("intermittent_decisions") {
        None => BTreeMap::new(),
        Some(map) => object(map, "`intermittent_decisions`")?
            .iter()
            .map(|(unit, decision)| {
                decision
                    .as_str()
                    .map(|d| (unit.clone(), d.to_string()))
                    .ok_or_else(|| format!("{unit}: the decision is no text"))
            })
            .collect::<Result<_, _>>()?,
    };
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
        intermittent_decisions,
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
            if m.grows_by_decision {
                fields.insert("release_growth".to_string(), s("decision"));
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
            "intermittent_decisions".to_string(),
            Json::Object(
                baseline
                    .intermittent_decisions
                    .iter()
                    .map(|(unit, decision)| (unit.clone(), s(decision)))
                    .collect(),
            ),
        ),
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
    let text = baseline_text_at("main")?;
    Some(read_baseline(&parse(&text, "main:quality/baseline.json")).expect("main's baseline"))
}

/// The last release that was published: the nearest `v*` tag reachable from
/// HEAD other than `run` (the candidate itself), as `git describe` finds it;
/// `NoTag` before the first tag, `Unreadable` when git fails. A label in the history without a tag (a candidate the release gate
/// refused) is never the last release.
fn last_release_tag(run: &str) -> LastRelease {
    let output = match Command::new("git")
        .args([
            "describe",
            "--tags",
            "--abbrev=0",
            "--match",
            "v*",
            "--exclude",
            run,
            "HEAD",
        ])
        .current_dir(workspace())
        .output()
    {
        Ok(output) => output,
        Err(e) => return LastRelease::Unreadable(format!("git: {e}")),
    };
    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() && !tag.is_empty() {
        LastRelease::Tag(tag)
    } else if stderr.contains("No names found") || stderr.contains("No tags can describe") {
        LastRelease::NoTag
    } else {
        LastRelease::Unreadable(format!("git describe: {stderr}"))
    }
}

/// What the tag lookup of the last release found: its tag, no `v*` tag at
/// all (git says so), or a lookup that failed (git missing, not a
/// repository), with git's reason.
#[derive(Clone, Debug, PartialEq)]
pub enum LastRelease {
    Tag(String),
    NoTag,
    Unreadable(String),
}

impl From<Option<&str>> for LastRelease {
    fn from(tag: Option<&str>) -> LastRelease {
        tag.map_or(LastRelease::NoTag, |tag| LastRelease::Tag(tag.to_string()))
    }
}

/// The text of `quality/baseline.json` at the revision `rev`, when it has one.
fn baseline_text_at(rev: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{rev}:quality/baseline.json")])
        .current_dir(workspace())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
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
        if source == "differential" {
            errors.extend(intermittent_against_main(baseline, main));
        }
    }
    errors
}

/// An intermittent difference is neither counted in 1.1 nor refused when it
/// shows, so listing a unit as intermittent that `main` does not list so is
/// a decision: it needs an `intermittent_decisions` entry of its own (1.7
/// counts the entries, not which units they are).
fn intermittent_against_main(baseline: &Baseline, main: &Baseline) -> Vec<String> {
    let mut errors = Vec::new();
    for (unit, known) in &baseline.known_differences {
        let on_main = main
            .known_differences
            .get(unit)
            .is_some_and(|k| k.intermittent && k.consumer == known.consumer);
        if known.intermittent
            && !on_main
            && !new_decision(
                baseline
                    .intermittent_decisions
                    .get(unit)
                    .map(String::as_str),
                main,
            )
        {
            errors.push(format!(
                "{unit}: listed as an intermittent {} difference, which main does not list, without an `intermittent_decisions` entry of its own",
                known.consumer
            ));
        }
    }
    errors
}

/// Every decision `main`'s baseline holds, trimmed: on a bar, on a
/// tolerance, for a slow unit, for an intermittent difference.
fn decisions_of(main: &Baseline) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for metric in main.metrics.values() {
        out.extend(metric.bars.values().filter_map(|b| b.decision.clone()));
        out.extend(metric.tolerance_decision.clone());
    }
    out.extend(main.eval_slow.values().cloned());
    out.extend(main.intermittent_decisions.values().cloned());
    out.into_iter().map(|d| d.trim().to_string()).collect()
}

/// Whether `decision` is a decision taken in this change: present, not
/// blank, and, trimmed, none of the decisions `main` already holds (for
/// this place or any other), so an old decision cannot be reused.
fn new_decision(decision: Option<&str>, main: &Baseline) -> bool {
    decision.is_some_and(|d| {
        let d = d.trim();
        !d.is_empty() && !decisions_of(main).contains(d)
    })
}

/// The fewest letters and digits of a decision that names added code.
pub const DECISION_LETTERS: usize = 40;

/// How the decision on the bar of a size cell above the last release reads
/// against the decisions the last release's baseline held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecisionRead {
    /// It names nothing: fewer letters and digits than a decision has.
    NamesNothing,
    /// It is one the last release held.
    LastReleases,
    New,
}

/// The letters and digits of `text`, case folded: what a decision of a size
/// row is compared by, so that case, punctuation, spacing and characters
/// that show nothing make no decision a new one.
fn letters_and_digits(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// [`DecisionRead`] of `decision` against `held`, each read by its
/// [`letters_and_digits`]: with fewer than [`DECISION_LETTERS`] of them it
/// names nothing; when it is one of `held`, or contains one (the last
/// release's decision with a sentence added), it is the last release's;
/// otherwise it is new. One character changed makes it new: the check reads
/// that the text differs, a person reads what it says. A text of `held`
/// with fewer than [`DECISION_LETTERS`] letters and digits is no decision
/// and carries nothing over. `tools/ignite-perf/src/remote.rs` holds the
/// same function for row 3.9 (the tool and the gate share no crate), and
/// both are held to the same cases.
pub fn read_decision(decision: &str, held: &BTreeSet<String>) -> DecisionRead {
    let read = letters_and_digits(decision);
    if read.chars().count() < DECISION_LETTERS {
        return DecisionRead::NamesNothing;
    }
    let carried = held
        .iter()
        .map(|held| letters_and_digits(held))
        .any(|held| held.chars().count() >= DECISION_LETTERS && read.contains(&held));
    if carried {
        DecisionRead::LastReleases
    } else {
        DecisionRead::New
    }
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
                && !new_decision(bar.decision.as_deref(), main)
            {
                errors.push(format!(
                    "{id} {target}: the bar moved from {} on main to {} without a `decision` of its own",
                    old_bar.value, bar.value
                ));
            }
        }
        if metric.tolerance.looser_than(old.tolerance)
            && !new_decision(metric.tolerance_decision.as_deref(), main)
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
        if !new_decision(baseline.eval_slow.get(unit).map(String::as_str), main) {
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

/// The statuses of a story that still tracks work (`aep plan artifact
/// lifecycle story`: draft, proposed, active); `implemented`, `rejected`
/// and `archived` end it.
const OPEN_STORY: [&str; 3] = ["draft", "proposed", "active"];

/// Whether the planning store holds `story` (`story:<id>`) with an open
/// status in its frontmatter.
fn story_open(story: &str) -> bool {
    let Some(id) = story.strip_prefix("story:") else {
        return false;
    };
    let path = workspace()
        .join(".engineering/planning/story")
        .join(format!("{id}.md"));
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let frontmatter = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---"))
        .map_or("", |(front, _)| front);
    frontmatter
        .lines()
        .find_map(|line| line.strip_prefix("status:"))
        .is_some_and(|status| OPEN_STORY.contains(&status.trim()))
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
        if known.intermittent && !story_open(&known.story) {
            errors.push(format!(
                "{unit}: an intermittent difference names {}, which is no open story (draft, proposed or active) of .engineering/planning/story/",
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
/// `ignite_rt::UNGUARDED_ENTRIES` excepted, each with its exact body).
fn unguarded_entries(root: &Path) -> i64 {
    let mut files = Vec::new();
    files_under(&root.join("crates/ignite-rt/src"), "rs", &mut files);
    let mut unguarded = 0;
    for text in read_all(&files) {
        for (at, _) in text.match_indices("\npub extern \"C\" fn ") {
            let rest = &text[at + 1..];
            let body = &rest[..rest.find("\n}\n").unwrap_or(rest.len())];
            let name = rest["pub extern \"C\" fn ".len()..]
                .split(['(', '<'])
                .next()
                .unwrap_or("");
            let guarded = body.contains("crate::entry(") || body.contains("crate::entry_never(");
            let excepted = ignite_rt::UNGUARDED_ENTRIES.iter().any(|(entry, whole)| {
                *entry == name
                    && whole.is_none_or(|whole| {
                        body.split_once("{\n").map(|(_, inside)| inside.trim()) == Some(whole)
                    })
            });
            if !guarded && !excepted {
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

/// Row 3.6 at or below its v0.15.0 values in `quality/history.jsonl`
/// (story `unsafe-sites-back`), counted as [`static_metrics`] counts it:
/// the release rule of `AGENTS.md` § Releases for the `unsafe` sites of
/// the substrate, which a bar moved by a decision does not relax.
#[test]
fn unsafe_sites_are_no_more_than_at_v0_15_0() {
    let before: BTreeMap<String, i64> = read_history()
        .into_iter()
        .filter(|record| record.run == "v0.15.0" && record.metric == "3.6")
        .map(|record| (record.target, record.value))
        .collect();
    assert_eq!(
        before.keys().map(String::as_str).collect::<Vec<_>>(),
        ["llvm", "rust"],
        "v0.15.0's row 3.6 in quality/history.jsonl"
    );
    let now: BTreeMap<String, i64> = static_metrics()
        .into_iter()
        .filter(|(id, _, _)| id == "3.6")
        .map(|(_, target, value)| (target, value))
        .collect();
    let over: Vec<String> = before
        .iter()
        .filter_map(|(target, then)| match now.get(target) {
            Some(value) if value <= then => None,
            Some(value) => Some(format!("3.6 {target}: {value}, at v0.15.0 {then}")),
            None => Some(format!("3.6 {target}: not measured")),
        })
        .collect();
    assert!(
        over.is_empty(),
        "more unsafe sites than at v0.15.0:\n  {}",
        over.join("\n  ")
    );
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

/// Row 3.2's release binaries at most 10 % over v0.15.0's values in
/// `quality/history.jsonl` (story `readiness-binary-size`), built as
/// `ignite-perf quality` builds them: the LLVM target against the runner
/// archive of the dev profile (this test binary's), the Rust target with
/// its generated release profile.
#[test]
fn release_binaries_stay_within_10_percent_of_v0_15_0() {
    let before: BTreeMap<String, i64> = read_history()
        .into_iter()
        .filter(|record| record.run == "v0.15.0" && record.metric == "3.2")
        .map(|record| (record.target, record.value))
        .collect();
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("quality-size");
    let cargo_target = scratch.join("cargo");
    let mut over = Vec::new();
    for (source, executable) in [
        ("examples/todo/todo.ignite", "todod"),
        ("examples/terminal/terminal.ignite", "terminald"),
    ] {
        for (name, target) in [
            ("llvm", ignite::Target::Llvm),
            ("rust", ignite::Target::Rust),
        ] {
            let cell = format!("{name}/{executable}/release");
            let options = ignite::BuildOptions {
                target,
                release: true,
                out: Some(scratch.join(&cell)),
                runtime: (target == ignite::Target::Llvm).then(crate::common::runner_archive),
                target_dir: (target == ignite::Target::Rust).then(|| cargo_target.clone()),
                ..Default::default()
            };
            let built = ignite::build(&workspace().join(source), &options)
                .unwrap_or_else(|error| panic!("{cell}: {error}"));
            let path = built
                .executables
                .iter()
                .find(|path| path.file_stem().and_then(|stem| stem.to_str()) == Some(executable))
                .unwrap_or_else(|| panic!("{cell}: no {executable} in {:?}", built.executables));
            let size = std::fs::metadata(path).unwrap().len();
            let reference = *before
                .get(&cell)
                .unwrap_or_else(|| panic!("no v0.15.0 record of 3.2 {cell}"));
            let ceiling = reference + reference / 10;
            if size > ceiling as u64 {
                over.push(format!(
                    "{cell}: {size} B, v0.15.0 {reference} B, at most {ceiling} B ({:+.1} %)",
                    (size as f64 / reference as f64 - 1.0) * 100.0
                ));
            }
        }
    }
    assert!(
        over.is_empty(),
        "over 110 % of v0.15.0:\n{}",
        over.join("\n")
    );
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
    /// On a release's record of a cell of a size row above the last
    /// release: the decision on its bar that holds it ([`run_records`];
    /// `ignite-perf quality-remote` writes it for 3.9). The release's row
    /// of the trend is generated from it.
    pub decision: Option<String>,
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
        if let Some(decision) = &self.decision {
            fields.insert("decision".to_string(), Json::Str(decision.clone()));
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
            decision: json
                .get("decision")
                .and_then(Json::as_str)
                .map(str::to_string),
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

/// The binary-size rows: 3.2 (bytes of `todod` and `terminald`, `task
/// quality`) and 3.9 (the stripped release binary against plain Rust,
/// `task quality-remote`). They are the rows the baseline marks with
/// `"release_growth": "decision"` and the only ones it may mark
/// (`read_baseline`, `the_size_rows_are_marked_and_no_other_row_is`): time
/// comes before bytes, so a release may add bytes with the code it adds and
/// may add time on no row (`AGENTS.md` § Releases). The list is fixed here,
/// as [`TIMING`] is, so a row relabelled as a size row is not exempted.
const SIZE: [&str; 2] = ["3.2", "3.9"];

fn is_timing(id: &str) -> bool {
    TIMING.contains(&id)
}

/// The rows of the native benchmark (`bench/native/`, source `bench`): Ignite
/// against plain Rust as a ratio in permille, measured by `ignite-perf
/// quality-remote` on the dedicated host only: throughput, p99 latency, peak
/// RSS and stripped binary size. Targets are `<consumer>/<program>/<cell>`
/// (`<consumer>/<program>` for the size), the consumer `llvm`, `rust` (the
/// Rust target) or `go`, the Go reference, recorded without a bar. A release
/// is refused in the run against BASE (`tools/ignite-perf/src/remote.rs`); the
/// bars are the first run's ratios, ratcheted by [`bless_bench`].
const BENCH: [&str; 4] = ["3.3", "3.7", "3.8", "3.9"];

fn is_bench(id: &str) -> bool {
    BENCH.contains(&id)
}

/// The short label of a benchmark row in the trend.
fn bench_label(id: &str) -> &'static str {
    match id {
        "3.3" => "rps",
        "3.7" => "p99",
        "3.8" => "RSS",
        _ => "size",
    }
}

/// A value with its unit.
fn shown(value: i64, unit: &str) -> String {
    match unit {
        "ms" => format!("{value} ms"),
        "bytes" => format!("{value} B"),
        "permille" => format!("{:.3}×", value as f64 / 1000.0),
        _ => value.to_string(),
    }
}

/// The values of the benchmark in the latest dedicated-host run that has
/// them, by `(metric, target)`.
fn latest_bench(history: &[Record]) -> BTreeMap<(String, String), i64> {
    host_runs(history)
        .into_iter()
        .rev()
        .map(|(_, values)| values)
        .find(|values| values.keys().any(|(id, _)| is_bench(id)))
        .map(|values| {
            values
                .into_iter()
                .filter(|((id, _), _)| is_bench(id))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a benchmark target gets a bar: Ignite's targets do, the Go
/// reference does not.
fn barred(target: &str) -> bool {
    !target.starts_with("go/")
}

/// The refusals of the benchmark's bars against the latest dedicated-host
/// run: a target without a bar and an improvement beyond the tolerance not
/// written into the baseline (`IGNITE_BLESS=1`, [`bless_bench`]). A worse
/// ratio is not refused here: runs on different days are different hosts,
/// so a release compares within one run (`ignite-perf quality-remote`).
fn bench_bars(baseline: &Baseline, history: &[Record]) -> Vec<String> {
    let mut errors = Vec::new();
    for ((id, target), value) in latest_bench(history) {
        let Some(metric) = baseline.metrics.get(&id) else {
            errors.push(format!("{id}: recorded but not in quality/baseline.json"));
            continue;
        };
        if !barred(&target) {
            continue;
        }
        match metric.bars.get(&target) {
            None => errors.push(format!(
                "{id} {target}: {value} has no bar (IGNITE_BLESS=1)"
            )),
            Some(bar) if metric.direction.better(bar.value, value, metric.tolerance) => errors
                .push(format!(
                    "{id} {target}: improved to {value} from {}; write it into quality/baseline.json (IGNITE_BLESS=1)",
                    bar.value
                )),
            Some(_) => {}
        }
    }
    errors
}

/// `baseline` with a bar for every benchmark target of the latest
/// dedicated-host run that has none and every improved bar raised to it.
fn bless_bench(baseline: &Baseline, history: &[Record]) -> Baseline {
    let mut out = baseline.clone();
    for ((id, target), value) in latest_bench(history) {
        let Some(metric) = out.metrics.get_mut(&id) else {
            continue;
        };
        if !barred(&target) {
            continue;
        }
        let direction = metric.direction;
        let bar = metric.bars.entry(target).or_insert(Bar {
            value,
            decision: None,
        });
        if direction.better(bar.value, value, Tolerance::Count(0)) {
            bar.value = value;
        }
    }
    out
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

/// A held cell as [`held_cells`] collects it: its value at the last
/// release where one is recorded, its value now and its decision.
type HeldCell<'a> = (Option<i64>, i64, &'a str);

/// The size cells above the last release that a decision holds in the
/// release `(run, commit, date)` of `history`, grouped by decision in the
/// order of the cells: the records of the release that carry a `decision`
/// ([`run_records`]; for 3.9 the dedicated host's records of the same run
/// and commit, written by `ignite-perf quality-remote`). Each cell is its
/// id, its target, its value at the last release (the row before for a
/// local record, BASE's for the host's) and its value now.
fn held_cells(
    baseline: &Baseline,
    history: &[Record],
    (run, commit, date): (&str, &str, &str),
    previous: &BTreeMap<(String, String), i64>,
) -> Vec<(String, Vec<String>)> {
    let mut cells: BTreeMap<(bool, &str, &str), HeldCell> = BTreeMap::new();
    for record in history {
        let Some(decision) = record.decision.as_deref() else {
            continue;
        };
        let on_host = dedicated(&record.host);
        if record.run != run || record.commit != commit || (!on_host && record.date != date) {
            continue;
        }
        let then = if on_host {
            record.base_value
        } else {
            previous
                .get(&(record.metric.clone(), record.target.clone()))
                .copied()
        };
        cells.insert(
            (on_host, &record.metric, &record.target),
            (then, record.value, decision),
        );
    }
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for ((_, id, target), (then, now, decision)) in cells {
        let unit = baseline.metrics.get(id).map_or("", |m| m.unit.as_str());
        let cell = match then {
            Some(then) => format!("{id} {target} {}→{}", shown(then, unit), shown(now, unit)),
            None => format!("{id} {target} {}", shown(now, unit)),
        };
        match out.iter_mut().find(|(d, _)| d == decision) {
            Some((_, cells)) => cells.push(cell),
            None => out.push((decision.to_string(), vec![cell])),
        }
    }
    out
}

/// `text` as it stands inside a cell of a Markdown table: a `|` escaped, a
/// line break as a space.
fn table_text(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// `quality/trend.md`: one row per release (and the bar), with the change
/// of every value against the row before; a release's row ends with the
/// size cells above the last release, each group under the decision that
/// holds it ([`held_cells`]).
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
            let held = held_cells(baseline, history, (&run, &commit, &date), previous);
            if !held.is_empty() {
                changes.push(format!(
                    "size cells above the last release: {}",
                    held.iter()
                        .map(|(decision, cells)| format!(
                            "{} (decision: {})",
                            cells.join(", "),
                            table_text(decision)
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
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
    let all_hosts = host_runs(history);
    let hosts: HostRuns = all_hosts
        .iter()
        .map(|(key, values)| {
            let timing = values
                .iter()
                .filter(|((id, _), _)| is_timing(id))
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            (key.clone(), timing)
        })
        .filter(|(_, values): &(_, BTreeMap<_, _>)| !values.is_empty())
        .collect();
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
    let bench_runs: Vec<_> = all_hosts
        .iter()
        .filter(|(_, values)| values.keys().any(|(id, _)| is_bench(id)))
        .collect();
    if !bench_runs.is_empty() {
        out.push_str(
            "\n## Against plain Rust on the dedicated host\n\nThe native benchmark of `ignite-perf quality-remote` (`bench/native/`, `docs/quality.md` row 3.3):\none row per run and cell, each implementation's median over the run's passes against the plain\nRust reference measured in the same pass (above 1 means more replies per second, a longer p99, more\npeak RSS or a larger stripped binary than plain Rust).\n\n| run | commit | date | host | cell | Ignite LLVM / Rust | Ignite Rust target / Rust | Go / Rust |\n|---|---|---|---|---|---|---|---|\n",
        );
        for ((run, commit, date, host), values) in bench_runs {
            let cells: BTreeSet<&str> = values
                .keys()
                .filter(|(id, _)| is_bench(id))
                .filter_map(|(_, target)| target.split_once('/').map(|(_, rest)| rest))
                .collect();
            for cell in cells {
                let column = |consumer: &str| {
                    BENCH
                        .iter()
                        .filter_map(|id| {
                            values
                                .get(&(id.to_string(), format!("{consumer}/{cell}")))
                                .map(|v| format!("{} {}", bench_label(id), shown(*v, "permille")))
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                out.push_str(&format!(
                    "| {run} | {} | {date} | {host} | {cell} | {} | {} | {} |\n",
                    &commit[..commit.len().min(8)],
                    column("llvm"),
                    column("rust"),
                    column("go")
                ));
            }
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
gated metric worse, and an improvement is written into the bar in the same change, so the bar only\n\
moves one way. A release measures every row against the release before it; a row that is worse is\n\
tracked: the release notes list it with the story that takes it. The list, its kinds and its\n\
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
they show the shared workstation's values (`host: local`), recorded but not comparable.\n\n\
Throughput, p99 latency, peak RSS and stripped binary size of the todo service and the terminal\n\
server (3.3, 3.7 to 3.9) are ratios against a plain-Rust implementation of the same program,\n\
measured on that host in the same run (`bench/native/`); a Go implementation is shown as a\n\
reference without a bar.\n\n",
    );
    let sizes: Vec<&str> = baseline
        .metrics
        .iter()
        .filter(|(_, m)| m.grows_by_decision)
        .map(|(id, _)| id.as_str())
        .collect();
    if !sizes.is_empty() {
        out.push_str(&format!(
            "The binary-size rows ({}) are the one exception for a release, marked `release_growth` in\n\
`quality/baseline.json`: a release that adds code may leave a cell of them above the release before\n\
it when the cell's bar carries a new decision naming the code that added the bytes, and the release\n\
run lists every such cell. Without that decision, or with one the release before it already held,\n\
the cell is listed as worse. A merge holds these rows to their bars like any other, and bytes are\n\
never bought with time.\n\n",
            sizes.join(", ")
        ));
    }
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
                if let Some(at) = on_host.filter(|_| is_timing(id) || is_bench(id)) {
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
            if !is_bench(id) {
                continue;
            }
            let Some(((_, _, _, host), values)) = hosts
                .iter()
                .rev()
                .find(|(_, v)| v.keys().any(|(i, _)| i == id))
            else {
                continue;
            };
            for ((_, target), value) in values.iter().filter(|((i, t), _)| i == id && !barred(t)) {
                out.push_str(&format!(
                    "| {id} | {} | {} | {target} | reference, no bar | {} | {host} |\n",
                    m.name,
                    m.kind,
                    shown(*value, &m.unit)
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

/// The release check against the last release that was published:
/// `last_release` is its tag (the nearest `v*` tag reachable from HEAD,
/// [`last_release_tag`]), and its reference is the history's last local run carrying that label.
/// Without a readable tag the `bar` run is the reference only when the
/// history holds no `v*` run; otherwise the release is refused with the
/// lookup's reason and `git fetch --tags`. A record under any
/// other `v*` label (a candidate the gate refused, the candidate itself) is
/// never the reference, and a tag without a record in the history is a
/// refusal naming the tag, not a pass.
///
/// A value worse than the reference by any amount is refused. Every row it
/// judges is deterministic (a count, or a size such as 3.2), so its
/// tolerance here is 0, whatever the metric's tolerance for a merge, and a
/// hill-climb row is judged like any other (a merge only records it). No
/// `decision` holds a row for a release (`AGENTS.md` § Releases): a bar a
/// decision moved worse than the reference refuses the release, whatever
/// the value, until the decision is withdrawn, and the refusal names it.
/// Timing and benchmark values are left out here: `ignite-perf
/// quality-remote` refuses them against BASE on the dedicated host. Their
/// bars are judged here against the bars in the baseline at the tag (`git
/// show <tag>:quality/baseline.json`): a bar a decision moved worse than the
/// tag's refuses the release ([`decided_against_the_tag`]).
///
/// The size rows are the one exception (`"release_growth": "decision"` in
/// the baseline, [`SIZE`]; `AGENTS.md` § Releases, 2026-11-11: a binary may
/// grow when a release adds code). A cell of one above the reference is not
/// refused when its bar carries a new `decision`, which names the code that
/// added the bytes: it is listed ([`Release::grown`], [`grown_section`]).
/// Without a decision on its bar it is refused, and the refusal says the
/// decision is missing. A decision is new when the baseline at the last
/// release's tag does not hold it, on that bar or anywhere else
/// ([`decisions_of`]), read by its letters and digits ([`read_decision`]):
/// one it holds named the last release's code and does not let a cell grow
/// again, reworded or with a sentence added, and the refusal asks for a new
/// one; a text with fewer than [`DECISION_LETTERS`] letters and digits is
/// no decision. A tag whose baseline cannot be read refuses the grown cell,
/// since its decision is not known to be new; before the first release
/// (the `bar` run the reference) there is none to carry over. A decision on
/// the bar of a size cell that is not above the reference holds nothing
/// and is refused until it is removed, so one written for a candidate that
/// was not cut does not hold a later growth. The two checks of a bar a
/// decision moved worse skip these rows.
pub fn release_refusals(
    baseline: &Baseline,
    history: &[Record],
    values: &[Value],
    last_release: impl Into<LastRelease>,
) -> Vec<String> {
    release_judged(baseline, history, values, last_release).refused
}

/// The baseline at the revision `rev`, when it can be read.
fn baseline_at(rev: &str) -> Option<Baseline> {
    read_baseline(&Json::parse(&baseline_text_at(rev)?).ok()?).ok()
}

/// [`release_refusals`] with the baseline at the last release's tag given
/// (`None` when it cannot be read).
pub fn release_refusals_against(
    baseline: &Baseline,
    at_tag: Option<&Baseline>,
    history: &[Record],
    values: &[Value],
    last_release: impl Into<LastRelease>,
) -> Vec<String> {
    release_judged_against(baseline, at_tag, history, values, last_release).refused
}

/// One cell of a size row above the last release that the decision on its
/// bar holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Grown {
    pub metric: String,
    pub target: String,
    /// The value at the last release.
    pub then: i64,
    pub now: i64,
    pub unit: String,
    pub decision: String,
}

/// What the release check found: every row worse than the last release
/// (`refused`, the name of the rule before 2026-11-11; [`record`] prints
/// them as tracked), and every grown cell of a size row a decision holds.
/// `unjudged` is set when the check had no reference to compare with (the
/// last release's tag unreadable, or no record of it in the history): then
/// `refused` says why, and that still stops the release.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Release {
    pub refused: Vec<String>,
    pub grown: Vec<Grown>,
    pub unjudged: bool,
}

/// The section of a release run's output that lists `grown`, one line per
/// cell: the cell, its value at the last release, its value now and the
/// decision on its bar. Empty when no cell grew.
pub fn grown_section(run: &str, grown: &[Grown]) -> String {
    if grown.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "release {run}: {} of the size rows above the last release, each held by the decision on its bar:\n",
        match grown.len() {
            1 => "1 cell".to_string(),
            n => format!("{n} cells"),
        }
    );
    for cell in grown {
        out.push_str(&format!(
            "  {} {}: {} at the last release, {} now; decision: {}\n",
            cell.metric,
            cell.target,
            shown(cell.then, &cell.unit),
            shown(cell.now, &cell.unit),
            cell.decision
        ));
    }
    out
}

/// [`release_judged_against`] with the baseline at the last release's tag
/// read from git.
pub fn release_judged(
    baseline: &Baseline,
    history: &[Record],
    values: &[Value],
    last_release: impl Into<LastRelease>,
) -> Release {
    let last_release = last_release.into();
    let at_tag = match &last_release {
        LastRelease::Tag(tag) => baseline_at(tag),
        _ => None,
    };
    release_judged_against(baseline, at_tag.as_ref(), history, values, last_release)
}

/// The release check ([`release_refusals`]) with the baseline at the last
/// release's tag given (`None` when it cannot be read).
pub fn release_judged_against(
    baseline: &Baseline,
    at_tag: Option<&Baseline>,
    history: &[Record],
    values: &[Value],
    last_release: impl Into<LastRelease>,
) -> Release {
    let refuse = |refused: Vec<String>| Release {
        refused,
        grown: Vec::new(),
        unjudged: true,
    };
    let last_release = last_release.into();
    let released = history.iter().any(|r| r.run.starts_with('v'));
    let label = match &last_release {
        LastRelease::Tag(tag) => tag.as_str(),
        LastRelease::NoTag | LastRelease::Unreadable(_) if released => {
            let why = match &last_release {
                LastRelease::Unreadable(why) => why.as_str(),
                _ => "no `v*` tag is reachable from HEAD",
            };
            return refuse(vec![format!(
                "the last release's tag could not be read ({why}), and quality/history.jsonl holds a release run: run `git fetch --tags` and the release check again"
            )]);
        }
        LastRelease::NoTag | LastRelease::Unreadable(_) => "bar",
    };
    let Some((_, last)) = runs(history)
        .into_iter()
        .rev()
        .find(|((run, _, _), _)| run == label)
    else {
        return refuse(match &last_release {
            LastRelease::Tag(tag) => vec![format!(
                "the last release {tag} has no record in quality/history.jsonl, so the release has no reference"
            )],
            _ => Vec::new(),
        });
    };
    let (mut refused, mut grown) = (Vec::new(), Vec::new());
    for v in values {
        let (Some(metric), Some(then)) = (
            baseline.metrics.get(&v.metric),
            last.get(&(v.metric.clone(), v.target.clone())),
        ) else {
            continue;
        };
        if is_timing(&v.metric) {
            continue;
        }
        let worse = metric.direction.worse(*then, v.value, Tolerance::Count(0));
        if metric.grows_by_decision {
            let decision = metric
                .bars
                .get(&v.target)
                .and_then(|bar| bar.decision.as_deref())
                .map(str::trim)
                .filter(|decision| !decision.is_empty());
            if !worse {
                if let Some(decision) = decision {
                    refused.push(format!(
                        "{} {}: {} at the last release, {} now; the decision `{decision}` on its bar holds nothing, the cell is not above the last release: remove it",
                        v.metric, v.target, then, v.value
                    ));
                }
                continue;
            }
            let cell = format!(
                "{} {}: {} at the last release, {} now ({}, any worse value)",
                v.metric,
                v.target,
                then,
                v.value,
                metric.direction.word()
            );
            let Some(decision) = decision else {
                refused.push(format!(
                    "{cell}; a size row grows at a release only by a `decision` on the bar of the grown cell, and a `decision` naming the added code is missing"
                ));
                continue;
            };
            // A text that names nothing is no decision whatever the last
            // release held, so it is read before the tag's baseline is.
            let read = match read_decision(decision, &BTreeSet::new()) {
                DecisionRead::NamesNothing => DecisionRead::NamesNothing,
                _ => match (&last_release, at_tag) {
                    (LastRelease::Tag(tag), None) => {
                        refused.push(format!(
                            "{cell}; the baseline at the last release {tag} could not be read, so the decision `{decision}` is not known to be new: run `git fetch --tags`"
                        ));
                        continue;
                    }
                    (LastRelease::Tag(_), Some(at_tag)) => {
                        read_decision(decision, &decisions_of(at_tag))
                    }
                    _ => DecisionRead::New,
                },
            };
            match read {
                DecisionRead::NamesNothing => refused.push(format!(
                    "{cell}; the `decision` on its bar (`{decision}`) has fewer than {DECISION_LETTERS} letters and digits and names nothing: a size row grows at a release only by a `decision` on the bar of the grown cell, and a `decision` naming the added code is missing"
                )),
                DecisionRead::LastReleases => refused.push(format!(
                    "{cell}; the decision `{decision}` is the last release's (the baseline at {} holds it, read by its letters and digits), and a new `decision` naming the added code is needed",
                    match &last_release {
                        LastRelease::Tag(tag) => tag.as_str(),
                        _ => "the last release",
                    }
                )),
                DecisionRead::New => grown.push(Grown {
                    metric: v.metric.clone(),
                    target: v.target.clone(),
                    then: *then,
                    now: v.value,
                    unit: metric.unit.clone(),
                    decision: decision.to_string(),
                }),
            }
            continue;
        }
        let decided = metric.bars.get(&v.target).and_then(|bar| {
            let decision = bar.decision.as_deref()?.trim();
            (!decision.is_empty()
                && metric
                    .direction
                    .worse(*then, bar.value, Tolerance::Count(0)))
            .then(|| {
                format!(
                    "; the decision `{decision}` moved the bar to {} and holds no row for a release: withdraw it",
                    bar.value
                )
            })
        });
        if !worse && decided.is_none() {
            continue;
        }
        let judged = if worse {
            format!(" ({}, any worse value)", metric.direction.word())
        } else {
            String::new()
        };
        refused.push(format!(
            "{} {}: {} at the last release, {} now{judged}{}",
            v.metric,
            v.target,
            then,
            v.value,
            decided.unwrap_or_default()
        ));
    }
    if let LastRelease::Tag(tag) = &last_release {
        refused.extend(decided_against_the_tag(baseline, at_tag, tag));
    }
    Release {
        refused,
        grown,
        unjudged: false,
    }
}

/// The timing and benchmark bars a `decision` moved worse than the bar of
/// the same row in `at_tag`, the baseline at the last release's `tag`: these
/// rows are measured only on the dedicated host, so their bars are the
/// reference here. A row without a bar at the tag refuses nothing; a tag
/// whose baseline cannot be read while such a decided bar stands is one
/// refusal. The size row among them (3.9) is left out: a decision on its
/// bar is what lets it grow, and `ignite-perf quality-remote` judges and
/// lists its cells.
fn decided_against_the_tag(
    baseline: &Baseline,
    at_tag: Option<&Baseline>,
    tag: &str,
) -> Vec<String> {
    let decided = baseline
        .metrics
        .iter()
        .filter(|(id, metric)| (is_timing(id) || is_bench(id)) && !metric.grows_by_decision)
        .flat_map(|(id, metric)| {
            metric.bars.iter().filter_map(move |(target, bar)| {
                let decision = bar.decision.as_deref()?.trim();
                (!decision.is_empty()).then_some((id, metric, target, bar.value, decision))
            })
        });
    let Some(at_tag) = at_tag else {
        return match decided.count() {
            0 => Vec::new(),
            n => vec![format!(
                "the baseline at the last release {tag} could not be read, so {n} timing and benchmark bars a decision holds are not compared: run `git fetch --tags`"
            )],
        };
    };
    decided
        .filter_map(|(id, metric, target, now, decision)| {
            let then = at_tag.metrics.get(id)?.bars.get(target)?.value;
            metric
                .direction
                .worse(then, now, Tolerance::Count(0))
                .then(|| {
                    format!(
                        "{id} {target}: bar {then} at {tag}, {now} now by the decision `{decision}`, which holds no row for a release: withdraw it"
                    )
                })
        })
        .collect()
}

/// What [`record`] judges a run by and prints for it: for a release
/// (`run` is `vX.Y.Z`) the release check against `last_release`, with the
/// baseline at its tag read from git ([`release_judged`]), and the list of
/// the grown size cells a decision holds ([`grown_section`]) as the text
/// `record` writes to stderr, refused or not; for any other run nothing.
pub fn run_judged(
    run: &str,
    baseline: &Baseline,
    history: &[Record],
    values: &[Value],
    last_release: LastRelease,
) -> (Release, String) {
    if !run.starts_with('v') {
        return (Release::default(), String::new());
    }
    let release = release_judged(baseline, history, values, last_release);
    let listed = grown_section(run, &release.grown);
    (release, listed)
}

/// What [`record`] writes to stderr for the rows of the release `run` that
/// the release check finds worse than at the last release (`worse`, the
/// `refused` of [`Release`]): they are tracked and refuse no release (the
/// operator, 2026-11-11: "track the numbers, but let's not block the
/// release"); the release notes list each with the story that takes it.
/// Nothing for an empty list.
pub fn tracked_section(run: &str, worse: &[String]) -> String {
    if worse.is_empty() {
        return String::new();
    }
    format!(
        "release {run}: worse than the last release, tracked and not refused:\n  {}\n",
        worse.join("\n  ")
    )
}

#[test]
fn a_release_prints_its_worse_rows_as_tracked() {
    assert_eq!(tracked_section("v0.16.0", &[]), "");
    let worse = [
        "3.7 llvm/terminal/connect-8: 1.099 at the last release, 1.282 now".to_string(),
        "1.2 eval: 770 at the last release, 771 now".to_string(),
    ];
    assert_eq!(
        tracked_section("v0.16.0", &worse),
        "release v0.16.0: worse than the last release, tracked and not refused:\n  3.7 llvm/terminal/connect-8: 1.099 at the last release, 1.282 now\n  1.2 eval: 770 at the last release, 771 now\n"
    );
}

/// The records [`record`] appends to the history for the run `label`: one
/// per value. Under a release's label the record of a cell in `grown`
/// carries the decision that holds it, which the release's row of the trend
/// is generated from.
pub fn run_records(
    values: &[Value],
    grown: &[Grown],
    commit: &str,
    date: &str,
    label: &str,
    host: &str,
) -> Vec<Record> {
    let held = |v: &Value| {
        grown
            .iter()
            .find(|cell| {
                label.starts_with('v') && cell.metric == v.metric && cell.target == v.target
            })
            .map(|cell| cell.decision.clone())
    };
    values
        .iter()
        .map(|v| Record {
            commit: commit.to_string(),
            date: date.to_string(),
            run: label.to_string(),
            metric: v.metric.clone(),
            target: v.target.clone(),
            value: v.value,
            host: host.to_string(),
            load: v.load.clone(),
            base: None,
            base_value: None,
            machine: None,
            decision: held(v),
        })
        .collect()
}

/// `task quality`: read every source's measured values from
/// `target/quality/` (the gate tests and `ignite-perf quality` wrote them),
/// print the rows of a release (`IGNITE_QUALITY_RUN=vX.Y.Z`) that are worse
/// than at the last ([`tracked_section`]: tracked, the release is not refused
/// by them), print the cells of a size row a decision lets grow
/// ([`grown_section`]), append one record per metric and target to
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
    for source in SOURCES.iter().filter(|s| **s != "bench") {
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
    let (
        Release {
            refused,
            grown,
            unjudged,
        },
        listed,
    ) = run_judged(&run, &baseline, &history, &values, last_release_tag(&run));
    // Rows worse than the last release are tracked. A check that compared
    // nothing has no rows: what it says stops the release.
    let (worse, stops) = if unjudged {
        (Vec::new(), refused)
    } else {
        (refused, Vec::new())
    };
    // Written to stderr itself, not with `eprintln!`: the harness captures
    // that of a passing test, and a release that passes still shows its
    // grown cells.
    std::io::stderr().write_all(listed.as_bytes()).unwrap();
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
    std::io::stderr()
        .write_all(tracked_section(&run, &worse).as_bytes())
        .unwrap();
    let label = if stops.is_empty() {
        run.clone()
    } else {
        "run".to_string()
    };
    let mut text = std::fs::read_to_string(history_path()).unwrap_or_default();
    for record in run_records(&values, &grown, &commit, &date, &label, &host) {
        text.push_str(&record.line());
        text.push('\n');
    }
    std::fs::write(history_path(), text).unwrap();
    write_pages(&baseline, &read_history());
    errors.extend(
        stops
            .iter()
            .map(|e| format!("release {run} was compared with no release: {e}")),
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
        let bench_labels = metric.source.as_deref() == Some("bench");
        assert!(
            metric.source.is_none() || bench_labels == is_bench(id),
            "{id}: the benchmark rows are exactly {BENCH:?}, labelled `source: bench`"
        );
        if bench_labels {
            assert_eq!(metric.unit, "permille", "{id}: a benchmark row is a ratio");
        }
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
    for line in std::fs::read_to_string(workspace().join("quality/bench.jsonl"))
        .unwrap_or_default()
        .lines()
    {
        let record = parse(line, "quality/bench.jsonl");
        assert_eq!(
            record.compact(),
            line,
            "a quality/bench.jsonl line not in canonical form"
        );
        for key in ["commit", "host", "implementation", "program", "cell"] {
            assert!(
                record.get(key).and_then(Json::as_str).is_some(),
                "{line}: no `{key}`"
            );
        }
    }
}

/// The generated texts are the ones the baseline and history give.
#[test]
fn trend_and_docs_are_generated_from_quality() {
    let mut baseline = load_baseline();
    let history = read_history();
    assert!(
        history.iter().any(Record::is_release),
        "quality/history.jsonl holds no first assessment (`bar`)"
    );
    if blessing() {
        baseline = bless_bench(&baseline, &history);
        save_baseline(&baseline);
        write_pages(&baseline, &history);
    }
    let errors = bench_bars(&baseline, &history);
    assert!(
        errors.is_empty(),
        "the benchmark's bars do not hold the latest dedicated-host run:\n  {}",
        errors.join("\n  ")
    );
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
        grows_by_decision: false,
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
        intermittent_decisions: BTreeMap::new(),
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
        decision: None,
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
    assert!(
        release_refusals(
            &baseline,
            &history,
            &[value("4.3", "rust", 312)],
            Some("v0.14.0")
        )
        .is_empty()
    );
    let refused = release_refusals(
        &baseline,
        &history,
        &[value("4.3", "rust", 310)],
        Some("v0.14.0"),
    );
    assert!(
        refused.len() == 1 && refused[0].contains("312 at the last release"),
        "{refused:?}"
    );
    let held = check(&baseline, "perf", &one("6.2", "llvm", 9000), None);
    assert!(
        held.is_empty(),
        "a timing row on host local was held: {held:?}"
    );
    let timed = release_refusals(
        &baseline,
        &history,
        &[value("6.2", "llvm", 9000)],
        Some("v0.14.0"),
    );
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

/// No `decision` holds a row for a release: a count or size row worse than
/// at the last release is refused by any amount (tolerance 0 for a
/// deterministic value, 3.2's 5 % included), whether or not a decision moved
/// its bar since, and the refusal names the decision so the reader sees what
/// to withdraw. The per-merge rule is unchanged: a merge may still move a bar
/// worse with a decision of its own.
#[test]
fn a_release_refuses_every_row_worse_than_the_last_release() {
    let decision = "story:x (2026-11-08): two more gaps";
    let size_target = "llvm/todod/release";
    let size = |bar: i64| {
        let mut m = metric(
            Direction::Lower,
            Tolerance::Percent(5),
            &[(size_target, bar)],
        );
        m.source = Some("perf".to_string());
        m.unit = "B".to_string();
        m
    };
    let at_last = baseline_of(&[
        ("3.2", size(1000)),
        (
            "4.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
        ),
    ]);
    let mut decided = baseline_of(&[
        ("3.2", size(1040)),
        (
            "4.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("rust", 2)]),
        ),
    ]);
    for (id, target) in [("3.2", size_target), ("4.2", "rust")] {
        decided
            .metrics
            .get_mut(id)
            .unwrap()
            .bars
            .get_mut(target)
            .unwrap()
            .decision = Some(decision.to_string());
    }
    let record = |metric: &str, target: &str, value: i64| Record {
        commit: "c".to_string(),
        date: "2026-11-07".to_string(),
        run: "v0.15.0".to_string(),
        metric: metric.to_string(),
        target: target.to_string(),
        value,
        host: "local".to_string(),
        load: None,
        base: None,
        base_value: None,
        machine: None,
        decision: None,
    };
    let history = vec![record("3.2", size_target, 1000), record("4.2", "rust", 1)];
    let value = |metric: &str, target: &str, value: i64| Value {
        metric: metric.to_string(),
        target: target.to_string(),
        value,
        load: None,
    };

    let merge = check(&decided, "static", &one("4.2", "rust", 2), Some(&at_last));
    assert!(merge.is_empty(), "a merge's decided bar: {merge:?}");
    let merge = check(
        &decided,
        "perf",
        &one("3.2", size_target, 1040),
        Some(&at_last),
    );
    assert!(merge.is_empty(), "a merge's decided bar: {merge:?}");

    let refused = release_refusals(
        &decided,
        &history,
        &[value("3.2", size_target, 1040), value("4.2", "rust", 2)],
        Some("v0.15.0"),
    );
    assert!(
        refused.len() == 2
            && refused[0].contains("1000 at the last release, 1040 now")
            && refused[1].contains("1 at the last release, 2 now")
            && refused.iter().all(|r| r.contains(decision)),
        "a row a decision moved worse was not refused, or the refusal does not name it: {refused:?}"
    );

    let refused = release_refusals(
        &at_last,
        &history,
        &[value("3.2", size_target, 1001)],
        Some("v0.15.0"),
    );
    assert!(
        refused.len() == 1
            && refused[0].contains("1000 at the last release, 1001 now")
            && !refused[0].contains("decision"),
        "a size one byte worse, inside its 5 %, was not refused: {refused:?}"
    );
    for (size, gaps) in [(1000, 1), (999, 0)] {
        let refused = release_refusals(
            &at_last,
            &history,
            &[value("3.2", size_target, size), value("4.2", "rust", gaps)],
            Some("v0.15.0"),
        );
        assert!(refused.is_empty(), "{refused:?}");
    }
}

/// `AGENTS.md` § Releases: "A bar a `decision` made worse since the last
/// release tag blocks the release until the value is back and the decision
/// is withdrawn." A size back at the last release's value, inside its 5 %
/// of the decided bar (so the merge check asks for no bless), with the
/// decision still in the baseline, is refused, and the refusal names the
/// decision to withdraw.
#[test]
fn a_release_waits_until_a_decision_that_moved_a_bar_worse_is_withdrawn() {
    let decision = "story:x (2026-11-08): dev-size bars moved";
    let target = "llvm/todod/release";
    let mut size = metric(Direction::Lower, Tolerance::Percent(5), &[(target, 1040)]);
    size.source = Some("perf".to_string());
    size.unit = "B".to_string();
    size.bars.get_mut(target).unwrap().decision = Some(decision.to_string());
    let decided = baseline_of(&[("3.2", size)]);
    let merge = check(&decided, "perf", &one("3.2", target, 1000), None);
    assert!(merge.is_empty(), "the merge check refused: {merge:?}");
    let history = vec![Record {
        commit: "c".to_string(),
        date: "2026-11-07".to_string(),
        run: "v0.15.0".to_string(),
        metric: "3.2".to_string(),
        target: target.to_string(),
        value: 1000,
        host: "local".to_string(),
        load: None,
        base: None,
        base_value: None,
        machine: None,
        decision: None,
    }];
    let refused = release_refusals(
        &decided,
        &history,
        &[Value {
            metric: "3.2".to_string(),
            target: target.to_string(),
            value: 1000,
            load: None,
        }],
        Some("v0.15.0"),
    );
    assert!(
        refused.len() == 1 && refused[0].contains(decision),
        "a decision that moved a bar worse than the last release is still in the baseline, and the release was not refused: {refused:?}"
    );
}

/// The cell the size-row tests judge.
const SIZE_CELL: &str = "rust/todod/release";

/// Row 3.2 with one bar on [`SIZE_CELL`]; `marked` is the baseline's
/// `"release_growth": "decision"`.
fn size_row(marked: bool, bar: i64, decision: Option<&str>) -> Metric {
    let mut m = metric(Direction::Lower, Tolerance::Percent(5), &[(SIZE_CELL, bar)]);
    m.source = Some("perf".to_string());
    m.unit = "bytes".to_string();
    m.grows_by_decision = marked;
    m.bars.get_mut(SIZE_CELL).unwrap().decision = decision.map(str::to_string);
    m
}

/// The last release's record of 3.2 on [`SIZE_CELL`], and a value of it now.
fn size_at_the_last_release(value: i64) -> Vec<Record> {
    let mut record = release_record("v0.15.0", "b", value);
    record.metric = "3.2".to_string();
    record.target = SIZE_CELL.to_string();
    vec![record]
}

fn size_now(value: i64) -> Value {
    Value {
        metric: "3.2".to_string(),
        target: SIZE_CELL.to_string(),
        value,
        load: None,
    }
}

/// `AGENTS.md` § Releases (2026-11-11): time comes before bytes, and a
/// binary may grow when a release adds code. A cell of a size row
/// (`"release_growth": "decision"`) above the last release is not refused
/// when its bar carries a new `decision`, one the baseline at the last
/// release's tag does not hold (its bar there had none, or another); it is
/// listed with the value at the last release, the value now and the
/// decision, and the release run's section holds the same.
#[test]
fn a_grown_cell_of_a_size_row_passes_by_a_decision_and_is_listed() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let baseline = baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]);
    let earlier = baseline_of(&[(
        "3.2",
        size_row(
            true,
            1000,
            Some("story:w (2026-11-08): the scheduler's run queue adds the bytes"),
        ),
    )]);
    let passed = release_refusals_against(
        &baseline,
        Some(&earlier),
        &size_at_the_last_release(1000),
        &[size_now(1040)],
        Some("v0.15.0"),
    );
    assert!(
        passed.is_empty(),
        "a new decision on a bar that held another at the last release was refused: {passed:?}"
    );
    let at_tag = baseline_of(&[("3.2", size_row(true, 1000, None))]);
    let judged = release_judged_against(
        &baseline,
        Some(&at_tag),
        &size_at_the_last_release(1000),
        &[size_now(1040)],
        Some("v0.15.0"),
    );
    assert!(
        judged.refused.is_empty(),
        "a size row grown by a decision was refused: {:?}",
        judged.refused
    );
    assert_eq!(
        judged.grown,
        vec![Grown {
            metric: "3.2".to_string(),
            target: SIZE_CELL.to_string(),
            then: 1000,
            now: 1040,
            unit: "bytes".to_string(),
            decision: decision.to_string(),
        }],
        "the grown cell is not listed"
    );
    assert!(
        release_refusals_against(
            &baseline,
            Some(&at_tag),
            &size_at_the_last_release(1000),
            &[size_now(1001)],
            Some("v0.15.0"),
        )
        .is_empty(),
        "one byte above the last release, by a decision, was refused"
    );
    let section = grown_section("v0.16.0", &judged.grown);
    assert!(
        section.contains("v0.16.0")
            && section.contains("3.2 rust/todod/release: 1000 B at the last release, 1040 B now")
            && section.contains(decision),
        "the release run's section does not hold the cell, both values and the decision: {section:?}"
    );
    assert_eq!(
        grown_section("v0.16.0", &[]),
        "",
        "a release without a grown cell prints a section"
    );
}

/// A decision is taken for the code one release adds. One the baseline at
/// the last release's tag already holds names that release's code, so it
/// does not let a cell grow again: the cell is refused, and the refusal
/// says the decision is the last release's and a new one naming the added
/// code is needed. Held is held anywhere in that baseline, trimmed, as a
/// merge's `decision` of its own is ([`new_decision`]): on the same bar, on
/// another bar of the row, on another row.
#[test]
fn a_grown_cell_held_by_the_last_releases_decision_is_refused() {
    let decision = "story:w (2026-11-08): the scheduler's run queue adds the bytes";
    let same_bar = baseline_of(&[("3.2", size_row(true, 1000, Some(decision)))]);
    let mut other_bar = size_row(true, 1000, None);
    other_bar.bars.insert(
        "llvm/todod/release".to_string(),
        Bar {
            value: 2000,
            decision: Some(decision.to_string()),
        },
    );
    let other_bar = baseline_of(&[("3.2", other_bar)]);
    let mut gaps_row = metric(Direction::Lower, Tolerance::Count(0), &[("rust", 2)]);
    gaps_row.bars.get_mut("rust").unwrap().decision = Some(decision.to_string());
    let other_row = baseline_of(&[("3.2", size_row(true, 1000, None)), ("4.2", gaps_row)]);
    for (at_tag, now_decision) in [
        (&same_bar, decision.to_string()),
        (&same_bar, format!("{decision} ")),
        (&other_bar, decision.to_string()),
        (&other_row, decision.to_string()),
    ] {
        let baseline = baseline_of(&[("3.2", size_row(true, 1040, Some(&now_decision)))]);
        let judged = release_judged_against(
            &baseline,
            Some(at_tag),
            &size_at_the_last_release(1000),
            &[size_now(1040)],
            Some("v0.15.0"),
        );
        assert!(
            judged.refused.len() == 1
                && judged.refused[0]
                    .starts_with("3.2 rust/todod/release: 1000 at the last release, 1040 now")
                && judged.refused[0].contains(decision)
                && judged.refused[0].contains("is the last release's")
                && judged.refused[0].contains("v0.15.0")
                && judged.refused[0].contains("a new `decision` naming the added code is needed"),
            "decision {now_decision:?} carried over from the last release: {:?}",
            judged.refused
        );
        assert!(judged.grown.is_empty(), "{:?}", judged.grown);
    }
}

/// Whether a decision is new is read from the baseline at the last
/// release's tag. When that cannot be read, a grown cell is refused, not
/// passed, and the refusal says so and names `git fetch --tags`. Before the
/// first release (no tag, the `bar` run the reference) there is no release
/// whose decision could be carried over, and the cell passes.
#[test]
fn a_grown_cell_is_refused_when_the_baseline_at_the_tag_cannot_be_read() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let baseline = baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]);
    let judged = release_judged_against(
        &baseline,
        None,
        &size_at_the_last_release(1000),
        &[size_now(1040)],
        Some("v0.15.0"),
    );
    assert!(
        judged.refused.len() == 1
            && judged.refused[0]
                .starts_with("3.2 rust/todod/release: 1000 at the last release, 1040 now")
            && judged.refused[0]
                .contains("the baseline at the last release v0.15.0 could not be read")
            && judged.refused[0].contains(decision)
            && judged.refused[0].contains("git fetch --tags"),
        "the tag's baseline unread: {:?}",
        judged.refused
    );
    assert!(judged.grown.is_empty(), "{:?}", judged.grown);

    let mut first = size_at_the_last_release(1000);
    first[0].run = "bar".to_string();
    let judged = release_judged_against(
        &baseline,
        None,
        &first,
        &[size_now(1040)],
        LastRelease::NoTag,
    );
    assert!(
        judged.refused.is_empty() && judged.grown.len() == 1,
        "before the first release: {judged:?}"
    );
}

/// The release check reads the baseline at the last release's tag with the
/// reader of this tree, and a baseline it cannot read refuses every grown
/// cell of a size row. Where the tag and its baseline are in the clone, the
/// reader reads it; a clone without the tag has nothing to read here.
#[test]
fn the_baseline_at_the_last_release_is_one_this_reader_reads() {
    let LastRelease::Tag(tag) = last_release_tag("the candidate") else {
        return;
    };
    let Some(text) = baseline_text_at(&tag) else {
        return;
    };
    let read = Json::parse(&text)
        .map_err(|e| e.to_string())
        .and_then(|json| read_baseline(&json));
    assert!(
        read.is_ok(),
        "quality/baseline.json at {tag} is not read by this tree's reader, so a release would refuse every grown size cell: {:?}",
        read.err()
    );
}

/// Growth stays deliberate: a cell of a size row above the last release
/// whose bar has no `decision` (or a blank one) is refused, by any amount,
/// and the refusal says that a decision naming the added code is missing.
#[test]
fn a_grown_cell_of_a_size_row_without_a_decision_is_refused() {
    for (decision, now) in [(None, 1040), (Some("  "), 1040), (None, 1001)] {
        let baseline = baseline_of(&[("3.2", size_row(true, 1040, decision))]);
        let judged = release_judged_against(
            &baseline,
            None,
            &size_at_the_last_release(1000),
            &[size_now(now)],
            Some("v0.15.0"),
        );
        assert!(
            judged.refused.len() == 1
                && judged.refused[0].starts_with(&format!(
                    "3.2 rust/todod/release: 1000 at the last release, {now} now"
                ))
                && judged.refused[0].contains("a `decision` naming the added code is missing"),
            "decision {decision:?}, {now} now: {:?}",
            judged.refused
        );
        assert!(judged.grown.is_empty(), "{:?}", judged.grown);
    }
    let unbarred = Value {
        target: "rust/terminald/release".to_string(),
        ..size_now(1040)
    };
    let mut history = size_at_the_last_release(1000);
    history[0].target = unbarred.target.clone();
    let judged = release_judged_against(
        &baseline_of(&[("3.2", size_row(true, 1040, Some("story:x: another cell")))]),
        None,
        &history,
        &[unbarred],
        Some("v0.15.0"),
    );
    assert!(
        judged.refused.len() == 1
            && judged.refused[0].contains("a `decision` naming the added code is missing")
            && judged.grown.is_empty(),
        "a grown cell without a bar passed on another cell's decision: {judged:?}"
    );
}

/// The rule is for size rows only: a row the baseline does not mark is
/// judged as before, so a `decision` holds no worse value of it, the same
/// bytes and the same decision included, and it is listed nowhere.
#[test]
fn a_row_that_is_no_size_row_is_refused_with_a_decision_as_before() {
    let decision = "story:x (2026-11-11): two more gaps";
    let mut gaps_row = metric(Direction::Lower, Tolerance::Count(0), &[("rust", 2)]);
    gaps_row.bars.get_mut("rust").unwrap().decision = Some(decision.to_string());
    let mut history = size_at_the_last_release(1000);
    history.push(release_record("v0.15.0", "b", 1));
    let baseline = baseline_of(&[
        ("3.2", size_row(false, 1040, Some(decision))),
        ("4.2", gaps_row),
    ]);
    let judged = release_judged_against(
        &baseline,
        None,
        &history,
        &[size_now(1040), gaps(2)],
        Some("v0.15.0"),
    );
    assert!(
        judged.refused.len() == 2
            && judged.refused[0].contains("1000 at the last release, 1040 now")
            && judged.refused[1].contains("1 at the last release, 2 now")
            && judged
                .refused
                .iter()
                .all(|r| r.contains(decision) && r.contains("withdraw it")),
        "a row that is no size row was held by a decision: {:?}",
        judged.refused
    );
    assert!(judged.grown.is_empty(), "{:?}", judged.grown);
}

/// A size row at or below the last release needs nothing: no decision, no
/// listing, no refusal.
#[test]
fn a_size_row_that_shrank_needs_no_decision() {
    for (bar, now) in [(1000, 990), (990, 990), (1000, 1000)] {
        let judged = release_judged_against(
            &baseline_of(&[("3.2", size_row(true, bar, None))]),
            None,
            &size_at_the_last_release(1000),
            &[size_now(now)],
            Some("v0.15.0"),
        );
        assert_eq!(
            judged,
            Release::default(),
            "bar {bar}, 1000 at the last release, {now} now"
        );
    }
}

/// The check that refuses a bar a `decision` moved worse than the last
/// release skips size rows, in both its forms: 3.2 against the last
/// release's value (a size back at that value with the decision still on a
/// higher bar is not asked to withdraw it as a moved bar; the size rule's
/// own refusal of a decision that holds nothing answers it), and 3.9
/// against its bar in the baseline at the tag. A benchmark row that is no
/// size row (3.8) is refused there as before, and only it counts when the
/// tag's baseline cannot be read.
#[test]
fn a_size_row_is_not_judged_by_the_moved_bar_checks() {
    let decision = "story:x (2026-11-11): the scheduler's bytes";
    let judged = release_judged_against(
        &baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]),
        None,
        &size_at_the_last_release(1000),
        &[size_now(1000)],
        Some("v0.15.0"),
    );
    assert!(
        judged.grown.is_empty()
            && judged.refused.len() == 1
            && judged.refused[0].contains("holds nothing")
            && !judged.refused[0].contains("withdraw it"),
        "a size back at the last release's value, its bar moved by a decision: {judged:?}"
    );
    let unmarked = release_refusals_against(
        &baseline_of(&[("3.2", size_row(false, 1040, Some(decision)))]),
        None,
        &size_at_the_last_release(1000),
        &[size_now(1000)],
        Some("v0.15.0"),
    );
    assert!(
        unmarked.len() == 1 && unmarked[0].contains("withdraw it"),
        "{unmarked:?}"
    );

    let ratio = |marked: bool, bar: i64, decision: Option<&str>| {
        let mut m = metric(
            Direction::Lower,
            Tolerance::Percent(5),
            &[("llvm/todo", bar)],
        );
        m.source = Some("bench".to_string());
        m.unit = "permille".to_string();
        m.grows_by_decision = marked;
        m.bars.get_mut("llvm/todo").unwrap().decision = decision.map(str::to_string);
        m
    };
    let history = vec![release_record("v0.15.0", "b", 1)];
    let at_tag = baseline_of(&[
        ("3.8", ratio(false, 720, None)),
        ("3.9", ratio(true, 720, None)),
    ]);
    let size_only = baseline_of(&[
        ("3.8", ratio(false, 720, None)),
        ("3.9", ratio(true, 869, Some(decision))),
    ]);
    for tag_baseline in [Some(&at_tag), None] {
        let refused =
            release_refusals_against(&size_only, tag_baseline, &history, &[], Some("v0.15.0"));
        assert!(
            refused.is_empty(),
            "the bar of 3.9 moved by a decision refused the release: {refused:?}"
        );
    }
    let both = baseline_of(&[
        ("3.8", ratio(false, 869, Some(decision))),
        ("3.9", ratio(true, 869, Some(decision))),
    ]);
    let refused = release_refusals_against(&both, Some(&at_tag), &history, &[], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].starts_with("3.8 llvm/todo: bar 720 at v0.15.0, 869 now"),
        "{refused:?}"
    );
    let refused = release_refusals_against(&both, None, &history, &[], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].contains("so 1 timing and benchmark bars"),
        "{refused:?}"
    );
}

/// One metric `id` as the baseline's JSON text, `release_growth` given as
/// JSON.
fn baseline_text_marked(id: &str, release_growth: &str) -> String {
    format!(
        r#"{{"about":"a","areas":["A"],"metrics":{{"{id}":{{"area":"A","name":"m","kind":"ratchet","direction":"lower","unit":"bytes","instrument":"i","source":"perf","tolerance":"5%","release_growth":{release_growth},"bars":{{"rust":{{"value":1}}}}}}}},"known_differences":{{}},"eval_cannot_run":{{}}}}"#
    )
}

/// The mark is one field of a metric, `"release_growth": "decision"`: read,
/// written back in the canonical form, and validated where the baseline is
/// read. Another value is an error, and so is the field on a row that is no
/// size row ([`SIZE`]): a timing row, a benchmark row other than 3.9, a
/// count.
#[test]
fn the_size_mark_is_read_written_and_validated() {
    for id in SIZE {
        let json = parse(&baseline_text_marked(id, r#""decision""#), "the baseline");
        let baseline = read_baseline(&json).unwrap();
        assert!(
            baseline.metrics[id].grows_by_decision,
            "{id}: the mark was not read"
        );
        let written = write_baseline(&baseline);
        assert!(
            written.contains("\"name\": \"m\",\n      \"release_growth\": \"decision\",\n"),
            "{id}: the mark was not written: {written}"
        );
        assert_eq!(
            read_baseline(&parse(&written, "the written baseline")).unwrap(),
            baseline
        );
    }
    let unmarked = baseline_of(&[("3.2", size_row(false, 1, None))]);
    assert!(!write_baseline(&unmarked).contains("release_growth"));
    for value in [r#""always""#, r#""""#, "true", "1", "null"] {
        let json = parse(&baseline_text_marked("3.2", value), "the baseline");
        let error = read_baseline(&json).unwrap_err();
        assert!(
            error.contains("3.2") && error.contains("`release_growth`") && error.contains(value),
            "release_growth {value}: {error}"
        );
    }
    for id in ["5.3", "6.1", "6.2", "3.3", "3.7", "3.8", "4.2"] {
        let json = parse(&baseline_text_marked(id, r#""decision""#), "the baseline");
        let error = read_baseline(&json).unwrap_err();
        assert!(
            error.contains(id) && error.contains("`release_growth`") && error.contains("3.2"),
            "{id} marked as a size row: {error}"
        );
    }
}

/// `quality/baseline.json` marks the size rows 3.2 and 3.9 and no other
/// row, and `docs/quality.md` and the book page say what the mark means.
#[test]
fn the_size_rows_are_marked_and_no_other_row_is() {
    let baseline = load_baseline();
    let marked: Vec<&str> = baseline
        .metrics
        .iter()
        .filter(|(_, m)| m.grows_by_decision)
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        marked, SIZE,
        "the rows of quality/baseline.json with `release_growth`"
    );
    let docs = std::fs::read_to_string(docs_path()).unwrap();
    assert!(
        docs.contains("`release_growth`") && docs.contains("\"release_growth\": \"decision\""),
        "docs/quality.md does not say what `release_growth` means"
    );
    let page = render_site_page(&baseline, &read_history());
    assert!(
        page.contains("binary-size rows (3.2, 3.9)") && page.contains("`release_growth`"),
        "the book page does not say what `release_growth` means"
    );
    let unmarked = render_site_page(&baseline_of(&[("3.2", size_row(false, 1, None))]), &[]);
    assert!(
        !unmarked.contains("release_growth"),
        "the book page names size rows the baseline does not mark"
    );
}

/// One local record of `4.2 rust` under the label `run`.
fn release_record(run: &str, commit: &str, value: i64) -> Record {
    Record {
        commit: commit.to_string(),
        date: "2026-11-10".to_string(),
        run: run.to_string(),
        metric: "4.2".to_string(),
        target: "rust".to_string(),
        value,
        host: "local".to_string(),
        load: None,
        base: None,
        base_value: None,
        machine: None,
        decision: None,
    }
}

fn gaps(value: i64) -> Value {
    Value {
        metric: "4.2".to_string(),
        target: "rust".to_string(),
        value,
        load: None,
    }
}

/// The reference of the release check is the last release that was
/// published (the nearest `v*` tag, passed in), not the last `v*` label in
/// the history: the records of a candidate the gate refused after it, here
/// a `v0.16.0` at two commits, are never the reference. Before the first
/// tag the reference is the `bar` run.
#[test]
fn a_release_is_judged_against_the_last_tagged_release_only() {
    let baseline = baseline_of(&[(
        "4.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("rust", 3)]),
    )]);
    let history = vec![
        release_record("bar", "a", 1),
        release_record("v0.15.0", "b", 1),
        release_record("v0.16.0", "c", 3),
        release_record("run", "d", 3),
        release_record("v0.16.0", "e", 3),
    ];
    let refused = release_refusals(&baseline, &history, &[gaps(3)], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].contains("1 at the last release, 3 now"),
        "a refused candidate's records were taken as the last release: {refused:?}"
    );
    let first = vec![release_record("bar", "a", 1), release_record("run", "c", 3)];
    let refused = release_refusals(&baseline, &first, &[gaps(3)], None);
    assert!(
        refused.len() == 1 && refused[0].contains("1 at the last release, 3 now"),
        "before the first tag the bar run is not the reference: {refused:?}"
    );
}

/// A tag whose label has no record in the history leaves the release
/// without a reference: that is an error naming the tag, not a pass.
#[test]
fn a_release_tag_without_a_record_is_an_error() {
    let baseline = baseline_of(&[(
        "4.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
    )]);
    let history = vec![
        release_record("bar", "a", 1),
        release_record("v0.14.0", "b", 1),
    ];
    let refused = release_refusals(&baseline, &history, &[gaps(1)], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].contains("v0.15.0") && refused[0].contains("no record"),
        "a tag without records let the release pass: {refused:?}"
    );
}

/// The `bar` run is the reference only when the history holds no `v*` run
/// at all. A history with a release and no readable tag (none fetched, a
/// shallow clone, git missing or failing) refuses the release, says the tag
/// could not be read and names `git fetch --tags`, git's reason included
/// where the lookup failed.
#[test]
fn a_release_without_a_readable_tag_is_refused_once_the_history_holds_a_release() {
    let baseline = baseline_of(&[(
        "4.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
    )]);
    let released = vec![
        release_record("bar", "a", 1),
        release_record("v0.15.0", "b", 1),
    ];
    for lookup in [
        LastRelease::NoTag,
        LastRelease::Unreadable("git: No such file or directory".to_string()),
    ] {
        let refused = release_refusals(&baseline, &released, &[gaps(1)], lookup.clone());
        assert!(
            refused.len() == 1
                && refused[0].contains("tag could not be read")
                && refused[0].contains("git fetch --tags"),
            "{lookup:?} with a release in the history let the release pass: {refused:?}"
        );
        if let LastRelease::Unreadable(why) = &lookup {
            assert!(refused[0].contains(why), "{refused:?}");
        }
    }
    let unreleased = vec![release_record("bar", "a", 1)];
    let refused = release_refusals(&baseline, &unreleased, &[gaps(2)], LastRelease::NoTag);
    assert!(
        refused.len() == 1 && refused[0].contains("1 at the last release, 2 now"),
        "{refused:?}"
    );
    assert!(release_refusals(&baseline, &unreleased, &[gaps(1)], LastRelease::NoTag).is_empty());
}

/// A hill-climb row is recorded, not refused, by a merge; a release judges
/// it like any other row: worse than the last release by any amount is
/// refused.
#[test]
fn a_release_judges_a_hill_climb_row_a_merge_only_records() {
    let mut climb = metric(
        Direction::Lower,
        Tolerance::Count(0),
        &[("llvm/patterns", 30)],
    );
    climb.kind = "hill-climb".to_string();
    let baseline = baseline_of(&[("1.3", climb)]);
    let merge = check(&baseline, "static", &one("1.3", "llvm/patterns", 46), None);
    assert!(
        merge.is_empty(),
        "a merge refused a hill-climb row: {merge:?}"
    );
    let mut history = vec![release_record("v0.15.0", "b", 30)];
    history[0].metric = "1.3".to_string();
    history[0].target = "llvm/patterns".to_string();
    let value = |value| Value {
        metric: "1.3".to_string(),
        target: "llvm/patterns".to_string(),
        value,
        load: None,
    };
    let refused = release_refusals(&baseline, &history, &[value(46)], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].contains("30 at the last release, 46 now"),
        "a hill-climb row worse than the last release was not refused: {refused:?}"
    );
    assert!(release_refusals(&baseline, &history, &[value(30)], Some("v0.15.0")).is_empty());
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
        decision: None,
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
    assert!(release_refusals(&baseline, &history, &[value], None).is_empty());
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

/// The baseline and history of one benchmark run on the dedicated host: the
/// todo cell `get-1` and the todo size, each for Ignite's two targets and the
/// Go reference, against plain Rust (permille).
fn bench_case() -> (Baseline, Vec<Record>) {
    let ratio = |direction, tolerance, bars: &[(&str, i64)]| {
        let mut m = metric(direction, tolerance, bars);
        m.source = Some("bench".to_string());
        m.unit = "permille".to_string();
        m
    };
    let baseline = baseline_of(&[
        (
            "3.3",
            ratio(
                Direction::Higher,
                Tolerance::Percent(10),
                &[("llvm/todo/get-1", 800)],
            ),
        ),
        ("3.7", ratio(Direction::Lower, Tolerance::Percent(20), &[])),
        (
            "3.9",
            ratio(
                Direction::Lower,
                Tolerance::Percent(5),
                &[("llvm/todo", 2000)],
            ),
        ),
        ("6.2", {
            let mut timing = metric(Direction::Lower, Tolerance::Percent(10), &[("llvm", 900)]);
            timing.source = Some("perf".to_string());
            timing.unit = "ms".to_string();
            timing
        }),
    ]);
    let ec2 = "ec2:c7i.metal-24xl";
    let record = |metric: &str, target: &str, value: i64| Record {
        commit: "e".to_string(),
        date: "2026-11-08".to_string(),
        run: "run".to_string(),
        metric: metric.to_string(),
        target: target.to_string(),
        value,
        host: ec2.to_string(),
        load: Some("0.10".to_string()),
        base: Some("e".to_string()),
        base_value: Some(value),
        machine: None,
        decision: None,
    };
    let history = vec![
        record("6.2", "llvm", 380),
        record("3.3", "go/todo/get-1", 500),
        record("3.3", "llvm/todo/get-1", 873),
        record("3.3", "rust/todo/get-1", 640),
        record("3.7", "llvm/todo/get-1", 1500),
        record("3.7", "rust/todo/get-1", 1200),
        record("3.9", "go/todo", 6000),
        record("3.9", "llvm/todo", 2100),
        record("3.9", "rust/todo", 900),
    ];
    (baseline, history)
}

/// The native benchmark (`docs/quality.md` row 3.3 and its companions 3.7 to
/// 3.9) has a table of its own in the trend, one row per run and cell with
/// the ratios of Ignite's two targets and Go against plain Rust; the timing
/// table keeps the timing rows only; the page shows the latest ratios and
/// the Go reference without a bar.
#[test]
fn the_native_benchmark_has_its_own_trend_and_page_rows() {
    let (baseline, history) = bench_case();
    assert_eq!(shown(873, "permille"), "0.873×");
    assert_eq!(BENCH, ["3.3", "3.7", "3.8", "3.9"]);
    let trend = render_trend(&baseline, &history);
    assert!(
        trend.contains(
            "| run | e | 2026-11-08 | ec2:c7i.metal-24xl | 6.2 llvm 380 ms | first on this host |"
        ),
        "{trend}"
    );
    assert!(
        trend.contains("## Against plain Rust on the dedicated host"),
        "{trend}"
    );
    assert!(
        trend.contains(
            "| run | e | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-1 | rps 0.873×, p99 1.500× | rps 0.640×, p99 1.200× | rps 0.500× |"
        ),
        "{trend}"
    );
    assert!(
        trend.contains(
            "| run | e | 2026-11-08 | ec2:c7i.metal-24xl | todo | size 2.100× | size 0.900× | size 6.000× |"
        ),
        "{trend}"
    );
    let page = render_site_page(&baseline, &history);
    assert!(
        page.contains("| llvm/todo/get-1 | 0.800× | 0.873× | ec2:c7i.metal-24xl |"),
        "{page}"
    );
    assert!(
        page.contains("| go/todo/get-1 | reference, no bar | 0.500× | ec2:c7i.metal-24xl |"),
        "{page}"
    );
}

/// The bars of the benchmark are its first run's ratios, ratcheted: a target
/// of the latest host run without a bar and an improvement beyond the
/// tolerance are refused until `IGNITE_BLESS=1` writes them; a worse ratio is
/// not refused here (another host, another day), the release refusal within
/// one run is in `ignite-perf quality-remote`; Go has no bar.
#[test]
fn the_bench_bars_take_the_latest_host_run() {
    let (baseline, history) = bench_case();
    let errors = bench_bars(&baseline, &history);
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("3.3 rust/todo/get-1: 640 has no bar")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|e| e.starts_with("3.7 llvm/todo/get-1: 1500 has no bar")),
        "{errors:?}"
    );
    assert!(!errors.iter().any(|e| e.contains("go/")), "{errors:?}");
    assert!(
        !errors.iter().any(|e| e.starts_with("3.9 llvm/todo")),
        "a worse size within the run was refused: {errors:?}"
    );
    let blessed = bless_bench(&baseline, &history);
    assert_eq!(blessed.metrics["3.3"].bars["llvm/todo/get-1"].value, 873);
    assert_eq!(blessed.metrics["3.3"].bars["rust/todo/get-1"].value, 640);
    assert_eq!(blessed.metrics["3.9"].bars["llvm/todo"].value, 2000);
    assert!(!blessed.metrics["3.3"].bars.contains_key("go/todo/get-1"));
    assert!(
        bench_bars(&blessed, &history).is_empty(),
        "{:?}",
        bench_bars(&blessed, &history)
    );
    let mut improved = blessed.clone();
    improved
        .metrics
        .get_mut("3.3")
        .unwrap()
        .bars
        .get_mut("llvm/todo/get-1")
        .unwrap()
        .value = 700;
    assert!(
        bench_bars(&improved, &history)
            .iter()
            .any(|e| e.starts_with("3.3 llvm/todo/get-1: improved to 873 from 700")),
        "{:?}",
        bench_bars(&improved, &history)
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
    // Main holds the text "story:x, measured 185 s" (for `other`), so it is
    // no new decision for either unit: a decision of main is never reused.
    let errors = slow_against_main(&stale, &main_with);
    assert!(
        errors.len() == 2
            && errors.iter().any(|e| e.contains("s.ignite::other"))
            && errors.iter().any(|e| e.contains("s.ignite::slow")),
        "a decision main holds was reused: {errors:?}"
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
        errors.len() == 1 && errors[0].contains("no open story"),
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

// ---- adversary, story quality-ratchet pass 2 ----------------------------

/// The differential baseline of pass 2's cases: 1.1 llvm at 0, 1.7 at 1,
/// and the known differences `known` (each intermittent, on LLVM, naming
/// `story`).
fn adversary2_differential(known: &[&str], story: &str) -> Baseline {
    let gate = |value: i64, target: &str| {
        let mut m = metric(Direction::Lower, Tolerance::Count(0), &[(target, value)]);
        m.source = Some("differential".to_string());
        m
    };
    let mut b = baseline_of(&[("1.1", gate(0, "llvm")), ("1.7", gate(1, "all"))]);
    for unit in known {
        b.known_differences.insert(
            unit.to_string(),
            Known {
                consumer: "llvm".to_string(),
                story: story.to_string(),
                what: "a timing assertion".to_string(),
                intermittent: true,
            },
        );
    }
    b
}

fn adversary2_outcome(llvm_differs: &[&str]) -> differential::Outcome {
    differential::Outcome {
        units: BTreeMap::new(),
        llvm_differs: llvm_differs
            .iter()
            .map(|u| (u.to_string(), "exit 101 on Rust, 0 on LLVM".to_string()))
            .collect(),
        eval_differs: BTreeMap::new(),
        eval_cannot: BTreeMap::new(),
        build_failed: BTreeMap::new(),
        unchecked: BTreeMap::new(),
        no_run: BTreeMap::new(),
        metrics: vec![(
            "1.1".to_string(),
            "llvm".to_string(),
            llvm_differs.len() as i64,
        )],
    }
}

/// A6 again: 1.7 counts the intermittent entries, not which ones. Main
/// lists `old` as intermittent; the branch drops `old` and lists a new,
/// steady LLVM regression `new` as intermittent instead. 1.7 stays 1, 1.1
/// leaves `new` out, and nothing compares `known_differences` with main's,
/// so the regression merges with no decision.
#[test]
fn adversary2_a_new_regression_swapped_in_for_an_intermittent_entry_is_refused() {
    let story = "story:llvm-store-update-off-lock";
    let main = adversary2_differential(&["t.ignite::old"], story);
    let branch = adversary2_differential(&["t.ignite::new"], story);
    let outcome = adversary2_outcome(&["t.ignite::new"]);
    let mut errors = differential_lists(&branch, &outcome);
    errors.extend(check(
        &branch,
        "differential",
        &steady(&branch, &outcome),
        Some(&main),
    ));
    assert!(
        !errors.is_empty(),
        "a steady LLVM difference listed as intermittent in place of main's entry passed"
    );
}

/// A2 again: "a decision of its own" is any text unequal to main's, so
/// main's own decision with a trailing space lets the bar worsen again.
#[test]
fn adversary2_main_decision_with_a_trailing_space_is_not_a_new_decision() {
    let decided = |value: i64, decision: &str| {
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
            .decision = Some(decision.to_string());
        b
    };
    let main = decided(18, "story:x, 2026-11-07");
    let worse_again = decided(30, "story:x, 2026-11-07 ");
    let errors = check(&worse_again, "static", &one("7.2", "all", 30), Some(&main));
    assert!(
        errors.iter().any(|e| e.contains("without a `decision`")),
        "the bar moved 18 -> 30 on main's decision plus a space and passed: {errors:?}"
    );
}

/// A6/1.7: an intermittent entry must name a story that tracks its fix; a
/// story the store holds as `implemented` tracks nothing.
#[test]
fn adversary2_an_intermittent_entry_naming_an_implemented_story_is_refused() {
    let branch = adversary2_differential(&["t.ignite::old"], "story:apply-recovery-way-out");
    let story = std::fs::read_to_string(
        workspace().join(".engineering/planning/story/apply-recovery-way-out.md"),
    )
    .unwrap();
    assert!(
        story.contains("\nstatus: implemented\n"),
        "fixture story moved"
    );
    let errors = differential_lists(&branch, &adversary2_outcome(&[]));
    assert!(
        !errors.is_empty(),
        "an intermittent difference parked on an implemented story passed"
    );
}

/// Gate adversary pass 2: a bar that carries a `decision` and is equal to or
/// better than the last release's value refuses nothing. The real baseline
/// holds three such bars (4.2 llvm/rfc9110 and rust/rfc9110 at 2, 7.4 all at
/// 38, each equal to v0.15.0's value); refusing them would hold a release
/// that is worse on no row.
#[test]
fn gate_adv2_a_decision_on_a_bar_not_worse_than_the_last_release_refuses_nothing() {
    let decided = |direction, bar: i64| {
        let mut m = metric(direction, Tolerance::Count(0), &[("rust", bar)]);
        m.bars.get_mut("rust").unwrap().decision =
            Some("story:x (2026-11-01): kept from an earlier wave".to_string());
        baseline_of(&[("4.2", m)])
    };
    let history = vec![release_record("v0.15.0", "b", 2)];
    for (direction, bar) in [
        (Direction::Lower, 2),
        (Direction::Lower, 1),
        (Direction::Higher, 2),
        (Direction::Higher, 3),
    ] {
        let refused = release_refusals(
            &decided(direction, bar),
            &history,
            &[gaps(2)],
            Some("v0.15.0"),
        );
        assert!(
            refused.is_empty(),
            "{direction:?} bar {bar} by a decision, 2 at the last release, 2 now, refused: {refused:?}"
        );
    }
    let refused = release_refusals(
        &decided(Direction::Lower, 3),
        &history,
        &[gaps(2)],
        Some("v0.15.0"),
    );
    assert!(
        refused.len() == 1 && refused[0].contains("kept from an earlier wave"),
        "{refused:?}"
    );
}

/// Gate adversary pass 2: the reference is the history's *last* local run
/// under the tag's label; a dedicated-host record under the same label is
/// never it, and neither is an earlier local run of that label.
#[test]
fn gate_adv2_the_reference_is_the_last_local_run_under_the_tag() {
    let baseline = baseline_of(&[(
        "4.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("rust", 2)]),
    )]);
    let mut remote = release_record("v0.15.0", "c", 9);
    remote.host = "ec2:c7i.metal-24xl".to_string();
    let history = vec![
        release_record("v0.15.0", "b", 1),
        remote,
        release_record("v0.15.0", "d", 2),
        release_record("run", "e", 5),
    ];
    let refused = release_refusals(&baseline, &history, &[gaps(2)], Some("v0.15.0"));
    assert!(
        refused.is_empty(),
        "an earlier local run of the tag's label was taken as the reference: {refused:?}"
    );
    let refused = release_refusals(&baseline, &history, &[gaps(3)], Some("v0.15.0"));
    assert!(
        refused.len() == 1 && refused[0].contains("2 at the last release, 3 now"),
        "{refused:?}"
    );
}

/// Gate adversary pass 2, red: `AGENTS.md` § Releases says a release is
/// refused for "a bar a `decision` moved worse than the last release,
/// whatever the value", and "A bar a `decision` made worse since the last
/// release tag blocks the release until the value is back and the decision
/// is withdrawn". The real baseline holds 3.9 llvm/terminal at 1820 by the
/// decision "Accepted for v0.16.0 (Timo, 2026-11-09) ..." (1378 at
/// v0.15.0, 1406 in v0.15.0's dedicated-host record), and three more 3.9
/// bars and five 3.3/3.7 bars decided the same way. `task quality` measures
/// no benchmark row and `release_refusals` skips timing rows, and
/// `ignite-perf quality-remote` judges values against BASE and reads no
/// decision: nothing refuses the release while the decision stands.
#[test]
fn gate_adv2_a_benchmark_bar_a_decision_moved_worse_refuses_the_release() {
    let decision =
        "Accepted for v0.16.0 (Timo, 2026-11-09): the dedicated-host run refused this row";
    let mut size = metric(
        Direction::Lower,
        Tolerance::Percent(5),
        &[("llvm/terminal", 1820)],
    );
    size.source = Some("bench".to_string());
    size.unit = "permille".to_string();
    size.bars.get_mut("llvm/terminal").unwrap().decision = Some(decision.to_string());
    let baseline = baseline_of(&[
        ("3.9", size),
        (
            "4.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
        ),
    ]);
    let mut host = release_record("v0.15.0", "a", 1406);
    host.host = "ec2:c7i.metal-24xl".to_string();
    host.metric = "3.9".to_string();
    host.target = "llvm/terminal".to_string();
    host.base = Some("a".to_string());
    host.base_value = Some(1406);
    let history = vec![host, release_record("v0.15.0", "b", 1)];
    let refused = release_refusals(&baseline, &history, &[gaps(1)], Some("v0.15.0"));
    assert!(
        refused.iter().any(|r| r.contains(decision)),
        "a benchmark bar a decision moved worse than the last release (1406 -> 1820) is in the baseline, and the release was not refused: {refused:?}"
    );
}

/// A timing or benchmark bar is measured only on the dedicated host, so the
/// release check holds it against the bar of the same row in the baseline at
/// the last release's tag: a bar a `decision` moved worse than that refuses
/// the release, naming the row, both bars and the decision. A decided bar
/// equal to or better than the tag's, and a row with no bar at the tag,
/// refuse nothing. A tag whose baseline cannot be read leaves decided
/// timing and benchmark bars uncompared, which is a refusal, not a pass.
#[test]
fn a_release_refuses_a_timing_or_benchmark_bar_a_decision_moved_worse_than_at_the_tag() {
    let decision = "story:x (2026-11-09): accepted for the release";
    let row = |source: &str, direction, tolerance, bars: &[(&str, i64)], decided: bool| {
        let mut m = metric(direction, tolerance, bars);
        m.source = Some(source.to_string());
        m.unit = if source == "bench" { "permille" } else { "ms" }.to_string();
        if decided {
            for bar in m.bars.values_mut() {
                bar.decision = Some(decision.to_string());
            }
        }
        m
    };
    let lower = Direction::Lower;
    let at_tag = baseline_of(&[
        (
            "3.9",
            row(
                "bench",
                lower,
                Tolerance::Percent(5),
                &[("llvm/terminal", 1378)],
                false,
            ),
        ),
        (
            "3.3",
            row(
                "bench",
                Direction::Higher,
                Tolerance::Percent(10),
                &[("llvm/todo/get-1", 1015)],
                false,
            ),
        ),
        (
            "3.7",
            row(
                "bench",
                lower,
                Tolerance::Percent(20),
                &[("rust/todo/get-1", 1044)],
                false,
            ),
        ),
        (
            "3.8",
            row(
                "bench",
                lower,
                Tolerance::Percent(10),
                &[("llvm/todo/get-1", 819)],
                false,
            ),
        ),
        (
            "6.2",
            row(
                "perf",
                lower,
                Tolerance::Percent(10),
                &[("llvm", 993)],
                false,
            ),
        ),
        (
            "4.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
        ),
    ]);
    let now = baseline_of(&[
        (
            "3.9",
            row(
                "bench",
                lower,
                Tolerance::Percent(5),
                &[("llvm/terminal", 1820), ("llvm/todo", 869)],
                true,
            ),
        ),
        (
            "3.3",
            row(
                "bench",
                Direction::Higher,
                Tolerance::Percent(10),
                &[("llvm/todo/get-1", 900)],
                true,
            ),
        ),
        (
            "3.7",
            row(
                "bench",
                lower,
                Tolerance::Percent(20),
                &[("rust/todo/get-1", 1044)],
                true,
            ),
        ),
        (
            "3.8",
            row(
                "bench",
                lower,
                Tolerance::Percent(10),
                &[("llvm/todo/get-1", 800)],
                true,
            ),
        ),
        (
            "6.2",
            row(
                "perf",
                lower,
                Tolerance::Percent(10),
                &[("llvm", 1200)],
                true,
            ),
        ),
        (
            "4.2",
            metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
        ),
    ]);
    let history = vec![release_record("v0.15.0", "b", 1)];
    let refused =
        release_refusals_against(&now, Some(&at_tag), &history, &[gaps(1)], Some("v0.15.0"));
    assert_eq!(refused.len(), 3, "{refused:?}");
    for (row, then, bar) in [
        ("3.3 llvm/todo/get-1", "1015", "900"),
        ("3.9 llvm/terminal", "1378", "1820"),
        ("6.2 llvm", "993", "1200"),
    ] {
        assert!(
            refused.iter().any(|r| r.starts_with(row)
                && r.contains(then)
                && r.contains(bar)
                && r.contains("v0.15.0")
                && r.contains(decision)),
            "{row}: a bar a decision moved from {then} at the tag to {bar} was not refused: {refused:?}"
        );
    }

    let unread = release_refusals_against(&now, None, &history, &[gaps(1)], Some("v0.15.0"));
    assert!(
        unread.len() == 1
            && unread[0].contains("v0.15.0")
            && unread[0].contains("could not be read"),
        "a tag whose baseline cannot be read let decided benchmark bars pass: {unread:?}"
    );
    let plain = release_refusals_against(&at_tag, None, &history, &[gaps(1)], Some("v0.15.0"));
    assert!(plain.is_empty(), "{plain:?}");
}

/// Size-rule adversary pass 1, the story's acceptance: "the list is also in
/// the record the release notes are written from, where one exists". The
/// release notes carry the release's row of `quality/trend.md` (`AGENTS.md`
/// § Releases), and that row is generated from the baseline and the history
/// by [`render_trend`]. It names a grown cell of a size row with its value
/// at the last release and its value now, among every other change; the
/// decision that holds the cell is the part of the list it does not carry,
/// so the only place a release shows why a binary grew is the stderr of the
/// run.
#[test]
fn sizerule_adv1_the_release_row_of_the_trend_carries_the_decision_of_a_grown_cell() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let baseline = baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]);
    let mut history = size_at_the_last_release(1000);
    let mut release = size_at_the_last_release(1040);
    release[0].run = "v0.16.0".to_string();
    release[0].commit = "c".to_string();
    release[0].decision = Some(decision.to_string());
    history.extend(release);
    let at_tag = baseline_of(&[("3.2", size_row(true, 1000, None))]);
    let judged = release_judged_against(
        &baseline,
        Some(&at_tag),
        &history[..1],
        &[size_now(1040)],
        Some("v0.15.0"),
    );
    assert!(
        judged.refused.is_empty() && judged.grown.len() == 1,
        "the release this history records passes with one grown cell: {judged:?}"
    );
    let trend = render_trend(&baseline, &history);
    let row = trend
        .lines()
        .find(|line| line.starts_with("| v0.16.0 |"))
        .expect("the release's row of the trend");
    assert!(
        row.contains("3.2 rust/todod/release 1000 B→1040 B"),
        "the release's row does not name the grown cell and both values: {row}"
    );
    assert!(
        row.contains(decision),
        "the release's row of quality/trend.md, which the release notes carry, names the grown cell and both values and not the decision that holds it: {row}"
    );
}

/// Size-rule adversary pass 1: `quality::record` judges a release with
/// [`release_judged`], which reads the baseline at the last release's tag
/// from git; every test of the unit gives that baseline by hand
/// ([`release_judged_against`]). A wrapper that handed over the tag's
/// baseline without its decisions left the unit's suite green (mutant M20
/// of the pass), and with it every decision is new. Here the tag's own
/// baseline is the reference: a decision it holds on a bar does not let a
/// cell grow, and one it does not hold does. A clone without a `v*` tag, or
/// whose tag's baseline holds no decision, has nothing to read here.
#[test]
fn sizerule_adv1_the_release_check_reads_the_last_releases_decisions_from_the_tag() {
    let LastRelease::Tag(tag) = last_release_tag("the candidate") else {
        eprintln!("no `v*` tag is reachable: nothing judged");
        return;
    };
    let Some(held) = baseline_at(&tag).and_then(|at_tag| {
        at_tag
            .metrics
            .values()
            .flat_map(|metric| metric.bars.values())
            .filter_map(|bar| bar.decision.clone())
            .find(|decision| !decision.trim().is_empty())
    }) else {
        eprintln!("the baseline at {tag} holds no decision on a bar: nothing judged");
        return;
    };
    let mut history = size_at_the_last_release(1000);
    history[0].run = tag.clone();
    let judge = |decision: &str| {
        release_judged(
            &baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]),
            &history,
            &[size_now(1040)],
            LastRelease::Tag(tag.clone()),
        )
    };
    let carried = judge(&held);
    assert!(
        carried.grown.is_empty()
            && carried.refused.len() == 1
            && carried.refused[0].contains("is the last release's")
            && carried.refused[0].contains(&tag),
        "a decision the baseline at {tag} holds let a size cell grow: {carried:?}"
    );
    let new = judge("story:x (2026-11-11): code no release before this one names");
    assert!(
        new.refused.is_empty() && new.grown.len() == 1,
        "a decision the baseline at {tag} does not hold: {new:?}"
    );
}

// ---- the size rule, fix round 2 ------------------------------------------

/// The decision the last release held, in the tests of how a decision reads.
const HELD: &str = "story:w (2026-11-08): the scheduler's run queue adds the bytes.";

/// A decision is read by its letters and digits, case folded: one with
/// fewer than 40 of them names nothing; one that is the last release's, or
/// contains it, is the last release's (upper case, the final dot dropped,
/// other punctuation, a zero-width space, a sentence appended or put before
/// it). One character of it changed reads as new: the check reads that the
/// text differs, a person reads what it says. A text of the last release
/// with fewer than 40 letters and digits is no decision either and carries
/// nothing over. `tools/ignite-perf/tests/remote.rs` holds the same cases
/// to `remote::read_decision`.
#[test]
fn a_decision_is_read_by_its_letters_and_digits() {
    let held = BTreeSet::from([HELD.to_string()]);
    let cases = [
        (HELD.to_string(), DecisionRead::LastReleases, "itself"),
        (
            HELD.to_uppercase(),
            DecisionRead::LastReleases,
            "in upper case",
        ),
        (
            HELD.trim_end_matches('.').to_string(),
            DecisionRead::LastReleases,
            "without its final dot",
        ),
        (
            "Story:W, 2026-11-08 - the  scheduler's run-queue adds the bytes".to_string(),
            DecisionRead::LastReleases,
            "with other punctuation and spacing",
        ),
        (
            HELD.replace(' ', "\u{200b} "),
            DecisionRead::LastReleases,
            "with zero-width spaces in it",
        ),
        (
            format!("{HELD} It holds the TLS client session too."),
            DecisionRead::LastReleases,
            "with a sentence appended",
        ),
        (
            format!("Accepted again for this release: {HELD}"),
            DecisionRead::LastReleases,
            "with a sentence put before it",
        ),
        (
            HELD.replace("bytes", "bytez"),
            DecisionRead::New,
            "with one character changed",
        ),
        (
            "story:x (2026-11-11): the TLS client session adds 40 B".to_string(),
            DecisionRead::New,
            "another decision of 40 letters and digits",
        ),
        (
            "story:x (2026-11-11): the TLS client session adds 4 B".to_string(),
            DecisionRead::NamesNothing,
            "39 letters and digits",
        ),
        (
            "\u{200b}".to_string(),
            DecisionRead::NamesNothing,
            "a zero-width space",
        ),
        (".".to_string(), DecisionRead::NamesNothing, "a dot"),
        ("x".to_string(), DecisionRead::NamesNothing, "one letter"),
        (String::new(), DecisionRead::NamesNothing, "nothing"),
        (
            "-- .. -- .. -- .. -- .. -- .. -- .. -- .. -- .. -- .. -- ..".to_string(),
            DecisionRead::NamesNothing,
            "60 characters, no letter or digit",
        ),
    ];
    for (decision, read, what) in cases {
        assert_eq!(
            read_decision(&decision, &held),
            read,
            "the last release's decision {what}: {decision:?}"
        );
    }
    let short = BTreeSet::from(["x".to_string(), ".".to_string(), String::new()]);
    assert_eq!(
        read_decision(
            "story:x (2026-11-11): the TLS client session adds 40 B",
            &short
        ),
        DecisionRead::New,
        "a text of the last release that names nothing carried over"
    );
}

/// The release check reads the decision of a grown cell that way: the last
/// release's decision reworded does not let the cell grow again, one
/// character changed does, and a text that names nothing is no decision,
/// whether the tag's baseline can be read or not.
#[test]
fn a_grown_cell_is_held_only_by_a_decision_new_in_its_letters_and_digits() {
    let at_tag = baseline_of(&[("3.2", size_row(true, 1000, Some(HELD)))]);
    let judge = |decision: &str, at_tag: Option<&Baseline>| {
        release_judged_against(
            &baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]),
            at_tag,
            &size_at_the_last_release(1000),
            &[size_now(1040)],
            Some("v0.15.0"),
        )
    };
    for decision in [
        HELD.to_uppercase(),
        HELD.trim_end_matches('.').to_string(),
        format!("{HELD} It holds the TLS client session too."),
    ] {
        let judged = judge(&decision, Some(&at_tag));
        assert!(
            judged.grown.is_empty()
                && judged.refused.len() == 1
                && judged.refused[0].contains("is the last release's")
                && judged.refused[0].contains("a new `decision` naming the added code is needed"),
            "{decision:?}: {judged:?}"
        );
    }
    let changed = judge(&HELD.replace("bytes", "bytez"), Some(&at_tag));
    assert!(
        changed.refused.is_empty() && changed.grown.len() == 1,
        "one character changed: {changed:?}"
    );
    for decision in ["\u{200b}", ".", "x", "story:x: the bytes"] {
        for at_tag in [Some(&at_tag), None] {
            let judged = judge(decision, at_tag);
            assert!(
                judged.grown.is_empty()
                    && judged.refused.len() == 1
                    && judged.refused[0]
                        .starts_with("3.2 rust/todod/release: 1000 at the last release, 1040 now")
                    && judged.refused[0].contains("names nothing")
                    && judged.refused[0].contains("a `decision` naming the added code is missing"),
                "{decision:?}: {judged:?}"
            );
        }
    }
}

/// A decision on the bar of a size cell that is not above the last release
/// holds nothing, and the release check refuses it until it is removed: a
/// decision written for a candidate that was not cut does not wait on the
/// bar to hold a later growth nobody decided. A blank decision is none; a
/// row that is no size row is judged as before.
#[test]
fn a_decision_on_a_size_cell_not_above_the_last_release_is_refused() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let at_tag = baseline_of(&[("3.2", size_row(true, 1000, None))]);
    for (now, at_tag) in [
        (1000, Some(&at_tag)),
        (990, Some(&at_tag)),
        (1000, None),
        (990, None),
    ] {
        let judged = release_judged_against(
            &baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]),
            at_tag,
            &size_at_the_last_release(1000),
            &[size_now(now)],
            Some("v0.15.0"),
        );
        assert!(
            judged.grown.is_empty()
                && judged.refused.len() == 1
                && judged.refused[0].starts_with(&format!(
                    "3.2 rust/todod/release: 1000 at the last release, {now} now"
                ))
                && judged.refused[0].contains(decision)
                && judged.refused[0].contains("holds nothing")
                && judged.refused[0].contains("remove it"),
            "{now} now, a decision on the bar: {judged:?}"
        );
    }
    let mut first = size_at_the_last_release(1000);
    first[0].run = "bar".to_string();
    let before_the_first_release = release_judged_against(
        &baseline_of(&[("3.2", size_row(true, 1040, Some(decision)))]),
        None,
        &first,
        &[size_now(1000)],
        LastRelease::NoTag,
    );
    assert!(
        before_the_first_release.refused.len() == 1
            && before_the_first_release.refused[0].contains("holds nothing"),
        "{before_the_first_release:?}"
    );
    let blank = release_judged_against(
        &baseline_of(&[("3.2", size_row(true, 1040, Some("  ")))]),
        Some(&at_tag),
        &size_at_the_last_release(1000),
        &[size_now(1000)],
        Some("v0.15.0"),
    );
    assert_eq!(blank, Release::default(), "a blank decision");
    let unmarked = release_judged_against(
        &baseline_of(&[("3.2", size_row(false, 1000, Some(decision)))]),
        Some(&at_tag),
        &size_at_the_last_release(1000),
        &[size_now(1000)],
        Some("v0.15.0"),
    );
    assert_eq!(
        unmarked,
        Release::default(),
        "a row that is no size row, its bar not moved"
    );
}

/// The release's record of a grown cell carries the decision that holds
/// it, written into `quality/history.jsonl` and read back; every other
/// record of the run, and every record of a run that is no release (a
/// refused release is labelled `run`), has no `decision`.
#[test]
fn the_record_of_a_grown_cell_carries_its_decision() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let values = [size_now(1040), gaps(2)];
    let grown = [Grown {
        metric: "3.2".to_string(),
        target: SIZE_CELL.to_string(),
        then: 1000,
        now: 1040,
        unit: "bytes".to_string(),
        decision: decision.to_string(),
    }];
    let records = run_records(&values, &grown, "c", "2026-11-11", "v0.16.0", "local");
    assert_eq!(records.len(), 2);
    assert_eq!(
        (
            records[0].metric.as_str(),
            records[0].value,
            records[0].decision.as_deref()
        ),
        ("3.2", 1040, Some(decision)),
        "the grown cell's record"
    );
    assert_eq!(records[1].decision, None, "a cell that did not grow");
    let line = records[0].line();
    assert!(
        line.contains(&format!(r#""decision":"{decision}""#)),
        "the line of the history: {line}"
    );
    assert_eq!(Record::parse(&line).unwrap(), records[0], "read back");
    assert!(
        !records[1].line().contains("decision"),
        "a record without a decision has the field: {}",
        records[1].line()
    );
    let refused = run_records(&values, &grown, "c", "2026-11-11", "run", "local");
    assert!(
        refused.iter().all(|record| record.decision.is_none()),
        "a run that is no release carries a decision: {refused:?}"
    );
}

/// The release's row of `quality/trend.md` lists the grown size cells with
/// both values under the decision that holds them, from the history alone:
/// cells of one decision are listed together and the decision is written
/// once; a cell of 3.9, recorded by the dedicated host's run of the same
/// release, is listed with BASE's ratio and HEAD's; a `|` in a decision
/// does not end the table cell. A release without a grown cell has no such
/// list.
#[test]
fn the_trend_lists_the_grown_cells_of_a_release_under_their_decisions() {
    let decision = "story:x (2026-11-11): the TLS client session adds 40 B";
    let piped = "story:y (2026-11-11): the HTTP client | its pool adds the bytes";
    let mut sizes = size_row(true, 1040, Some(decision));
    sizes.bars.insert(
        "llvm/todod/release".to_string(),
        Bar {
            value: 2100,
            decision: Some(decision.to_string()),
        },
    );
    let mut ratio = metric(
        Direction::Lower,
        Tolerance::Percent(5),
        &[("rust/todo", 864)],
    );
    ratio.source = Some("bench".to_string());
    ratio.unit = "permille".to_string();
    ratio.grows_by_decision = true;
    let baseline = baseline_of(&[("3.2", sizes), ("3.9", ratio)]);
    let cell = |run: &str, commit: &str, target: &str, value: i64, decision: Option<&str>| {
        let mut record = release_record(run, commit, value);
        record.metric = "3.2".to_string();
        record.target = target.to_string();
        record.decision = decision.map(str::to_string);
        record
    };
    let mut on_the_host = release_record("v0.16.0", "c", 864);
    on_the_host.metric = "3.9".to_string();
    on_the_host.target = "rust/todo".to_string();
    on_the_host.host = "ec2:c7i.large".to_string();
    on_the_host.base = Some("b".to_string());
    on_the_host.base_value = Some(723);
    on_the_host.decision = Some(piped.to_string());
    let history = vec![
        cell("v0.15.0", "b", "llvm/todod/release", 2000, None),
        cell("v0.15.0", "b", SIZE_CELL, 1000, None),
        cell("v0.16.0", "c", "llvm/todod/release", 2100, Some(decision)),
        cell("v0.16.0", "c", SIZE_CELL, 1040, Some(decision)),
        on_the_host,
        cell("v0.17.0", "d", "llvm/todod/release", 2100, None),
        cell("v0.17.0", "d", SIZE_CELL, 1030, None),
    ];
    let trend = render_trend(&baseline, &history);
    let row = |run: &str| {
        trend
            .lines()
            .find(|line| line.starts_with(&format!("| {run} |")))
            .unwrap_or_else(|| panic!("the row of {run}:\n{trend}"))
    };
    let grown = row("v0.16.0");
    assert!(
        grown.ends_with(
            "; size cells above the last release: 3.2 llvm/todod/release 2000 B→2100 B, 3.2 rust/todod/release 1000 B→1040 B (decision: story:x (2026-11-11): the TLS client session adds 40 B); 3.9 rust/todo 0.723×→0.864× (decision: story:y (2026-11-11): the HTTP client \\| its pool adds the bytes) |"
        ),
        "the release's row: {grown}"
    );
    assert_eq!(
        grown.matches(decision).count(),
        1,
        "the decision of two cells is written once: {grown}"
    );
    assert_eq!(
        grown.replace("\\|", "").matches('|').count(),
        8,
        "a `|` of a decision ended a table cell: {grown}"
    );
    for run in ["v0.15.0", "v0.17.0"] {
        assert!(
            !row(run).contains("size cells above the last release")
                && !row(run).contains("decision"),
            "a release without a grown cell: {}",
            row(run)
        );
    }
}

/// `record` judges a release and prints its list through [`run_judged`]:
/// the list of the grown cells is the text it returns, whether the release
/// is refused on another row or not, and a run that is no release is not
/// judged. The baseline at the last release's tag reaches the judgement
/// from git: a decision that baseline holds refuses the cell and lists
/// nothing, one it does not hold is listed (a clone without a `v*` tag, or
/// whose tag's baseline holds no decision on a bar, has nothing to read for
/// that part).
#[test]
fn the_release_run_reads_the_tags_decisions_and_returns_its_list() {
    let decision = "story:x (2026-11-11): code no release before this one names";
    let baseline = |decision: &str| {
        let gaps_row = metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]);
        baseline_of(&[
            ("3.2", size_row(true, 1040, Some(decision))),
            ("4.2", gaps_row),
        ])
    };
    let list = format!(
        "release v0.1.0: 1 cell of the size rows above the last release, each held by the decision on its bar:\n  3.2 rust/todod/release: 1000 B at the last release, 1040 B now; decision: {decision}\n"
    );
    let mut first = size_at_the_last_release(1000);
    first[0].run = "bar".to_string();
    first.push(release_record("bar", "b", 1));
    let (judged, listed) = run_judged(
        "v0.1.0",
        &baseline(decision),
        &first,
        &[size_now(1040), gaps(1)],
        LastRelease::NoTag,
    );
    assert!(
        judged.refused.is_empty() && judged.grown.len() == 1,
        "{judged:?}"
    );
    assert_eq!(listed, list, "the list of a release that passes");
    let (judged, listed) = run_judged(
        "v0.1.0",
        &baseline(decision),
        &first,
        &[size_now(1040), gaps(2)],
        LastRelease::NoTag,
    );
    assert!(
        judged.refused.len() == 1 && judged.refused[0].starts_with("4.2 rust"),
        "{judged:?}"
    );
    assert_eq!(listed, list, "the list of a release refused on another row");
    let (judged, listed) = run_judged(
        "run",
        &baseline(decision),
        &first,
        &[size_now(1040), gaps(2)],
        LastRelease::NoTag,
    );
    assert_eq!(
        (judged, listed),
        (Release::default(), String::new()),
        "a run that is no release"
    );

    let LastRelease::Tag(tag) = last_release_tag("the candidate") else {
        eprintln!("no `v*` tag is reachable: the tag's baseline is not read here");
        return;
    };
    let Some(held) = baseline_at(&tag).and_then(|at_tag| {
        at_tag
            .metrics
            .values()
            .flat_map(|metric| metric.bars.values())
            .filter_map(|bar| bar.decision.clone())
            .find(|decision| read_decision(decision, &BTreeSet::new()) == DecisionRead::New)
    }) else {
        eprintln!("the baseline at {tag} holds no decision on a bar: nothing read here");
        return;
    };
    let mut history = size_at_the_last_release(1000);
    history[0].run = tag.clone();
    history.push(release_record(&tag, "b", 1));
    let run = |decision: &str| {
        run_judged(
            "v99.0.0",
            &baseline(decision),
            &history,
            &[size_now(1040), gaps(1)],
            LastRelease::Tag(tag.clone()),
        )
    };
    let (carried, listed) = run(&held);
    assert!(
        carried.grown.is_empty()
            && carried.refused.len() == 1
            && carried.refused[0].contains("is the last release's")
            && carried.refused[0].contains(&tag),
        "a decision the baseline at {tag} holds: {carried:?}"
    );
    assert_eq!(listed, "", "a refused cell is listed");
    let (new, listed) = run(decision);
    assert!(new.refused.is_empty() && new.grown.len() == 1, "{new:?}");
    assert_eq!(
        listed,
        list.replace("v0.1.0", "v99.0.0"),
        "the list against the baseline at {tag}"
    );
}

// ---- rule adversary 1: a release that tracks its worse rows -------------

/// A release whose check compared no row: the tag of the last release could
/// not be read, or the tag has no record in the history. `record` prints as
/// tracked, and records under the release's label, only what a check without
/// `unjudged` found; with it what the check says stops the release
/// and the run is labelled `run`. Row 4.2 stands at 9 here and at 1 at the
/// last release. (The adversary's case, moved onto the field `record` reads.)
#[test]
fn rule_adv1_a_release_that_compared_no_row_is_not_printed_as_tracked() {
    let baseline = baseline_of(&[(
        "4.2",
        metric(Direction::Lower, Tolerance::Count(0), &[("rust", 1)]),
    )]);
    let released = vec![
        release_record("bar", "a", 1),
        release_record("v0.15.0", "b", 1),
    ];
    let unrecorded = vec![
        release_record("bar", "a", 1),
        release_record("v0.14.0", "b", 1),
    ];
    let cases = [
        (
            "the tag lookup failed",
            &released,
            LastRelease::Unreadable("git: No such file or directory".to_string()),
        ),
        ("no tag is fetched", &released, LastRelease::NoTag),
        (
            "the tag has no record in the history",
            &unrecorded,
            LastRelease::Tag("v0.15.0".to_string()),
        ),
    ];
    let mut passed = Vec::new();
    for (case, history, lookup) in cases {
        let (release, _) = run_judged("v0.16.0", &baseline, history, &[gaps(9)], lookup);
        assert_eq!(release.refused.len(), 1, "{case}: {release:?}");
        if !release.unjudged {
            passed.push(format!("{case}: {:?}", release.refused));
        }
    }
    let (compared, _) = run_judged(
        "v0.16.0",
        &baseline,
        &released,
        &[gaps(9)],
        LastRelease::Tag("v0.15.0".to_string()),
    );
    assert!(
        !compared.unjudged && compared.refused.len() == 1,
        "a release compared with its last release, one row worse: {compared:?}"
    );
    assert!(
        passed.is_empty(),
        "a release whose check compared no row (4.2 rust: 1 at the last release, 9 now) is printed as a tracked row, recorded under its label and passed:\n  {}",
        passed.join("\n  ")
    );
}
