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
            for shot in shots {
                if !named.contains(&shot) {
                    errors.push(format!(
                        "{name}/{shot} is named by neither round.json nor report.md"
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
    out.push(Bar {
        id: "external_requests",
        what: "requests off the local server (L2 and the trial)",
        limit: "0",
        cells: views
            .iter()
            .map(|view| {
                let l2 = metric(
                    metrics,
                    part(view),
                    &format!("pages/{}", view.name),
                    "external_requests",
                );
                let trial: i64 = at(view).iter().map(|s| int(s, "external_requests")).sum();
                zero(Some(l2.unwrap_or(0) + trial))
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

/// A moment of the trial worth a look: a session that stopped, or one of
/// the costliest steps.
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
    frames: Vec<(&'static str, i64)>,
}

fn worst(sessions: &[Json]) -> Vec<Worst> {
    let mut failed = Vec::new();
    let mut pointer = Vec::new();
    let mut keyboard = Vec::new();
    for session in sessions {
        let id = text(session, "session").to_string();
        let mode = text(session, "mode").to_string();
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
            failed.push(Worst {
                session: id.clone(),
                mode: mode.clone(),
                viewport: text(session, "viewport").to_string(),
                gating: text(session, "group") == "gating",
                segment: stopped.map_or(String::new(), |s| text(s, "id").to_string()),
                label: stopped.map_or("stopped".to_string(), |s| text(s, "error").to_string()),
                page: last.map_or(String::new(), |f| text(f, "page").to_string()),
                looks: 0,
                glance: 0,
                actions: 0,
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
            let actions = int(step, "actions");
            let mut shots = vec![("start", frame)];
            if mode == "pointer" && actions > 1 {
                shots.push(("found", frame + actions - 1));
            }
            let moment = Worst {
                session: id.clone(),
                mode: mode.clone(),
                viewport: text(session, "viewport").to_string(),
                gating: text(session, "group") == "gating",
                segment,
                label: text(step, "label").to_string(),
                page: text(step, "page").to_string(),
                looks: int(step, "looks"),
                glance: int(step, "glance"),
                actions,
                frames: shots,
            };
            if mode == "pointer" {
                pointer.push(moment);
            } else {
                keyboard.push(moment);
            }
        }
    }
    let order = |a: &Worst, b: &Worst| {
        b.looks
            .cmp(&a.looks)
            .then(b.glance.cmp(&a.glance))
            .then(b.actions.cmp(&a.actions))
            .then(b.gating.cmp(&a.gating))
            .then(a.session.cmp(&b.session))
            .then(a.frames[0].1.cmp(&b.frames[0].1))
    };
    pointer.sort_by(order);
    keyboard.sort_by(order);
    let distinct = |all: Vec<Worst>, n: usize| -> Vec<Worst> {
        let mut out: Vec<Worst> = Vec::new();
        for moment in all {
            if out.len() == n {
                break;
            }
            if !out
                .iter()
                .any(|o| o.label == moment.label && o.page == moment.page)
            {
                out.push(moment);
            }
        }
        out
    };
    failed.truncate(5);
    failed.extend(distinct(pointer, 3));
    failed.extend(distinct(keyboard, 3));
    failed
}

fn copy(from: &Path, to: &Path, missing: &mut Vec<String>) -> bool {
    match std::fs::copy(from, to) {
        Ok(_) => true,
        Err(e) => {
            missing.push(format!("{}: {e}", from.display()));
            false
        }
    }
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
        "| {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
        field("round"),
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

/// Writes `ux/rounds/$UX_ROUND/` from the output of the gating suite and
/// the trial under `target/ux/`, and rebuilds `ux/rounds/trend.md`. It
/// replaces `report.md`, `round.json` and `shots/` and leaves every other
/// file of the round directory (triage, the review agents' reports).
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
    let shots = dir.join("shots");
    let _ = std::fs::remove_dir_all(&shots);
    std::fs::create_dir_all(&shots).unwrap();

    let mut missing: Vec<String> = Vec::new();
    let metrics_path = root.join("target/ux/metrics.json");
    let metrics = metrics_path.is_file().then(|| parse(&metrics_path));
    if metrics.is_none() {
        missing.push("gating suite: target/ux/metrics.json is missing, so L2, L3 and the ratchet are not measured".to_string());
    }
    let baseline = parse(&ux.join("baseline.json"));
    let previous = round_dirs(&rounds)
        .into_iter()
        .rfind(|(n, name)| *n < number && rounds.join(name).join("round.json").is_file());
    let last = previous
        .as_ref()
        .map(|(_, name)| parse(&rounds.join(name).join("round.json")));
    let last_name = previous.as_ref().map(|(_, name)| name.clone());
    let sessions = read_sessions(&raw);
    assert!(
        !sessions.is_empty(),
        "{} holds no session: run the trial first (task ux-round ROUND={round})",
        raw.join("sessions").display()
    );
    let views = views(&sessions);
    let tasks = task_ids(&ux);
    let bars = bars(metrics.as_ref(), &sessions, &views, &tasks);
    let ratchet = ratchet(metrics.as_ref(), &baseline, last.as_ref());
    let mut files = vec!["report.md".to_string()];

    let worst = worst(&sessions);
    let mut worst_json = Vec::new();
    let mut worst_md = String::new();
    for (k, moment) in worst.iter().enumerate() {
        let rank = k + 1;
        let mut shots_json = Vec::new();
        let mut images = String::new();
        for (what, frame) in &moment.frames {
            let from = raw.join(format!("frames/{}/{frame:03}.jpg", moment.session));
            let name = format!("shots/worst-{rank}-{what}.jpg");
            if copy(&from, &dir.join(&name), &mut missing) {
                files.push(name.clone());
                shots_json.push(obj(vec![
                    ("what", st(*what)),
                    ("frame", Json::Int(*frame)),
                    ("shot", st(name.clone())),
                ]));
                let _ = write!(images, "\n   ![{what}, frame {frame}]({name})");
            }
        }
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
            if moment.mode == "keyboard" {
                " (Tab presses)"
            } else {
                ""
            },
            moment.glance,
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
            ("glance", Json::Int(moment.glance)),
            ("actions", Json::Int(moment.actions)),
            ("shots", Json::Array(shots_json)),
        ]));
    }

    let mut sheets_json = Vec::new();
    let mut sheets_md = String::new();
    for view in &views {
        let contact = raw.join(format!("contact/{}.json", view.name));
        if !contact.is_file() {
            missing.push(format!(
                "contact sheet {}: {} is missing",
                view.name,
                contact.display()
            ));
            continue;
        }
        let contact = parse(&contact);
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
        if copy(
            &root.join(text(&contact, "sheet")),
            &dir.join(&name),
            &mut missing,
        ) {
            files.push(name.clone());
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

    for session in &sessions {
        if session.get("completed").and_then(Json::as_bool) != Some(true) {
            missing.push(format!(
                "session {} stopped after {} of {} tasks",
                text(session, "session"),
                int(session, "tasks_done"),
                int(session, "tasks")
            ));
        }
    }
    let mut inputs = vec![
        format!("questionnaire: {NOT_RUN}"),
        format!("L5 probe: {NOT_RUN}"),
        "L4 keyed replay: not run (no keys)".to_string(),
        "person: none; the screen-only user of `ux/lib/bot.ts` drove every session (SKILL.md section 3.1, a driver bot)".to_string(),
        "free session: not run (optional)".to_string(),
        "rrweb: not in `ux/package-lock.json`; every session is recorded as a Playwright trace instead, `target/ux/round/traces/<session>.zip` (`npx playwright show-trace`), not committed for its size; `task ux-round` records it again".to_string(),
        "review agents: `review-heuristics.md` and `review-report.md` are written into this directory after this command".to_string(),
    ];
    if last.is_none() {
        inputs.push(format!("last round: none, {round} is the first"));
    }
    inputs.extend(missing);

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
        worst
            .iter()
            .filter(|w| w.mode == mode)
            .map(|w| w.looks)
            .max()
    };
    let completed = sessions
        .iter()
        .filter(|s| s.get("completed").and_then(Json::as_bool) == Some(true))
        .count() as i64;
    let trend = obj(vec![
        ("round", st(round.clone())),
        ("sessions", Json::Int(sessions.len() as i64)),
        ("sessions_completed", Json::Int(completed)),
        ("bars_missed", Json::Int(gating_missed)),
        ("ratchet_worse", Json::Int(worse)),
        ("ratchet_better", Json::Int(better)),
        ("trial_pointer_actions", Json::Int(gating_sum("pointer"))),
        ("trial_keyboard_actions", Json::Int(gating_sum("keyboard"))),
        ("worst_pointer_looks", opt(worst_of("pointer"))),
        ("worst_keyboard_tabs", opt(worst_of("keyboard"))),
        ("questionnaire", st(NOT_RUN)),
        ("l5_probe", st(NOT_RUN)),
    ]);

    // report.md
    let mut md = String::new();
    let _ = writeln!(md, "# UX round {round}\n");
    let _ = writeln!(
        md,
        "Written by `task ux-round ROUND={round}` (`site::round::write`) from the gating suite (`task ux`'s Playwright run, `target/ux/metrics.json`) and the trial (`ux/round/sessions.spec.ts`: {} sessions, the cards below at every viewport). Do not edit; run the command again.\n",
        sessions.len()
    );
    let _ = writeln!(
        md,
        "- Last round: {}.\n- Questionnaire: {NOT_RUN}.\n- L5 probe: {NOT_RUN}.\n- Recording: Playwright traces (rrweb is not in the lockfile).\n- Gating viewports: {}. Measured only, reported and not gated: {}.\n- Frames: each taken 0.75 s after its moment (the start and every click, wheel turn, opened menu, typed text or Enter), at `prefers-reduced-motion: reduce`.\n",
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
    let _ = writeln!(
        md,
        "## Bars\n\nSKILL.md section 4. A gating cell is `met` or `missed`; a measured-only cell is `reported`; `not measured` where nothing ran.\n"
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
            match c {
                None => {
                    let _ = write!(md, " not measured |");
                }
                Some((_, _, t)) => {
                    let _ = write!(md, " {t} {verdict} |");
                }
            }
            cells_json.push(obj(vec![
                ("viewport", st(view.name.clone())),
                ("gating", Json::Bool(view.gating)),
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
        .as_ref()
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
        "\n## Trial\n\nThree cards (SKILL.md section 3.1): two anchors, kept fixed for later rounds, and the rotating card of {round}, under `ux/scenarios/cards/`. Each runs as one session per viewport, from the landing page in the light theme.\n\n| card | kind | mode | persona | tasks, in order | twists |\n|---|---|---|---|---|---|"
    );
    let mut card_ids: Vec<String> = sessions
        .iter()
        .map(|s| text(s, "card").to_string())
        .collect();
    card_ids.dedup();
    let mut cards_json = Vec::new();
    for id in &card_ids {
        let card = parse(&ux.join(format!("scenarios/cards/{id}.json")));
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
            "| `ux/scenarios/cards/{id}.json` | {} | {} | {} | {} | {} |",
            text(&card, "kind"),
            text(&card, "mode"),
            cell(text(&card, "persona")),
            tasks.join(", "),
            cell(&twists.join("; ")),
        );
        cards_json.push(obj(vec![
            ("id", st(id.clone())),
            ("kind", st(text(&card, "kind"))),
            ("mode", st(text(&card, "mode"))),
            ("file", st(format!("ux/scenarios/cards/{id}.json"))),
        ]));
    }
    let _ = writeln!(
        md,
        "\nLooks count each inspection of the screen; for the keyboard they are Tab presses. Glance cost: 1 a control in the primary region, 2 elsewhere in view, 3 only after a scroll or an opened menu.\n\n| session | viewport | completed | tasks | actions | last round | looks | steers | time to act, max | glance median | glance p95 | step actions, max |\n|---|---|---|---|---|---|---|---|---|---|---|---|"
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
            .as_ref()
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
            int(session, "glance_median"),
            int(session, "glance_p95"),
            int(session, "step_actions_max"),
        );
        let mut entry = vec![("session", st(id)), ("completed", Json::Bool(done))];
        for key in ["card", "kind", "mode", "viewport", "group", "trace"] {
            entry.push((key, st(text(session, key))));
        }
        for key in [
            "tasks_done",
            "tasks",
            "actions",
            "looks",
            "steers",
            "time_to_act_max",
            "glance_median",
            "glance_p95",
            "step_actions_max",
            "external_requests",
        ] {
            entry.push((key, Json::Int(int(session, key))));
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
        "\n## Worst moments\n\nSessions that stopped, then the three costliest pointer steps and the three costliest keyboard steps (most looks, then glance, then actions; one per control and page). `start` is the screen the user faced when it began to look for the control, `found` the screen on which it found it.\n"
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
    for input in &inputs {
        let _ = writeln!(md, "- {input}");
    }
    std::fs::write(dir.join("report.md"), &md).unwrap();

    let round_json = obj(vec![
        ("round", st(round.clone())),
        ("command", st(format!("task ux-round ROUND={round}"))),
        ("last_round", last_name.clone().map_or(Json::Null, st)),
        ("questionnaire", st(NOT_RUN)),
        ("l5_probe", st(NOT_RUN)),
        ("recording", st("playwright-trace")),
        ("frame_delay_ms", Json::Int(750)),
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
            Json::Array(inputs.iter().map(|i| st(i.clone())).collect()),
        ),
        (
            "files",
            Json::Array(files.iter().map(|f| st(f.clone())).collect()),
        ),
        ("trend", trend.clone()),
    ]);
    let mut out = String::new();
    round_json.write(&mut out, 0);
    out.push('\n');
    std::fs::write(dir.join("round.json"), out).unwrap();

    let mut trend_md = String::from(
        "# UX rounds\n\nOne row per round, rebuilt by `task ux-round` from each `R-nn/round.json`. Bars missed count gating cells only; trial actions sum the sessions at the gating viewports.\n\n| round | sessions completed | bars missed | ratchet worse | ratchet better | trial pointer actions | trial keyboard actions | worst pointer looks | worst keyboard tabs | questionnaire | L5 probe |\n|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (_, name) in round_dirs(&rounds) {
        let row = if name == round {
            trend.clone()
        } else {
            match std::fs::read_to_string(rounds.join(&name).join("round.json"))
                .ok()
                .and_then(|t| Json::parse(&t).ok())
                .and_then(|j| j.get("trend").cloned())
            {
                Some(row) => row,
                None => continue,
            }
        };
        trend_md.push_str(&trend_row(&row));
        trend_md.push('\n');
    }
    std::fs::write(rounds.join("trend.md"), trend_md).unwrap();

    let errors = shape_errors(&rounds);
    assert!(
        errors.is_empty(),
        "the round just written:\n{}",
        errors.join("\n")
    );
}
