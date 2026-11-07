//! The rounds of the website's UI loop (`ux/rounds/`, SKILL.md section 3.2).
//! `task ux-round ROUND=R-nn` runs the gating suite and the round's trial
//! (`ux/round/`, Playwright) and then [`write`], which turns their output
//! under `target/ux/` into `ux/rounds/R-nn/` (`report.md`, `round.json`,
//! `shots/`) and rebuilds `ux/rounds/trend.md`. [`write`] is ignored here so
//! a plain `cargo test` needs no browser; [`round_dirs_hold_what_they_name`]
//! checks every committed round's shape on every run.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ignite_eval::Json;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse(path: &Path) -> Json {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Json::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The round directories under `rounds`, `R-` and digits, in round order.
fn round_dirs(rounds: &Path) -> Vec<(u32, String)> {
    let mut found: Vec<(u32, String)> = std::fs::read_dir(rounds)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    round_number(&name).map(|n| (n, name))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

fn round_number(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("R-")?;
    if digits.len() < 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The relative link targets of a Markdown text: `](target)`, without an
/// anchor, skipping URLs and in-page anchors.
fn links(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = markdown;
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = rest[..end].split('#').next().unwrap_or("");
        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            out.push(target.to_string());
        }
        rest = &rest[end..];
    }
    out
}

/// The shape errors of the rounds under `rounds`: each round holds
/// `report.md` and `round.json`; every file `round.json` names under
/// `files` and every relative link of `report.md` exists inside the round
/// directory; every file under `shots/` is named; `trend.md` has one row
/// per round.
fn shape_errors(rounds: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let dirs = round_dirs(rounds);
    if dirs.is_empty() {
        errors.push(format!("{} holds no round R-nn", rounds.display()));
    }
    let trend = std::fs::read_to_string(rounds.join("trend.md")).unwrap_or_else(|_| {
        errors.push("trend.md is missing".to_string());
        String::new()
    });
    for (_, name) in &dirs {
        let dir = rounds.join(name);
        if !trend
            .lines()
            .any(|line| line.starts_with(&format!("| {name} |")))
        {
            errors.push(format!("trend.md has no row for {name}"));
        }
        let mut named = Vec::new();
        match std::fs::read_to_string(dir.join("round.json")) {
            Err(_) => errors.push(format!("{name}/round.json is missing")),
            Ok(text) => match Json::parse(&text) {
                Err(e) => errors.push(format!("{name}/round.json: {e}")),
                Ok(json) => {
                    if json.get("round").and_then(Json::as_str) != Some(name.as_str()) {
                        errors.push(format!("{name}/round.json does not name the round {name}"));
                    }
                    match json.get("files").and_then(Json::as_array) {
                        None => errors.push(format!("{name}/round.json has no `files` list")),
                        Some(files) => {
                            for file in files {
                                match file.as_str() {
                                    Some(file) => named.push(file.to_string()),
                                    None => errors.push(format!(
                                        "{name}/round.json: `files` holds {}",
                                        file.kind()
                                    )),
                                }
                            }
                        }
                    }
                }
            },
        }
        match std::fs::read_to_string(dir.join("report.md")) {
            Err(_) => errors.push(format!("{name}/report.md is missing")),
            Ok(report) => named.extend(links(&report)),
        }
        for file in &named {
            if file.starts_with('/') || file.split('/').any(|part| part == "..") {
                errors.push(format!("{name} names {file}, outside its directory"));
            } else if !dir.join(file).is_file() {
                errors.push(format!("{name} names {file}, which does not exist"));
            }
        }
        if let Ok(shots) = std::fs::read_dir(dir.join("shots")) {
            let mut shots: Vec<String> = shots
                .flatten()
                .map(|entry| format!("shots/{}", entry.file_name().to_string_lossy()))
                .collect();
            shots.sort();
            // The triage and the review agents' reports name their own
            // shots, in links or in code spans.
            let mut notes = String::new();
            for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") && entry.file_name() != "report.md" {
                    notes.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                }
            }
            for shot in shots {
                if !named.contains(&shot) && !notes.contains(shot.as_str()) {
                    errors.push(format!(
                        "{name}/{shot} is named by neither round.json, report.md nor another note of the round"
                    ));
                }
            }
        }
    }
    errors
}

#[test]
fn round_dirs_hold_what_they_name() {
    let errors = shape_errors(&workspace().join("ux/rounds"));
    assert!(errors.is_empty(), "ux/rounds:\n{}", errors.join("\n"));
}

const NOT_RUN: &str = "not run (no person, no keys)";
const STEP_ACTIONS_BAR: i64 = 2;
const RAW: &str = "target/ux/round";

fn int(json: &Json, key: &str) -> i64 {
    json.get(key).and_then(Json::as_i64).unwrap_or(0)
}

fn text<'a>(json: &'a Json, key: &str) -> &'a str {
    json.get(key).and_then(Json::as_str).unwrap_or("")
}

fn obj(entries: Vec<(&str, Json)>) -> Json {
    Json::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn st(value: impl Into<String>) -> Json {
    Json::Str(value.into())
}

fn opt(value: Option<i64>) -> Json {
    value.map_or(Json::Null, Json::Int)
}

fn shown(value: Option<i64>) -> String {
    value.map_or("none".to_string(), |v| v.to_string())
}

/// A session's glance cost: `n/a` for the keyboard, which has none.
fn glance(session: &Json, key: &str) -> String {
    if text(session, "mode") == "keyboard" {
        "n/a".to_string()
    } else {
        int(session, key).to_string()
    }
}

fn cell(value: &str) -> String {
    value.replace('|', "\\|")
}

/// The metric `key` of `scenario` in one part of `target/ux/metrics.json`.
fn metric(metrics: Option<&Json>, part: &str, scenario: &str, key: &str) -> Option<i64> {
    metrics?.get(part)?.get(scenario)?.get(key)?.as_i64()
}

/// A viewport of the round, gating ones first, each group widest first.
struct View {
    name: String,
    gating: bool,
}

fn views(sessions: &[Json]) -> Vec<View> {
    let mut found: Vec<(bool, i64, String)> = Vec::new();
    for session in sessions {
        let name = text(session, "viewport").to_string();
        if !found.iter().any(|(_, _, n)| *n == name) {
            let width = name
                .split('x')
                .next()
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
            found.push((text(session, "group") == "gating", width, name));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    found
        .into_iter()
        .map(|(gating, _, name)| View { name, gating })
        .collect()
}

/// One bar of SKILL.md section 4 at every viewport: the value, whether it
/// meets the bar, and its text; `None` where it was not measured.
struct Bar {
    id: &'static str,
    what: &'static str,
    limit: &'static str,
    cells: Vec<Option<(i64, bool, String)>>,
}

fn zero(value: Option<i64>) -> Option<(i64, bool, String)> {
    value.map(|v| (v, v == 0, v.to_string()))
}

fn bars(metrics: Option<&Json>, sessions: &[Json], views: &[View], tasks: &[String]) -> Vec<Bar> {
    let part = |view: &View| if view.gating { "scenarios" } else { "measured" };
    let at = |view: &View| -> Vec<&Json> {
        sessions
            .iter()
            .filter(|s| text(s, "viewport") == view.name)
            .collect()
    };
    let l2 = |id: &'static str, what: &'static str, key: &'static str| Bar {
        id,
        what,
        limit: "0",
        cells: views
            .iter()
            .map(|view| {
                zero(metric(
                    metrics,
                    part(view),
                    &format!("pages/{}", view.name),
                    key,
                ))
            })
            .collect(),
    };
    let mut out = vec![
        l2(
            "layout.overflow",
            "L2: no horizontal page scroll, nothing past its box",
            "layout.overflow",
        ),
        l2(
            "layout.truncated",
            "L2: no text or control cut off",
            "layout.truncated",
        ),
        l2(
            "always_visible.misses",
            "L2: always-visible controls in view, opaque, uncovered",
            "always_visible.misses",
        ),
        l2(
            "axe.serious_critical",
            "L2: axe-core serious or critical violations",
            "axe.serious_critical",
        ),
        l2(
            "reduced_motion.violations",
            "L2: motion under reduced motion",
            "reduced_motion.violations",
        ),
    ];
    out.push(l2(
        "external_requests",
        "L2: requests off the local server",
        "external_requests",
    ));
    out.push(Bar {
        id: "trial.external_requests",
        what: "trial: requests off the local server",
        limit: "0",
        cells: views
            .iter()
            .map(|view| {
                let here = at(view);
                (!here.is_empty()).then(|| {
                    zero(Some(here.iter().map(|s| int(s, "external_requests")).sum())).unwrap()
                })
            })
            .collect(),
    });
    out.push(Bar {
        id: "suite.tasks_reached",
        what: "L3 suite: the screen-only user reaches each task's truth, pointer and keyboard",
        limit: "all",
        cells: views
            .iter()
            .map(|view| {
                let reached = tasks
                    .iter()
                    .filter(|task| {
                        metric(
                            metrics,
                            part(view),
                            &format!("{task}/{}", view.name),
                            "screen_only.actions",
                        )
                        .is_some()
                    })
                    .count() as i64;
                (reached > 0 || view.gating).then(|| {
                    let all = tasks.len() as i64;
                    (reached, reached == all, format!("{reached}/{all}"))
                })
            })
            .collect(),
    });
    let completed = |keyboard_only: bool| -> Vec<Option<(i64, bool, String)>> {
        views
            .iter()
            .map(|view| {
                let here: Vec<&Json> = at(view)
                    .into_iter()
                    .filter(|s| !keyboard_only || text(s, "mode") == "keyboard")
                    .collect();
                (!here.is_empty()).then(|| {
                    let done = here
                        .iter()
                        .filter(|s| s.get("completed").and_then(Json::as_bool) == Some(true))
                        .count() as i64;
                    let all = here.len() as i64;
                    (done, done == all, format!("{done}/{all}"))
                })
            })
            .collect()
    };
    out.push(Bar {
        id: "trial.sessions_completed",
        what: "trial: sessions that reach every task's truth",
        limit: "all",
        cells: completed(false),
    });
    out.push(Bar {
        id: "trial.keyboard_completed",
        what: "trial: keyboard-alone sessions that reach every task's truth",
        limit: "all",
        cells: completed(true),
    });
    out.push(Bar {
        id: "trial.step_actions.max",
        what: "trial: actions one pointer step takes, max (every fix at most 2 actions)",
        limit: "≤ 2",
        cells: views
            .iter()
            .map(|view| {
                let here: Vec<&Json> = at(view)
                    .into_iter()
                    .filter(|s| text(s, "mode") == "pointer")
                    .collect();
                (!here.is_empty()).then(|| {
                    let max = here
                        .iter()
                        .map(|s| int(s, "step_actions_max"))
                        .max()
                        .unwrap_or(0);
                    (max, max <= STEP_ACTIONS_BAR, max.to_string())
                })
            })
            .collect(),
    });
    out
}

/// A ratchet row: a baseline entry or a measured value the baseline lacks.
struct Ratchet {
    scenario: String,
    metric: String,
    value: Option<i64>,
    baseline: Option<i64>,
    tolerance: Json,
    last: Option<i64>,
    verdict: &'static str,
}

fn tolerance_exceeded(tolerance: &Json, base: i64, value: i64) -> bool {
    match tolerance {
        Json::Int(n) => value > base + n,
        Json::Str(t) => {
            let p: i64 = t.trim_end_matches('%').parse().unwrap_or(0);
            value * 100 > base * (100 + p)
        }
        _ => value > base,
    }
}

fn ratchet(metrics: Option<&Json>, baseline: &Json, last: Option<&Json>) -> Vec<Ratchet> {
    let last_value = |scenario: &str, metric: &str| -> Option<i64> {
        last?.get("ratchet")?.as_array()?.iter().find_map(|row| {
            (text(row, "scenario") == scenario && text(row, "metric") == metric)
                .then(|| row.get("value").and_then(Json::as_i64))
                .flatten()
        })
    };
    let mut rows = Vec::new();
    let base = baseline.get("scenarios").and_then(Json::as_object);
    for (scenario, entries) in base.into_iter().flatten() {
        for (name, entry) in entries.as_object().into_iter().flatten() {
            let value = metric(metrics, "scenarios", scenario, name);
            let baseline = entry.get("value").and_then(Json::as_i64);
            let tolerance = entry.get("tolerance").cloned().unwrap_or(Json::Int(0));
            let verdict = match (value, baseline) {
                (None, _) => "not measured",
                (Some(_), None) => "new",
                (Some(v), Some(b)) if v < b => "better",
                (Some(v), Some(b)) if v == b => "same",
                (Some(v), Some(b)) if !tolerance_exceeded(&tolerance, b, v) => {
                    "worse within tolerance"
                }
                _ => "worse beyond tolerance",
            };
            rows.push(Ratchet {
                scenario: scenario.clone(),
                metric: name.clone(),
                value,
                baseline,
                tolerance,
                last: last_value(scenario, name),
                verdict,
            });
        }
    }
    let measured = metrics
        .and_then(|m| m.get("scenarios"))
        .and_then(Json::as_object);
    for (scenario, values) in measured.into_iter().flatten() {
        for (name, value) in values.as_object().into_iter().flatten() {
            let known = base
                .and_then(|b| b.get(scenario))
                .and_then(|s| s.get(name))
                .is_some();
            if !known {
                rows.push(Ratchet {
                    scenario: scenario.clone(),
                    metric: name.clone(),
                    value: value.as_i64(),
                    baseline: None,
                    tolerance: Json::Null,
                    last: last_value(scenario, name),
                    verdict: "new",
                });
            }
        }
    }
    rows
}

/// A moment of the trial worth a look: a session that stopped, or a step.
#[derive(Clone)]
struct Worst {
    session: String,
    mode: String,
    viewport: String,
    gating: bool,
    segment: String,
    label: String,
    page: String,
    looks: i64,
    glance: i64,
    actions: i64,
    stopped: bool,
    frames: Vec<(&'static str, i64)>,
}

/// Every moment of the trial: one per session that stopped (its last
/// frame) and one per step, with the frame it began to look on (`start`)
/// and the frame it found the control on (`found`, when that differs).
fn moments(sessions: &[Json]) -> Vec<Worst> {
    let mut out = Vec::new();
    for session in sessions {
        let id = text(session, "session").to_string();
        let mode = text(session, "mode").to_string();
        let viewport = text(session, "viewport").to_string();
        let gating = text(session, "group") == "gating";
        let segments = session
            .get("segments")
            .and_then(Json::as_array)
            .unwrap_or(&[]);
        let frames = session
            .get("frames")
            .and_then(Json::as_array)
            .unwrap_or(&[]);
        if session.get("completed").and_then(Json::as_bool) != Some(true) {
            let stopped = segments.iter().find(|s| s.get("error").is_some());
            let last = frames.last();
            out.push(Worst {
                session: id.clone(),
                mode: mode.clone(),
                viewport: viewport.clone(),
                gating,
                segment: stopped.map_or(String::new(), |s| text(s, "id").to_string()),
                label: stopped.map_or("stopped".to_string(), |s| text(s, "error").to_string()),
                page: last.map_or(String::new(), |f| text(f, "page").to_string()),
                looks: 0,
                glance: 0,
                actions: 0,
                stopped: true,
                frames: last.map_or(Vec::new(), |f| vec![("stopped", int(f, "n"))]),
            });
        }
        let steps = session.get("steps").and_then(Json::as_array).unwrap_or(&[]);
        for (index, step) in steps.iter().enumerate() {
            let segment = segments
                .iter()
                .find(|s| {
                    let first = int(s, "first_step") as usize;
                    index >= first && index < first + int(s, "steps") as usize
                })
                .map_or(String::new(), |s| {
                    format!("{} {}", text(s, "kind"), text(s, "id"))
                });
            let frame = int(step, "frame");
            let mut shots = vec![("start", frame)];
            if let Some(found) = step.get("found").and_then(Json::as_i64)
                && found != frame
            {
                shots.push(("found", found));
            }
            out.push(Worst {
                session: id.clone(),
                mode: mode.clone(),
                viewport: viewport.clone(),
                gating,
                segment,
                label: text(step, "label").to_string(),
                page: text(step, "page").to_string(),
                looks: int(step, "looks"),
                glance: int(step, "glance"),
                actions: int(step, "actions"),
                stopped: false,
                frames: shots,
            });
        }
    }
    out
}

fn costlier(a: &Worst, b: &Worst) -> std::cmp::Ordering {
    b.looks
        .cmp(&a.looks)
        .then(b.glance.cmp(&a.glance))
        .then(b.actions.cmp(&a.actions))
        .then(a.session.cmp(&b.session))
        .then(a.frames[0].1.cmp(&b.frames[0].1))
}

/// The worst moments, per viewport in the round's order: the sessions that
/// stopped, then the costliest pointer step and the costliest keyboard
/// step.
fn worst(all: &[Worst], views: &[View]) -> Vec<Worst> {
    let mut out = Vec::new();
    for view in views {
        let here: Vec<&Worst> = all.iter().filter(|m| m.viewport == view.name).collect();
        out.extend(here.iter().filter(|m| m.stopped).map(|m| (*m).clone()));
        for mode in ["pointer", "keyboard"] {
            let mut steps: Vec<&Worst> = here
                .iter()
                .copied()
                .filter(|m| !m.stopped && m.mode == mode)
                .collect();
            steps.sort_by(|a, b| costlier(a, b));
            if let Some(first) = steps.first() {
                out.push((*first).clone());
            }
        }
    }
    out
}

fn read_sessions(raw: &Path) -> Vec<Json> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(raw.join("sessions"))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.retain(|f| f.extension().is_some_and(|e| e == "json"));
    files.sort();
    files.iter().map(|f| parse(f)).collect()
}

fn task_ids(ux: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(ux.join("scenarios"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().join("task.json").is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

fn trend_row(trend: &Json) -> String {
    let field = |key: &str| match trend.get(key) {
        Some(Json::Int(n)) => n.to_string(),
        Some(Json::Str(s)) => cell(s),
        _ => "none".to_string(),
    };
    format!(
        "| {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
        field("round"),
        field("gating_suite"),
        field("sessions_completed"),
        field("sessions"),
        field("bars_missed"),
        field("ratchet_worse"),
        field("ratchet_better"),
        field("trial_pointer_actions"),
        field("trial_keyboard_actions"),
        field("worst_pointer_looks"),
        field("worst_keyboard_tabs"),
        field("questionnaire"),
        field("l5_probe"),
    )
}

/// What a round is written from: the gating suite's output, the trial's
/// sessions and contact sheets, the cards, the baseline and the last round.
struct Inputs {
    round: String,
    /// `target/ux/metrics.json`.
    metrics: Option<Json>,
    /// `target/ux/gating.json`, written by `ux/lib/summary-reporter.ts`.
    gating: Option<Json>,
    baseline: Json,
    last: Option<(String, Json)>,
    sessions: Vec<Json>,
    /// `target/ux/round/contact/<viewport>.json` by viewport.
    contacts: BTreeMap<String, Json>,
    /// `ux/scenarios/cards/<id>.json` by id.
    cards: BTreeMap<String, Json>,
    tasks: Vec<String>,
    /// The screenshots of missed gating-suite bars, `target/ux/missed/…`,
    /// by bar id and viewport: workspace-relative paths.
    bar_shots: BTreeMap<(String, String), String>,
}

/// A rendered round: the two files, the trend row, and the copies into
/// `shots/` (a workspace-relative source, a round-relative target).
struct Output {
    report: String,
    round: Json,
    trend: Json,
    copies: Vec<(String, String)>,
}

fn frame_file(session: &str, frame: i64) -> String {
    format!("{RAW}/frames/{session}/{frame:03}.jpg")
}

/// Renders a round. `exists` tells whether a workspace-relative file is
/// there to copy; a missing one is named under the missing inputs.
fn render(inputs: &Inputs, exists: &dyn Fn(&str) -> bool) -> Output {
    let round = inputs.round.as_str();
    let gating = inputs.gating.as_ref();
    // A metrics.json is this run's only next to the gating suite's own
    // report; without one it may be an earlier run's and is not used.
    let metrics = gating.and(inputs.metrics.as_ref());
    let last = inputs.last.as_ref().map(|(_, json)| json);
    let last_name = inputs.last.as_ref().map(|(name, _)| name.clone());
    let sessions = &inputs.sessions;
    let mut missing: Vec<String> = Vec::new();
    let gating_line = match gating {
        None => {
            missing.push("gating suite: target/ux/gating.json is missing, so it did not report and target/ux/metrics.json is not used: L2, L3 and the ratchet are not measured".to_string());
            "did not report (no `target/ux/gating.json`); its metrics are not used".to_string()
        }
        Some(g) => {
            if metrics.is_none() {
                missing.push("gating suite: target/ux/metrics.json is missing, so L2, L3 and the ratchet are not measured".to_string());
            }
            format!(
                "{}, {} failed, {} passed, {} flaky, {} skipped",
                text(g, "status"),
                int(g, "failed"),
                int(g, "passed"),
                int(g, "flaky"),
                int(g, "skipped")
            )
        }
    };
    let failed_tests: Vec<String> = gating
        .and_then(|g| g.get("failed_tests"))
        .and_then(Json::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| t.as_str().map(str::to_string))
        .collect();
    let views = views(sessions);
    let bars = bars(metrics, sessions, &views, &inputs.tasks);
    let ratchet = ratchet(metrics, &inputs.baseline, last);
    let mut files = vec!["report.md".to_string()];
    let mut copies: Vec<(String, String)> = Vec::new();
    let mut take = |from: String, to: String, missing: &mut Vec<String>| -> bool {
        if exists(&from) {
            files.push(to.clone());
            copies.push((from, to));
            true
        } else {
            missing.push(format!("{from} is missing"));
            false
        }
    };

    let all = moments(sessions);
    let mut worst = worst(&all, &views);
    // The moment that shows each missed trial bar, as an index into `worst`.
    let mut bar_moments: BTreeMap<(String, String), usize> = BTreeMap::new();
    for bar in &bars {
        for (c, view) in bar.cells.iter().zip(&views) {
            if !view.gating || !matches!(c, Some((_, false, _))) {
                continue;
            }
            let pick: Option<Worst> = match bar.id {
                "trial.step_actions.max" => {
                    let mut steps: Vec<&Worst> = all
                        .iter()
                        .filter(|m| !m.stopped && m.viewport == view.name && m.mode == "pointer")
                        .collect();
                    steps.sort_by(|a, b| b.actions.cmp(&a.actions).then(costlier(a, b)));
                    steps.first().map(|m| (*m).clone())
                }
                "trial.sessions_completed" | "trial.keyboard_completed" => all
                    .iter()
                    .find(|m| {
                        m.stopped
                            && m.viewport == view.name
                            && (bar.id == "trial.sessions_completed" || m.mode == "keyboard")
                    })
                    .cloned(),
                _ => None,
            };
            if let Some(pick) = pick {
                let at = worst
                    .iter()
                    .position(|w| w.session == pick.session && w.frames == pick.frames)
                    .unwrap_or_else(|| {
                        worst.push(pick);
                        worst.len() - 1
                    });
                bar_moments.insert((bar.id.to_string(), view.name.clone()), at);
            }
        }
    }

    let mut worst_json = Vec::new();
    let mut worst_md = String::new();
    let mut worst_shots: Vec<Vec<(&'static str, String)>> = Vec::new();
    for (k, moment) in worst.iter().enumerate() {
        let rank = k + 1;
        let mut shots_json = Vec::new();
        let mut shots = Vec::new();
        let mut images = String::new();
        for (what, frame) in &moment.frames {
            let name = format!("shots/worst-{rank}-{what}.jpg");
            if take(
                frame_file(&moment.session, *frame),
                name.clone(),
                &mut missing,
            ) {
                shots_json.push(obj(vec![
                    ("what", st(*what)),
                    ("frame", Json::Int(*frame)),
                    ("shot", st(name.clone())),
                ]));
                let _ = write!(images, "\n   ![{what}, frame {frame}]({name})");
                shots.push((*what, name));
            }
        }
        worst_shots.push(shots);
        let keyboard = moment.mode == "keyboard";
        let _ = writeln!(
            worst_md,
            "{rank}. `{}` ({} at {}), {}: `{}` on `{}`, {} looks{}, glance {}, {} actions.{images}",
            moment.session,
            moment.mode,
            moment.viewport,
            if moment.segment.is_empty() {
                "session".to_string()
            } else {
                moment.segment.clone()
            },
            cell(&moment.label),
            moment.page,
            moment.looks,
            if keyboard { " (Tab presses)" } else { "" },
            if keyboard {
                "n/a".to_string()
            } else {
                moment.glance.to_string()
            },
            moment.actions,
        );
        worst_json.push(obj(vec![
            ("rank", Json::Int(rank as i64)),
            ("session", st(moment.session.clone())),
            ("mode", st(moment.mode.clone())),
            ("viewport", st(moment.viewport.clone())),
            ("segment", st(moment.segment.clone())),
            ("label", st(moment.label.clone())),
            ("page", st(moment.page.clone())),
            ("looks", Json::Int(moment.looks)),
            (
                "glance",
                if keyboard {
                    Json::Null
                } else {
                    Json::Int(moment.glance)
                },
            ),
            ("actions", Json::Int(moment.actions)),
            ("shots", Json::Array(shots_json)),
        ]));
    }

    // The screenshot of every missed gating bar: a worst moment's, a
    // stopped or requesting session's last frame, or the gating suite's.
    let mut bar_links: BTreeMap<(String, String), String> = BTreeMap::new();
    for bar in &bars {
        for (c, view) in bar.cells.iter().zip(&views) {
            if !view.gating || !matches!(c, Some((_, false, _))) {
                continue;
            }
            let key = (bar.id.to_string(), view.name.clone());
            let shot = if let Some(at) = bar_moments.get(&key) {
                let shots = &worst_shots[*at];
                shots
                    .iter()
                    .find(|(what, _)| *what == "found")
                    .or(shots.first())
                    .map(|(_, name)| name.clone())
            } else if bar.id == "trial.external_requests" {
                sessions
                    .iter()
                    .find(|s| text(s, "viewport") == view.name && int(s, "external_requests") > 0)
                    .and_then(|s| {
                        let last = s.get("frames")?.as_array()?.last()?;
                        let name = format!("shots/bar-{}-{}.jpg", bar.id, view.name);
                        take(
                            frame_file(text(s, "session"), int(last, "n")),
                            name.clone(),
                            &mut missing,
                        )
                        .then_some(name)
                    })
            } else {
                inputs.bar_shots.get(&key).and_then(|from| {
                    let name = format!("shots/bar-{}-{}.jpg", bar.id, view.name);
                    take(from.clone(), name.clone(), &mut missing).then_some(name)
                })
            };
            match shot {
                Some(shot) => {
                    bar_links.insert(key, shot);
                }
                None => missing.push(format!(
                    "bar {} missed at {} without a screenshot",
                    bar.id, view.name
                )),
            }
        }
    }

    let mut sheets_json = Vec::new();
    let mut sheets_md = String::new();
    for view in &views {
        let Some(contact) = inputs.contacts.get(&view.name) else {
            missing.push(format!(
                "contact sheet {}: {RAW}/contact/{}.json is missing",
                view.name, view.name
            ));
            continue;
        };
        for tile in contact.get("tiles").and_then(Json::as_array).unwrap_or(&[]) {
            if tile.get("file").and_then(Json::as_str).is_none() {
                missing.push(format!(
                    "contact sheet {}: {} in {} not reached: {}",
                    view.name,
                    text(tile, "task"),
                    text(tile, "theme"),
                    text(tile, "error")
                ));
            }
        }
        let name = format!("shots/contact-{}.jpg", view.name);
        if take(
            text(contact, "sheet").to_string(),
            name.clone(),
            &mut missing,
        ) {
            sheets_json.push(obj(vec![
                ("viewport", st(view.name.clone())),
                ("gating", Json::Bool(view.gating)),
                ("shot", st(name.clone())),
            ]));
            let _ = writeln!(
                sheets_md,
                "### {}{}\n\n![key pages at {}]({name})\n",
                view.name,
                if view.gating { "" } else { " (measured only)" },
                view.name
            );
        }
    }

    for session in sessions {
        if session.get("completed").and_then(Json::as_bool) != Some(true) {
            missing.push(format!(
                "session {} stopped after {} of {} tasks",
                text(session, "session"),
                int(session, "tasks_done"),
                int(session, "tasks")
            ));
        }
    }
    let mut inputs_md = vec![
        format!("questionnaire: {NOT_RUN}"),
        format!("L5 probe: {NOT_RUN}"),
        "L4 keyed replay: not run (no keys)".to_string(),
        "person: none; the screen-only user of `ux/lib/bot.ts` drove every session (SKILL.md section 3.1, a driver bot)".to_string(),
        "free session: not run (optional)".to_string(),
        "rrweb: not in `ux/package-lock.json`; every session is recorded as a Playwright trace instead, `target/ux/round/traces/<session>.zip` (`npx playwright show-trace`), not committed for its size; `task ux-round` records it again".to_string(),
        "review agents: `review-heuristics.md` and `review-report.md` are written into this directory after this command".to_string(),
    ];
    if last.is_none() {
        inputs_md.push(format!("last round: none, {round} is the first"));
    }
    inputs_md.extend(missing);

    let gating_missed = bars
        .iter()
        .flat_map(|bar| bar.cells.iter().zip(&views))
        .filter(|(c, v)| v.gating && matches!(c, Some((_, false, _))))
        .count() as i64;
    let worse = ratchet
        .iter()
        .filter(|r| r.verdict == "worse beyond tolerance")
        .count() as i64;
    let better = ratchet.iter().filter(|r| r.verdict == "better").count() as i64;
    let gating_sum = |mode: &str| -> i64 {
        sessions
            .iter()
            .filter(|s| text(s, "group") == "gating" && text(s, "mode") == mode)
            .map(|s| int(s, "actions"))
            .sum()
    };
    let worst_of = |mode: &str| {
        all.iter()
            .filter(|w| w.gating && !w.stopped && w.mode == mode)
            .map(|w| w.looks)
            .max()
    };
    let completed = sessions
        .iter()
        .filter(|s| s.get("completed").and_then(Json::as_bool) == Some(true))
        .count() as i64;
    let trend = obj(vec![
        ("round", st(round)),
        ("sessions", Json::Int(sessions.len() as i64)),
        ("sessions_completed", Json::Int(completed)),
        ("bars_missed", Json::Int(gating_missed)),
        ("ratchet_worse", Json::Int(worse)),
        ("ratchet_better", Json::Int(better)),
        ("trial_pointer_actions", Json::Int(gating_sum("pointer"))),
        ("trial_keyboard_actions", Json::Int(gating_sum("keyboard"))),
        ("worst_pointer_looks", opt(worst_of("pointer"))),
        ("worst_keyboard_tabs", opt(worst_of("keyboard"))),
        (
            "gating_suite",
            st(gating.map_or("did not report", |g| text(g, "status"))),
        ),
        ("questionnaire", st(NOT_RUN)),
        ("l5_probe", st(NOT_RUN)),
    ]);

    let mut md = String::new();
    let _ = writeln!(md, "# UX round {round}\n");
    let _ = writeln!(
        md,
        "Written by `task ux-round ROUND={round}` (`site::round::write`) from the gating suite (`task ux`'s Playwright run, `target/ux/metrics.json`) and the trial (`ux/round/sessions.spec.ts`: {} sessions, the cards below at every viewport). Do not edit; run the command again.\n",
        sessions.len()
    );
    let _ = writeln!(
        md,
        "- Gating suite: {gating_line}.\n- Last round: {}.\n- Questionnaire: {NOT_RUN}.\n- L5 probe: {NOT_RUN}.\n- Recording: Playwright traces (rrweb is not in the lockfile).\n- Gating viewports: {}. Measured only, reported and not gated: {}.\n- Frames: each taken 0.75 s after its moment (the start, every click, wheel turn, opened menu, typed text and Enter, and the keyboard focus reaching a control before Enter), at `prefers-reduced-motion: reduce`.\n",
        last_name.clone().unwrap_or_else(|| "none".to_string()),
        views
            .iter()
            .filter(|v| v.gating)
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        views
            .iter()
            .filter(|v| !v.gating)
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    );
    if !failed_tests.is_empty() {
        let _ = writeln!(md, "The gating suite's failed tests:\n");
        for test in &failed_tests {
            let _ = writeln!(md, "- {test}");
        }
        md.push('\n');
    }
    let _ = writeln!(
        md,
        "## Bars\n\nSKILL.md section 4. A gating cell is `met` or `missed`, a missed one with its screenshot; a measured-only cell is `reported`; `not measured` where nothing ran.\n"
    );
    let _ = write!(md, "| bar | metric | limit |");
    for view in &views {
        let _ = write!(
            md,
            " {}{} |",
            view.name,
            if view.gating { "" } else { " (measured)" }
        );
    }
    let _ = write!(md, "\n|---|---|---|");
    for _ in &views {
        md.push_str("---|");
    }
    md.push('\n');
    let mut bars_json = Vec::new();
    for bar in &bars {
        let _ = write!(md, "| {} | `{}` | {} |", bar.what, bar.id, bar.limit);
        let mut cells_json = Vec::new();
        for (c, view) in bar.cells.iter().zip(&views) {
            let verdict = match c {
                None => "not measured",
                Some(_) if !view.gating => "reported",
                Some((_, true, _)) => "met",
                Some((_, false, _)) => "missed",
            };
            let shot = bar_links.get(&(bar.id.to_string(), view.name.clone()));
            match (c, shot) {
                (None, _) => {
                    let _ = write!(md, " not measured |");
                }
                (Some((_, _, t)), Some(shot)) => {
                    let _ = write!(md, " {t} {verdict} ([shot]({shot})) |");
                }
                (Some((_, _, t)), None) => {
                    let _ = write!(md, " {t} {verdict} |");
                }
            }
            cells_json.push(obj(vec![
                ("viewport", st(view.name.clone())),
                ("gating", Json::Bool(view.gating)),
                ("shot", shot.map_or(Json::Null, |s| st(s.clone()))),
                ("value", opt(c.as_ref().map(|c| c.0))),
                ("shown", c.as_ref().map_or(Json::Null, |c| st(c.2.clone()))),
                ("verdict", st(verdict)),
            ]));
        }
        md.push('\n');
        bars_json.push(obj(vec![
            ("id", st(bar.id)),
            ("bar", st(bar.what)),
            ("limit", st(bar.limit)),
            ("cells", Json::Array(cells_json)),
        ]));
    }
    let _ = writeln!(
        md,
        "\n## Ratchet\n\nThe gating suite's values against `ux/baseline.json` (`site::ux::ratchet` gates them) and the last round ({}). Lower is better.\n\n| scenario | metric | {round} | baseline | tolerance | last round | verdict |\n|---|---|---|---|---|---|---|",
        last_name.clone().unwrap_or_else(|| "none".to_string())
    );
    let mut ratchet_json = Vec::new();
    for row in &ratchet {
        let tolerance = match &row.tolerance {
            Json::Int(n) => n.to_string(),
            Json::Str(s) => s.clone(),
            _ => "none".to_string(),
        };
        let _ = writeln!(
            md,
            "| {} | `{}` | {} | {} | {} | {} | {} |",
            row.scenario,
            row.metric,
            shown(row.value),
            shown(row.baseline),
            tolerance,
            shown(row.last),
            row.verdict
        );
        ratchet_json.push(obj(vec![
            ("scenario", st(row.scenario.clone())),
            ("metric", st(row.metric.clone())),
            ("value", opt(row.value)),
            ("baseline", opt(row.baseline)),
            ("tolerance", row.tolerance.clone()),
            ("last", opt(row.last)),
            ("verdict", st(row.verdict)),
        ]));
    }
    let _ = writeln!(
        md,
        "\n## Measured-only viewports\n\nThe gating suite's L2 at the measured-only viewports, reported and not gated.\n\n| scenario | metric | {round} |\n|---|---|---|"
    );
    let mut measured_json = BTreeMap::new();
    if let Some(measured) = metrics
        .and_then(|m| m.get("measured"))
        .and_then(Json::as_object)
    {
        for (scenario, values) in measured {
            for (name, value) in values.as_object().into_iter().flatten() {
                let _ = writeln!(md, "| {scenario} | `{name}` | {} |", shown(value.as_i64()));
            }
            measured_json.insert(scenario.clone(), values.clone());
        }
    }

    let _ = writeln!(
        md,
        "\n## Trial\n\nThree cards (SKILL.md section 3.1): two anchors, kept fixed for later rounds, and the rotating card of {round}, under `ux/scenarios/cards/`. Each runs as one session per viewport, from the landing page, in the theme its card starts in; the theme is set once, so a switch stays on later pages.\n\n| card | kind | mode | theme | persona | tasks, in order | twists |\n|---|---|---|---|---|---|---|"
    );
    let mut card_ids: Vec<String> = sessions
        .iter()
        .map(|s| text(s, "card").to_string())
        .collect();
    card_ids.dedup();
    let mut cards_json = Vec::new();
    let no_card = Json::Object(BTreeMap::new());
    for id in &card_ids {
        let card = inputs.cards.get(id).unwrap_or(&no_card);
        let tasks: Vec<&str> = card
            .get("tasks")
            .and_then(Json::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(Json::as_str)
            .collect();
        let twists: Vec<String> = card
            .get("twists")
            .and_then(Json::as_array)
            .unwrap_or(&[])
            .iter()
            .map(|t| {
                format!(
                    "{} (before {}): {}",
                    text(t, "id"),
                    text(t, "before"),
                    text(t, "why")
                )
            })
            .collect();
        let _ = writeln!(
            md,
            "| `ux/scenarios/cards/{id}.json` | {} | {} | {} | {} | {} | {} |",
            text(card, "kind"),
            text(card, "mode"),
            text(card, "theme"),
            cell(text(card, "persona")),
            tasks.join(", "),
            cell(&twists.join("; ")),
        );
        cards_json.push(obj(vec![
            ("id", st(id.clone())),
            ("kind", st(text(card, "kind"))),
            ("mode", st(text(card, "mode"))),
            ("theme", st(text(card, "theme"))),
            ("file", st(format!("ux/scenarios/cards/{id}.json"))),
        ]));
    }
    let _ = writeln!(
        md,
        "\nLooks count each inspection of the screen; for the keyboard they are Tab presses. Glance cost (pointer only, `n/a` for the keyboard): 1 a control in the primary region, 2 elsewhere in view, 3 only after a scroll or an opened menu.\n\n| session | viewport | completed | tasks | actions | last round | looks | steers | time to act, max | glance median | glance p95 | step actions, max |\n|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    let mut sessions_json = Vec::new();
    let mut ordered: Vec<&Json> = sessions.iter().collect();
    let position = |s: &Json| {
        views
            .iter()
            .position(|v| v.name == text(s, "viewport"))
            .unwrap_or(0)
    };
    ordered.sort_by(|a, b| {
        text(a, "card")
            .cmp(text(b, "card"))
            .then(position(a).cmp(&position(b)))
    });
    for session in ordered {
        let id = text(session, "session");
        let last_actions = last
            .and_then(|l| l.get("sessions"))
            .and_then(Json::as_array)
            .and_then(|all| {
                all.iter()
                    .find(|s| text(s, "session") == id)
                    .and_then(|s| s.get("actions"))
                    .and_then(Json::as_i64)
            });
        let done = session.get("completed").and_then(Json::as_bool) == Some(true);
        let _ = writeln!(
            md,
            "| `{id}` | {}{} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} |",
            text(session, "viewport"),
            if text(session, "group") == "gating" {
                ""
            } else {
                " (measured)"
            },
            if done { "yes" } else { "no" },
            int(session, "tasks_done"),
            int(session, "tasks"),
            int(session, "actions"),
            shown(last_actions),
            int(session, "looks"),
            int(session, "steers"),
            int(session, "time_to_act_max"),
            glance(session, "glance_median"),
            glance(session, "glance_p95"),
            int(session, "step_actions_max"),
        );
        let mut entry = vec![("session", st(id)), ("completed", Json::Bool(done))];
        for key in [
            "card", "kind", "mode", "theme", "viewport", "group", "trace",
        ] {
            entry.push((key, st(text(session, key))));
        }
        for key in [
            "tasks_done",
            "tasks",
            "actions",
            "looks",
            "steers",
            "time_to_act_max",
            "step_actions_max",
            "external_requests",
        ] {
            entry.push((key, Json::Int(int(session, key))));
        }
        let keyboard = text(session, "mode") == "keyboard";
        for key in ["glance_median", "glance_p95"] {
            entry.push((
                key,
                if keyboard {
                    Json::Null
                } else {
                    Json::Int(int(session, key))
                },
            ));
        }
        entry.push(("last_actions", opt(last_actions)));
        entry.push((
            "segments",
            session
                .get("segments")
                .cloned()
                .unwrap_or(Json::Array(Vec::new())),
        ));
        sessions_json.push(obj(entry));
    }
    let _ = writeln!(
        md,
        "\n## Worst moments\n\nPer viewport: the sessions that stopped, then the costliest pointer step and the costliest keyboard step (most looks, then glance, then actions); then the step behind each missed trial bar. `start` is the screen the user faced when it began to look for the control, `found` the screen on which it found it (for the keyboard, the frame with the focus on it, before Enter).\n"
    );
    md.push_str(if worst_md.is_empty() {
        "None.\n"
    } else {
        &worst_md
    });
    let _ = writeln!(
        md,
        "\n## Contact sheets\n\nThe landing page and each task's truth page, reached by the pointer user, in the light and the dark theme, each 0.75 s after it was reached.\n"
    );
    md.push_str(&sheets_md);
    let _ = writeln!(
        md,
        "## Questionnaire\n\n{NOT_RUN}.\n\n## L5 probe misses\n\n{NOT_RUN}.\n\n## Missing inputs\n"
    );
    for input in &inputs_md {
        let _ = writeln!(md, "- {input}");
    }

    let round_json = obj(vec![
        ("round", st(round)),
        ("command", st(format!("task ux-round ROUND={round}"))),
        ("last_round", last_name.clone().map_or(Json::Null, st)),
        ("questionnaire", st(NOT_RUN)),
        ("l5_probe", st(NOT_RUN)),
        ("recording", st("playwright-trace")),
        ("frame_delay_ms", Json::Int(750)),
        (
            "gating_suite",
            gating
                .cloned()
                .unwrap_or_else(|| obj(vec![("status", st("did not report"))])),
        ),
        (
            "viewports",
            Json::Array(
                views
                    .iter()
                    .map(|v| {
                        obj(vec![
                            ("name", st(v.name.clone())),
                            ("gating", Json::Bool(v.gating)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("bars", Json::Array(bars_json)),
        ("ratchet", Json::Array(ratchet_json)),
        ("measured", Json::Object(measured_json)),
        ("cards", Json::Array(cards_json)),
        ("sessions", Json::Array(sessions_json)),
        ("worst", Json::Array(worst_json)),
        ("contact_sheets", Json::Array(sheets_json)),
        (
            "missing_inputs",
            Json::Array(inputs_md.iter().map(|i| st(i.clone())).collect()),
        ),
        (
            "files",
            Json::Array(files.iter().map(|f| st(f.clone())).collect()),
        ),
        ("trend", trend.clone()),
    ]);
    Output {
        report: md,
        round: round_json,
        trend,
        copies,
    }
}

fn trend_md(rows: &[Json]) -> String {
    let mut out = String::from(
        "# UX rounds\n\nOne row per round, rebuilt by `task ux-round` from each `R-nn/round.json`. Bars missed count gating cells only; trial actions and the worst looks and Tab presses are taken at the gating viewports only.\n\n| round | gating suite | sessions completed | bars missed | ratchet worse | ratchet better | trial pointer actions | trial keyboard actions | worst pointer looks | worst keyboard tabs | questionnaire | L5 probe |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for row in rows {
        out.push_str(&trend_row(row));
        out.push('\n');
    }
    out
}

fn read_dir_json(dir: &Path) -> BTreeMap<String, Json> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            out.insert(stem, parse(&path));
        }
    }
    out
}

/// The screenshots of missed gating-suite bars under `target/ux/missed/
/// <viewport>/<bar>/`, the first by name per bar and viewport.
fn bar_shots(root: &Path) -> BTreeMap<(String, String), String> {
    let mut out = BTreeMap::new();
    let base = root.join("target/ux/missed");
    let mut found: Vec<PathBuf> = Vec::new();
    for viewport in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        for bar in std::fs::read_dir(viewport.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            for shot in std::fs::read_dir(bar.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                found.push(shot.path());
            }
        }
    }
    found.sort();
    for path in found {
        let rel = path.strip_prefix(root).unwrap();
        let parts: Vec<String> = rel
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        if let [_, _, _, viewport, bar, _] = parts.as_slice() {
            out.entry((bar.clone(), viewport.clone()))
                .or_insert_with(|| parts.join("/"));
        }
    }
    out
}

/// Removes the shots this command wrote into `dir` before, the ones its
/// old `round.json` names; every other file (triage, the review agents'
/// reports and their shots) stays.
fn clear_written(dir: &Path) {
    std::fs::create_dir_all(dir.join("shots")).unwrap();
    if let Ok(old) = std::fs::read_to_string(dir.join("round.json"))
        && let Ok(old) = Json::parse(&old)
    {
        for file in old.get("files").and_then(Json::as_array).unwrap_or(&[]) {
            if let Some(file) = file.as_str().filter(|f| f.starts_with("shots/")) {
                let _ = std::fs::remove_file(dir.join(file));
            }
        }
    }
}

/// Writes `ux/rounds/$UX_ROUND/` from the output of the gating suite and
/// the trial under `target/ux/`, and rebuilds `ux/rounds/trend.md`. It
/// replaces `report.md`, `round.json` and `shots/<written>` and leaves
/// every other file of the round directory (triage, the review agents'
/// reports and their shots).
#[test]
#[ignore = "run by task ux-round after the Playwright trial"]
fn write() {
    let round = std::env::var("UX_ROUND").unwrap_or_default();
    let number = round_number(&round)
        .unwrap_or_else(|| panic!("UX_ROUND is {round:?}, not a round id like R-01"));
    let root = workspace();
    let ux = root.join("ux");
    let raw = root.join(RAW);
    let rounds = ux.join("rounds");
    let dir = rounds.join(&round);

    let optional = |path: PathBuf| path.is_file().then(|| parse(&path));
    let previous = round_dirs(&rounds)
        .into_iter()
        .rfind(|(n, name)| *n < number && rounds.join(name).join("round.json").is_file());
    let sessions = read_sessions(&raw);
    assert!(
        !sessions.is_empty(),
        "{} holds no session: run the trial first (task ux-round ROUND={round})",
        raw.join("sessions").display()
    );
    let inputs = Inputs {
        round: round.clone(),
        metrics: optional(root.join("target/ux/metrics.json")),
        gating: optional(root.join("target/ux/gating.json")),
        baseline: parse(&ux.join("baseline.json")),
        last: previous.map(|(_, name)| {
            let json = parse(&rounds.join(&name).join("round.json"));
            (name, json)
        }),
        sessions,
        contacts: read_dir_json(&raw.join("contact")),
        cards: read_dir_json(&ux.join("scenarios/cards")),
        tasks: task_ids(&ux),
        bar_shots: bar_shots(&root),
    };
    let output = render(&inputs, &|file| root.join(file).is_file());

    clear_written(&dir);
    for (from, to) in &output.copies {
        std::fs::copy(root.join(from), dir.join(to))
            .unwrap_or_else(|e| panic!("{from} to {to}: {e}"));
    }
    std::fs::write(dir.join("report.md"), &output.report).unwrap();
    let mut text = String::new();
    output.round.write(&mut text, 0);
    text.push('\n');
    std::fs::write(dir.join("round.json"), text).unwrap();

    let mut rows = Vec::new();
    for (_, name) in round_dirs(&rounds) {
        if name == round {
            rows.push(output.trend.clone());
        } else if let Some(row) = std::fs::read_to_string(rounds.join(&name).join("round.json"))
            .ok()
            .and_then(|t| Json::parse(&t).ok())
            .and_then(|j| j.get("trend").cloned())
        {
            rows.push(row);
        }
    }
    std::fs::write(rounds.join("trend.md"), trend_md(&rows)).unwrap();

    let errors = shape_errors(&rounds);
    assert!(
        errors.is_empty(),
        "the round just written:\n{}",
        errors.join("\n")
    );
}

/// Synthetic rounds for the writer's rules (review-report R-01 section 3).
mod rules {
    use super::*;

    fn step(label: &str, page: &str, frame: i64, looks: i64, actions: i64, found: i64) -> Json {
        obj(vec![
            ("label", st(label)),
            ("page", st(page)),
            ("frame", Json::Int(frame)),
            ("found", Json::Int(found)),
            ("looks", Json::Int(looks)),
            ("glance", Json::Int(3)),
            ("actions", Json::Int(actions)),
        ])
    }

    fn session(card: &str, mode: &str, viewport: &str, gating: bool, steps: Vec<Json>) -> Json {
        let actions_max = steps.iter().map(|s| int(s, "actions")).max().unwrap_or(0);
        let looks: i64 = steps.iter().map(|s| int(s, "looks")).sum();
        let frames: Vec<Json> = (1..=40)
            .map(|n| {
                obj(vec![
                    ("n", Json::Int(n)),
                    ("label", st("moment")),
                    ("page", st("index.html")),
                    ("theme", st("light")),
                ])
            })
            .collect();
        obj(vec![
            ("session", st(format!("{card}-{viewport}"))),
            ("card", st(card)),
            ("kind", st("anchor")),
            ("mode", st(mode)),
            ("theme", st("light")),
            ("viewport", st(viewport)),
            ("group", st(if gating { "gating" } else { "measured" })),
            ("completed", Json::Bool(true)),
            ("tasks_done", Json::Int(1)),
            ("tasks", Json::Int(1)),
            ("actions", Json::Int(looks)),
            ("looks", Json::Int(looks)),
            ("steers", Json::Int(0)),
            ("time_to_act_max", Json::Int(actions_max)),
            (
                "glance_median",
                Json::Int(if mode == "pointer" { 3 } else { 0 }),
            ),
            (
                "glance_p95",
                Json::Int(if mode == "pointer" { 3 } else { 0 }),
            ),
            ("step_actions_max", Json::Int(actions_max)),
            ("external_requests", Json::Int(0)),
            ("trace", st("target/ux/round/traces/x.zip")),
            (
                "segments",
                Json::Array(vec![obj(vec![
                    ("kind", st("task")),
                    ("id", st("faq-platforms")),
                    ("completed", Json::Bool(true)),
                    ("first_step", Json::Int(0)),
                    ("steps", Json::Int(steps.len() as i64)),
                ])]),
            ),
            ("steps", Json::Array(steps)),
            ("frames", Json::Array(frames)),
        ])
    }

    fn card(id: &str, mode: &str, theme: &str) -> Json {
        obj(vec![
            ("id", st(id)),
            ("kind", st("anchor")),
            ("mode", st(mode)),
            ("theme", st(theme)),
            ("persona", st("someone")),
            ("tasks", Json::Array(vec![st("faq-platforms")])),
            ("twists", Json::Array(Vec::new())),
        ])
    }

    fn metrics(overflow_1440: i64) -> Json {
        let pages = |overflow: i64| {
            obj(vec![
                ("layout.overflow", Json::Int(overflow)),
                ("layout.truncated", Json::Int(0)),
                ("always_visible.misses", Json::Int(0)),
                ("axe.serious_critical", Json::Int(0)),
                ("reduced_motion.violations", Json::Int(0)),
                ("external_requests", Json::Int(0)),
            ])
        };
        let task = obj(vec![("screen_only.actions", Json::Int(1))]);
        obj(vec![
            (
                "scenarios",
                obj(vec![
                    ("pages/1440x900", pages(overflow_1440)),
                    ("pages/390x844", pages(0)),
                    ("faq-platforms/1440x900", task.clone()),
                    ("faq-platforms/390x844", task),
                ]),
            ),
            ("measured", obj(vec![("pages/1024x768", pages(0))])),
        ])
    }

    fn passed() -> Json {
        obj(vec![
            ("status", st("passed")),
            ("passed", Json::Int(272)),
            ("failed", Json::Int(0)),
            ("flaky", Json::Int(0)),
            ("skipped", Json::Int(0)),
            ("failed_tests", Json::Array(Vec::new())),
        ])
    }

    /// The R-01 shape: a pointer card and a keyboard card at two gating
    /// viewports and one measured one; the FAQ step costs 13 at 1440x900
    /// and 23 at 390x844, other pointer steps 2.
    fn inputs() -> Inputs {
        let faq = |looks| {
            step(
                "click link /^FAQ$/",
                "index.html",
                2,
                looks,
                looks,
                1 + looks,
            )
        };
        let small = |frame| step("click link /^Guide$/", "index.html", frame, 2, 2, frame + 1);
        let home = |tabs| {
            step(
                "enter on link /^Ignite home$/",
                "book/faq.html",
                5,
                tabs,
                tabs + 1,
                6,
            )
        };
        let sessions = vec![
            session("faq", "pointer", "1440x900", true, vec![faq(13), small(20)]),
            session(
                "faq",
                "pointer",
                "390x844",
                true,
                vec![faq(23), small(30), small(32)],
            ),
            session("faq", "pointer", "1024x768", false, vec![faq(50)]),
            session("keys", "keyboard", "1440x900", true, vec![home(57)]),
            session("keys", "keyboard", "390x844", true, vec![home(20)]),
            session("keys", "keyboard", "1024x768", false, vec![home(90)]),
        ];
        Inputs {
            round: "R-01".to_string(),
            metrics: Some(metrics(0)),
            gating: Some(passed()),
            baseline: obj(vec![("scenarios", obj(Vec::new()))]),
            last: None,
            sessions,
            contacts: BTreeMap::new(),
            cards: [
                ("faq".to_string(), card("faq", "pointer", "dark")),
                ("keys".to_string(), card("keys", "keyboard", "light")),
            ]
            .into_iter()
            .collect(),
            tasks: vec!["faq-platforms".to_string()],
            bar_shots: BTreeMap::new(),
        }
    }

    fn render_all(inputs: &Inputs) -> Output {
        render(inputs, &|_| true)
    }

    fn row<'a>(report: &'a str, start: &str) -> &'a str {
        report
            .lines()
            .find(|line| line.starts_with(start))
            .unwrap_or_else(|| panic!("no line starting {start:?} in:\n{report}"))
    }

    fn section<'a>(report: &'a str, heading: &str) -> &'a str {
        let at = report
            .find(heading)
            .unwrap_or_else(|| panic!("no {heading:?}"));
        let rest = &report[at + heading.len()..];
        &rest[..rest.find("\n## ").unwrap_or(rest.len())]
    }

    #[test]
    fn t2_a_keyboard_glance_is_not_applicable() {
        let out = render_all(&inputs());
        let line = row(&out.report, "| `keys-1440x900` |");
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        assert_eq!(&cells[10..12], ["n/a", "n/a"], "{line}");
        let worst = section(&out.report, "## Worst moments");
        for line in worst.lines().filter(|l| l.contains("(keyboard at")) {
            assert!(line.contains("glance n/a"), "{line}");
        }
    }

    #[test]
    fn t3_worst_moments_are_chosen_per_viewport() {
        let out = render_all(&inputs());
        let worst = section(&out.report, "## Worst moments");
        for viewport in ["1440x900", "390x844", "1024x768"] {
            assert!(
                worst.contains(&format!("`faq-{viewport}` (pointer at {viewport}), task faq-platforms: `click link /^FAQ$/`")),
                "the FAQ step at {viewport} is missing:\n{worst}"
            );
            assert!(
                worst.contains(&format!("`keys-{viewport}` (keyboard at {viewport})")),
                "the keyboard step at {viewport} is missing:\n{worst}"
            );
        }
    }

    #[test]
    fn t3_every_missed_bar_has_a_screenshot() {
        let mut inputs = inputs();
        inputs.metrics = Some(metrics(2));
        inputs.bar_shots.insert(
            ("layout.overflow".to_string(), "1440x900".to_string()),
            "target/ux/missed/1440x900/layout.overflow/light-index.html.jpg".to_string(),
        );
        let out = render_all(&inputs);
        let bars = section(&out.report, "## Bars");
        let mut missed = 0;
        for line in bars.lines().filter(|l| l.starts_with("| ")) {
            for cell in line.split('|').filter(|c| c.contains("missed")) {
                missed += 1;
                assert!(
                    cell.contains("](shots/"),
                    "a missed bar without a screenshot: {cell:?} in {line}"
                );
            }
        }
        assert_eq!(missed, 3, "{bars}");
        for (_, to) in &out.copies {
            assert!(
                out.report.contains(to.as_str()) || to.starts_with("shots/worst"),
                "{to}"
            );
        }
    }

    #[test]
    fn t3_the_trend_counts_gating_viewports_only() {
        let out = render_all(&inputs());
        assert_eq!(out.trend.get("worst_pointer_looks"), Some(&Json::Int(23)));
        assert_eq!(out.trend.get("worst_keyboard_tabs"), Some(&Json::Int(57)));
    }

    #[test]
    fn t4_external_requests_without_metrics_are_not_measured() {
        let mut inputs = inputs();
        inputs.metrics = None;
        let out = render_all(&inputs);
        let line = row(&out.report, "| L2: requests off the local server");
        assert!(!line.contains("met"), "{line}");
        assert!(line.contains("not measured"), "{line}");
    }

    #[test]
    fn t5_metrics_without_a_gating_report_are_not_reported() {
        let mut inputs = inputs();
        inputs.gating = None;
        let out = render_all(&inputs);
        let line = row(&out.report, "| L2: no horizontal page scroll");
        assert!(
            !line.contains("met"),
            "metrics of an unknown run reported: {line}"
        );
        assert!(
            out.report.contains("Gating suite: did not report"),
            "{}",
            out.report
        );
    }

    #[test]
    fn t5_a_failed_gating_suite_is_reported() {
        let mut inputs = inputs();
        inputs.gating = Some(obj(vec![
            ("status", st("failed")),
            ("passed", Json::Int(270)),
            ("failed", Json::Int(2)),
            ("flaky", Json::Int(0)),
            ("skipped", Json::Int(0)),
            (
                "failed_tests",
                Json::Array(vec![st("contract.spec.ts › L2 fires on overflow")]),
            ),
        ]));
        let out = render_all(&inputs);
        assert!(
            out.report
                .contains("Gating suite: failed, 2 failed, 270 passed"),
            "{}",
            out.report
        );
        assert!(
            out.report
                .contains("contract.spec.ts › L2 fires on overflow")
        );
        assert_eq!(
            out.round.get("gating_suite").and_then(|g| g.get("status")),
            Some(&st("failed"))
        );
        assert_eq!(out.trend.get("gating_suite"), Some(&st("failed")));
    }

    #[test]
    fn t6_a_keyboard_worst_moment_shows_the_focus_before_enter() {
        let out = render_all(&inputs());
        let worst = section(&out.report, "## Worst moments");
        let line = worst
            .lines()
            .position(|l| l.contains("`keys-1440x900` (keyboard"))
            .expect("keyboard moment");
        let next: Vec<&str> = worst.lines().skip(line + 1).take(2).collect();
        assert!(
            next.iter().any(|l| l.contains("![found, frame 6]")),
            "{next:?}"
        );
        assert!(worst.contains("for the keyboard, the frame with the focus on it, before Enter"));
    }

    #[test]
    fn t7_the_report_names_each_card_s_starting_theme() {
        let out = render_all(&inputs());
        assert!(!out.report.contains("in the light theme"), "{}", out.report);
        let line = row(&out.report, "| `ux/scenarios/cards/faq.json` |");
        assert!(line.contains("| dark |"), "{line}");
    }

    #[test]
    fn the_writer_keeps_the_reviewers_files() {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("round-keeps");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("shots")).unwrap();
        for file in [
            "shots/worst-1-start.jpg",
            "shots/review-menu.png",
            "triage.md",
        ] {
            std::fs::write(dir.join(file), "x").unwrap();
        }
        std::fs::write(
            dir.join("round.json"),
            r#"{"files": ["report.md", "shots/worst-1-start.jpg"]}"#,
        )
        .unwrap();
        clear_written(&dir);
        assert!(!dir.join("shots/worst-1-start.jpg").exists());
        assert!(dir.join("shots/review-menu.png").exists());
        assert!(dir.join("triage.md").exists());
    }
}
