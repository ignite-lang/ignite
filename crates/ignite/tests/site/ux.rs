//! The website's UI loop (`ux/`, story `docs-site-ui-loop`): the ratchet of
//! the measured values against `ux/baseline.json` and the rule that a
//! closed S1 or S2 finding names its regression asset. The browser side is
//! Playwright under `ux/`; `task ux` runs it, which writes
//! `target/ux/metrics.json`, then runs `site::ux::ratchet` (ignored here, so a
//! plain `cargo test` needs no browser).
//!
//! Every value is lower-is-better. The ratchet refuses a value worse than
//! the baseline beyond its tolerance, a value the baseline holds and the
//! run did not measure, a measured value the baseline does not hold, and an
//! improvement not written into the baseline; and, against `main`'s
//! baseline, a baseline entry made worse without a `decision`.
//! `IGNITE_BLESS=1` writes the improvements and the new values into the
//! baseline; it never writes a worse one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_eval::Json;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A tolerance: an absolute count, or a percentage of the baseline value.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tolerance {
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

    /// Whether `value` is worse than `base` beyond this tolerance.
    fn exceeded(self, base: i64, value: i64) -> bool {
        match self {
            Tolerance::Count(n) => value > base + n,
            Tolerance::Percent(p) => value * 100 > base * (100 + p),
        }
    }

    /// Whether this tolerance admits more than `other`; a change of kind
    /// (count to percentage or back) counts as looser.
    fn looser_than(self, other: Tolerance) -> bool {
        match (self, other) {
            (Tolerance::Count(a), Tolerance::Count(b)) => a > b,
            (Tolerance::Percent(a), Tolerance::Percent(b)) => a > b,
            _ => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    value: i64,
    tolerance: Tolerance,
    decision: Option<String>,
}

type Values = BTreeMap<String, BTreeMap<String, i64>>;
type Baseline = BTreeMap<String, BTreeMap<String, Entry>>;

fn object<'a>(value: &'a Json, what: &str) -> Result<&'a BTreeMap<String, Json>, String> {
    match value {
        Json::Object(entries) => Ok(entries),
        other => Err(format!("{what} is not an object: {}", other.compact())),
    }
}

fn read_baseline(json: &Json) -> Result<Baseline, String> {
    let root = object(json, "the baseline")?;
    for key in ["about", "scenarios", "tolerances"] {
        if !root.contains_key(key) {
            return Err(format!("the baseline has no `{key}`"));
        }
    }
    let mut out = Baseline::new();
    for (scenario, metrics) in object(&root["scenarios"], "`scenarios`")? {
        let mut entries = BTreeMap::new();
        for (key, entry) in object(metrics, scenario)? {
            let fields = object(entry, &format!("{scenario} {key}"))?;
            let value = fields
                .get("value")
                .and_then(Json::as_i64)
                .ok_or_else(|| format!("{scenario} {key}: no integer `value`"))?;
            let tolerance = Tolerance::parse(
                fields
                    .get("tolerance")
                    .ok_or_else(|| format!("{scenario} {key}: no `tolerance`"))?,
            )
            .map_err(|e| format!("{scenario} {key}: {e}"))?;
            let decision = fields
                .get("decision")
                .and_then(Json::as_str)
                .map(str::to_string);
            entries.insert(
                key.clone(),
                Entry {
                    value,
                    tolerance,
                    decision,
                },
            );
        }
        out.insert(scenario.clone(), entries);
    }
    Ok(out)
}

fn read_values(json: &Json) -> Result<Values, String> {
    let root = object(json, "the metrics")?;
    let scenarios = root
        .get("scenarios")
        .ok_or("the metrics have no `scenarios`")?;
    let mut out = Values::new();
    for (scenario, metrics) in object(scenarios, "`scenarios`")? {
        let mut values = BTreeMap::new();
        for (key, value) in object(metrics, scenario)? {
            let value = value
                .as_i64()
                .ok_or_else(|| format!("{scenario} {key}: not an integer"))?;
            values.insert(key.clone(), value);
        }
        out.insert(scenario.clone(), values);
    }
    Ok(out)
}

/// The tolerance a key gets when it first enters the baseline: steps 5 %,
/// every other key a count (SKILL.md section 6).
fn default_tolerance(key: &str) -> Tolerance {
    if key.contains("steps") {
        Tolerance::Percent(5)
    } else {
        Tolerance::Count(0)
    }
}

/// Every reason the measured `values` do not hold against `baseline`, and
/// `baseline` against `main`'s baseline when there is one.
fn check(baseline: &Baseline, values: &Values, main: Option<&Baseline>) -> Vec<String> {
    let mut errors = Vec::new();
    for (scenario, entries) in baseline {
        for (key, entry) in entries {
            match values.get(scenario).and_then(|v| v.get(key)) {
                None => errors.push(format!("{scenario} {key}: missing (baseline {})", entry.value)),
                Some(&value) if entry.tolerance.exceeded(entry.value, value) => errors.push(format!(
                    "{scenario} {key}: worse, {value} against the baseline {} ({:?})",
                    entry.value, entry.tolerance
                )),
                Some(&value) if value < entry.value => errors.push(format!(
                    "{scenario} {key}: improved to {value} from {}; write it into ux/baseline.json (IGNITE_BLESS=1)",
                    entry.value
                )),
                Some(_) => {}
            }
        }
    }
    for (scenario, measured) in values {
        for (key, value) in measured {
            if !baseline.get(scenario).is_some_and(|e| e.contains_key(key)) {
                errors.push(format!(
                    "{scenario} {key}: {value} is not in ux/baseline.json (IGNITE_BLESS=1)"
                ));
            }
        }
    }
    if let Some(main) = main {
        for (scenario, entries) in baseline {
            for (key, entry) in entries {
                let Some(old) = main.get(scenario).and_then(|e| e.get(key)) else {
                    continue;
                };
                let decided = entry
                    .decision
                    .as_deref()
                    .is_some_and(|d| !d.trim().is_empty());
                if old.tolerance.exceeded(old.value, entry.value) && !decided {
                    errors.push(format!(
                        "{scenario} {key}: the baseline moved from {} on main to {} without a `decision`",
                        old.value, entry.value
                    ));
                }
                if entry.tolerance.looser_than(old.tolerance) && !decided {
                    errors.push(format!(
                        "{scenario} {key}: the tolerance loosened from {:?} on main to {:?} without a `decision`",
                        old.tolerance, entry.tolerance
                    ));
                }
            }
        }
    }
    errors
}

/// `baseline` with every improved and every new value taken from `values`;
/// a worse value is left as it is.
fn bless(baseline: &Baseline, values: &Values) -> Baseline {
    let mut out = baseline.clone();
    for (scenario, measured) in values {
        let entries = out.entry(scenario.clone()).or_default();
        for (key, value) in measured {
            match entries.get_mut(key) {
                Some(entry) if *value < entry.value => entry.value = *value,
                Some(_) => {}
                None => {
                    entries.insert(
                        key.clone(),
                        Entry {
                            value: *value,
                            tolerance: default_tolerance(key),
                            decision: None,
                        },
                    );
                }
            }
        }
    }
    out
}

fn write_baseline(original: &Json, baseline: &Baseline) -> String {
    let mut root = object(original, "the baseline").unwrap().clone();
    let scenarios = baseline
        .iter()
        .map(|(scenario, entries)| {
            let entries = entries
                .iter()
                .map(|(key, entry)| {
                    let mut fields = BTreeMap::new();
                    fields.insert("value".to_string(), Json::Int(entry.value));
                    fields.insert(
                        "tolerance".to_string(),
                        match entry.tolerance {
                            Tolerance::Count(n) => Json::Int(n),
                            Tolerance::Percent(p) => Json::Str(format!("{p}%")),
                        },
                    );
                    if let Some(decision) = &entry.decision {
                        fields.insert("decision".to_string(), Json::Str(decision.clone()));
                    }
                    (key.clone(), Json::Object(fields))
                })
                .collect();
            (scenario.clone(), Json::Object(entries))
        })
        .collect();
    root.insert("scenarios".to_string(), Json::Object(scenarios));
    let mut text = String::new();
    Json::Object(root).write(&mut text, 0);
    text.push('\n');
    text
}

fn parse(text: &str, what: &str) -> Json {
    Json::parse(text).unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// `main`'s `ux/baseline.json`, when `main` has one.
fn main_baseline() -> Option<Baseline> {
    let output = Command::new("git")
        .args(["show", "main:ux/baseline.json"])
        .current_dir(workspace())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(read_baseline(&parse(&text, "main:ux/baseline.json")).expect("main:ux/baseline.json"))
}

#[test]
fn the_baseline_is_well_formed() {
    let path = workspace().join("ux/baseline.json");
    let text = std::fs::read_to_string(&path).expect("ux/baseline.json");
    let json = parse(&text, "ux/baseline.json");
    let baseline = read_baseline(&json).unwrap();
    assert!(!baseline.is_empty(), "ux/baseline.json holds no scenario");
    assert_eq!(
        write_baseline(&json, &baseline),
        text,
        "ux/baseline.json is not in its canonical form"
    );
}

/// The ratchet over the run `task ux` just made. Ignored by a plain
/// `cargo test`: it needs `target/ux/metrics.json`, which only the browser
/// run writes; `task ux` runs it by name.
#[test]
#[ignore]
fn ratchet() {
    let root = workspace();
    let metrics_path = root.join("target/ux/metrics.json");
    let metrics = std::fs::read_to_string(&metrics_path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run `task ux`, which writes it",
            metrics_path.display()
        )
    });
    let values = read_values(&parse(&metrics, "target/ux/metrics.json")).unwrap();
    let baseline_path = root.join("ux/baseline.json");
    let text = std::fs::read_to_string(&baseline_path).expect("ux/baseline.json");
    let json = parse(&text, "ux/baseline.json");
    let mut baseline = read_baseline(&json).unwrap();
    if std::env::var_os("IGNITE_BLESS").is_some() {
        baseline = bless(&baseline, &values);
        std::fs::write(&baseline_path, write_baseline(&json, &baseline)).unwrap();
    }
    let errors = check(&baseline, &values, main_baseline().as_ref());
    assert!(
        errors.is_empty(),
        "the UI ratchet refuses this run:\n  {}",
        errors.join("\n  ")
    );
}

fn entry(value: i64, tolerance: Tolerance) -> Entry {
    Entry {
        value,
        tolerance,
        decision: None,
    }
}

fn one(scenario: &str, key: &str, value: i64) -> Values {
    BTreeMap::from([(
        scenario.to_string(),
        BTreeMap::from([(key.to_string(), value)]),
    )])
}

fn base(scenario: &str, key: &str, entry: Entry) -> Baseline {
    BTreeMap::from([(
        scenario.to_string(),
        BTreeMap::from([(key.to_string(), entry)]),
    )])
}

/// The break-tests of the ratchet: each refusal fires on its own case, an
/// equal value and a step value inside 5 % pass.
#[test]
fn the_ratchet_refuses_worse_missing_new_and_untaken_values() {
    let count = base(
        "t/1440x900",
        "screen_only.actions",
        entry(4, Tolerance::Count(0)),
    );
    assert_eq!(
        check(&count, &one("t/1440x900", "screen_only.actions", 4), None),
        Vec::<String>::new()
    );
    let worse = check(&count, &one("t/1440x900", "screen_only.actions", 5), None);
    assert!(worse.len() == 1 && worse[0].contains("worse"), "{worse:?}");
    let better = check(&count, &one("t/1440x900", "screen_only.actions", 3), None);
    assert!(
        better.len() == 1 && better[0].contains("improved"),
        "{better:?}"
    );
    let missing = check(&count, &Values::new(), None);
    assert!(
        missing.len() == 1 && missing[0].contains("missing"),
        "{missing:?}"
    );
    let new = check(
        &Baseline::new(),
        &one("t/390x844", "keyboard.actions", 9),
        None,
    );
    assert!(
        new.len() == 1 && new[0].contains("not in ux/baseline.json"),
        "{new:?}"
    );

    let steps = base(
        "t/1440x900",
        "screen_only.time_to_act_steps.max",
        entry(20, Tolerance::Percent(5)),
    );
    let key = "screen_only.time_to_act_steps.max";
    assert!(check(&steps, &one("t/1440x900", key, 21), None).is_empty());
    assert_eq!(check(&steps, &one("t/1440x900", key, 22), None).len(), 1);

    let blessed = bless(&count, &one("t/1440x900", "screen_only.actions", 3));
    assert_eq!(blessed["t/1440x900"]["screen_only.actions"].value, 3);
    let not_worse = bless(&count, &one("t/1440x900", "screen_only.actions", 5));
    assert_eq!(not_worse["t/1440x900"]["screen_only.actions"].value, 4);
    let added = bless(&Baseline::new(), &one("t/1440x900", key, 7));
    assert_eq!(
        added["t/1440x900"][key].tolerance,
        Tolerance::Percent(5),
        "a step key enters with 5 %"
    );
}

#[test]
fn a_baseline_made_worse_than_mains_needs_a_decision() {
    let main = base(
        "pages/390x844",
        "axe.serious_critical",
        entry(0, Tolerance::Count(0)),
    );
    let raised = base(
        "pages/390x844",
        "axe.serious_critical",
        entry(2, Tolerance::Count(0)),
    );
    let values = one("pages/390x844", "axe.serious_critical", 2);
    let errors = check(&raised, &values, Some(&main));
    assert!(
        errors.len() == 1 && errors[0].contains("without a `decision`"),
        "{errors:?}"
    );
    let mut decided = raised.clone();
    decided
        .get_mut("pages/390x844")
        .unwrap()
        .get_mut("axe.serious_critical")
        .unwrap()
        .decision = Some("R-01-D1".to_string());
    assert!(check(&decided, &values, Some(&main)).is_empty());

    let loose = base(
        "t/1440x900",
        "keyboard.actions",
        entry(4, Tolerance::Count(1)),
    );
    let strict = base(
        "t/1440x900",
        "keyboard.actions",
        entry(4, Tolerance::Count(0)),
    );
    let percent = base(
        "t/1440x900",
        "keyboard.actions",
        entry(4, Tolerance::Percent(5)),
    );
    let four = one("t/1440x900", "keyboard.actions", 4);
    assert!(check(&strict, &four, Some(&loose)).is_empty(), "tightened");
    assert_eq!(
        check(&percent, &four, Some(&strict)).len(),
        1,
        "kind changed"
    );
}

const FINDING_FIELDS: [&str; 11] = [
    "id",
    "title",
    "class",
    "severity",
    "metric",
    "status",
    "found_by",
    "evidence_recording",
    "evidence_screenshot",
    "evidence_lines",
    "regression_asset",
];

/// The header of a finding record: the `key: value` lines between the
/// leading `---` fences.
fn finding_header(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err("does not start with a `---` header".to_string());
    }
    let mut header = BTreeMap::new();
    for line in lines {
        if line == "---" {
            return Ok(header);
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("header line {line:?} is not `key: value`"))?;
        header.insert(key.trim().to_string(), value.trim().to_string());
    }
    Err("the header has no closing `---`".to_string())
}

/// Every reason the finding `name` (its file name) breaks the rules of
/// SKILL.md section 5; `exists` says whether a workspace path exists.
fn finding_errors(name: &str, text: &str, exists: &dyn Fn(&str) -> bool) -> Vec<String> {
    let header = match finding_header(text) {
        Ok(header) => header,
        Err(e) => return vec![format!("{name}: {e}")],
    };
    let mut errors = Vec::new();
    for field in FINDING_FIELDS {
        if !header.contains_key(field) {
            errors.push(format!("{name}: no `{field}`"));
        }
    }
    let field = |key: &str| header.get(key).map(String::as_str).unwrap_or("");
    if Some(field("id")) != name.strip_suffix(".md") {
        errors.push(format!("{name}: id {:?} is not the file name", field("id")));
    }
    let classes = [
        "layout",
        "cue",
        "affordance",
        "copy",
        "noise",
        "engine",
        "input",
    ];
    if !classes.contains(&field("class")) {
        errors.push(format!(
            "{name}: class {:?} is not one of {classes:?}",
            field("class")
        ));
    }
    if !["S1", "S2", "S3"].contains(&field("severity")) {
        errors.push(format!(
            "{name}: severity {:?} is not S1, S2 or S3",
            field("severity")
        ));
    }
    if !["open", "closed"].contains(&field("status")) {
        errors.push(format!(
            "{name}: status {:?} is not open or closed",
            field("status")
        ));
    }
    if field("status") == "closed" && ["S1", "S2"].contains(&field("severity")) {
        let asset = field("regression_asset");
        let (path, test) = match asset.split_once(" :: ") {
            Some((path, test)) => (path.trim(), test.trim()),
            None => (asset.trim(), ""),
        };
        if path.is_empty() || path == "none" {
            errors.push(format!(
                "{name}: closed {} finding without a regression_asset",
                field("severity")
            ));
        } else if path.starts_with('/') || path.split('/').any(|part| part == "..") {
            errors.push(format!(
                "{name}: regression_asset {path} is not a plain workspace path"
            ));
        } else if !(path.starts_with("ux/") || path.starts_with("crates/")) {
            errors.push(format!(
                "{name}: regression_asset {path} is not a test under ux/ or crates/"
            ));
        } else if !exists(path) {
            errors.push(format!("{name}: regression_asset {path} does not exist"));
        } else if test.is_empty() {
            errors.push(format!(
                "{name}: regression_asset names no test: write `{path} :: <test name>`"
            ));
        } else if !is_test_file(path) {
            errors.push(format!(
                "{name}: regression_asset {path} is no test file (ux/tests/*.spec.ts or crates/*/tests/**/*.rs)"
            ));
        } else {
            let text = std::fs::read_to_string(workspace().join(path)).unwrap_or_default();
            if !declares_test(path, &text, test) {
                errors.push(format!(
                    "{name}: regression_asset names {test:?}, which is no test declared in {path}"
                ));
            }
        }
    }
    errors
}

/// A file that holds tests: a Playwright spec under `ux/tests/`, or a Rust
/// file under a crate's `tests/`.
fn is_test_file(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["ux", "tests", .., file] => file.ends_with(".spec.ts"),
        ["crates", _, "tests", .., file] => file.ends_with(".rs"),
        _ => false,
    }
}

/// Whether `text`, the file at `path`, declares the test `test` by its whole
/// name: `test("<name>"` (or with `'` or a backquote) in a spec, or a
/// `fn <name>(` whose attributes hold `#[test]` in a Rust file.
fn declares_test(path: &str, text: &str, test: &str) -> bool {
    if path.ends_with(".spec.ts") {
        return ['"', '\'', '`']
            .iter()
            .any(|quote| text.contains(&format!("test({quote}{test}{quote}")));
    }
    let name = test.strip_prefix("fn ").unwrap_or(test);
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return false;
    }
    let lines: Vec<&str> = text.lines().collect();
    lines.iter().enumerate().any(|(index, line)| {
        if !line.trim_start().starts_with(&format!("fn {name}(")) {
            return false;
        }
        lines[..index]
            .iter()
            .rev()
            .map(|above| above.trim())
            .take_while(|above| above.starts_with("#[") || above.starts_with("///"))
            .any(|above| above == "#[test]")
    })
}

#[test]
fn every_closed_s1_and_s2_finding_has_a_regression_asset() {
    let root = workspace();
    let dir = root.join("ux/findings");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("ux/findings")
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("UXF-") && name.ends_with(".md"))
        .collect();
    names.sort();
    let exists = |path: &str| root.join(path).exists();
    let errors: Vec<String> = names
        .iter()
        .flat_map(|name| {
            let text = std::fs::read_to_string(dir.join(name)).unwrap();
            finding_errors(name, &text, &exists)
        })
        .collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn a_closed_s2_finding_without_an_asset_is_refused() {
    let record = |severity: &str, status: &str, asset: &str| {
        format!(
            "---\nid: UXF-900\ntitle: t\nclass: layout\nseverity: {severity}\nmetric: m\nstatus: {status}\nfound_by: L2\nevidence_recording: none\nevidence_screenshot: none\nevidence_lines: none\nregression_asset: {asset}\n---\n\nbody\n"
        )
    };
    let exists = |path: &str| path == "ux/tests/regressions.spec.ts";
    let closed = finding_errors("UXF-900.md", &record("S2", "closed", "none"), &exists);
    assert!(
        closed.len() == 1 && closed[0].contains("without a regression_asset"),
        "{closed:?}"
    );
    let s1 = finding_errors("UXF-900.md", &record("S1", "closed", ""), &exists);
    assert_eq!(s1.len(), 1, "{s1:?}");
    let gone = finding_errors(
        "UXF-900.md",
        &record("S2", "closed", "ux/tests/gone.spec.ts :: x"),
        &exists,
    );
    assert!(
        gone.len() == 1 && gone[0].contains("does not exist"),
        "{gone:?}"
    );
    assert!(
        finding_errors(
            "UXF-900.md",
            &record(
                "S2",
                "closed",
                "ux/tests/regressions.spec.ts :: UXF-001: scrolling code blocks and tables take focus",
            ),
            &exists
        )
        .is_empty()
    );
    for (asset, reason) in [
        ("ux/tests/layout.spec.ts", "names no test"),
        (
            "ux/tests/layout.spec.ts :: no such test",
            "no test declared in ux/tests/layout.spec.ts",
        ),
        ("/etc/passwd :: root", "not a plain workspace path"),
        ("ux/../README.md :: Ignite", "not a plain workspace path"),
        ("README.md :: Ignite", "not a test under ux/ or crates/"),
        (
            "ux/tests/regressions.spec.ts :: UXF-001",
            "no test declared in",
        ),
        ("ux/lib/layout.ts :: overflow", "is no test file"),
        (
            "crates/ignite/tests/site/ux.rs :: fn finding_errors",
            "no test declared in",
        ),
    ] {
        let errors = finding_errors("UXF-900.md", &record("S2", "closed", asset), &|_| true);
        assert!(
            errors.len() == 1 && errors[0].contains(reason),
            "{asset}: {errors:?}"
        );
    }
    let rust = record(
        "S2",
        "closed",
        "crates/ignite/tests/site/ux.rs :: fn every_closed_s1_and_s2_finding_has_a_regression_asset",
    );
    assert_eq!(
        finding_errors("UXF-900.md", &rust, &|_| true),
        Vec::<String>::new(),
        "a #[test] fn names a test"
    );
    assert!(finding_errors("UXF-900.md", &record("S2", "open", "none"), &exists).is_empty());
    assert!(finding_errors("UXF-900.md", &record("S3", "closed", "none"), &exists).is_empty());
    let unnamed = finding_errors("UXF-901.md", &record("S3", "open", "none"), &exists);
    assert!(
        unnamed.len() == 1 && unnamed[0].contains("not the file name"),
        "{unnamed:?}"
    );
}

/// Adversary: a branch that raises an entry's `tolerance` instead of its
/// `value` lets a worse run pass, with no `decision`, against `main`'s
/// baseline.
#[test]
fn adversary_a_tolerance_raised_against_mains_needs_a_decision() {
    let main = base(
        "pages/1440x900",
        "axe.serious_critical",
        entry(0, Tolerance::Count(0)),
    );
    let loosened = base(
        "pages/1440x900",
        "axe.serious_critical",
        entry(0, Tolerance::Count(50)),
    );
    let values = one("pages/1440x900", "axe.serious_critical", 7);
    let errors = check(&loosened, &values, Some(&main));
    assert!(
        !errors.is_empty(),
        "7 serious axe violations pass against main's 0 by a raised tolerance and no decision"
    );
}

/// Adversary: the regression asset of a closed S2 finding must name
/// something that exists in the file it points at, not only the file.
#[test]
fn adversary_a_regression_asset_naming_no_test_is_refused() {
    let record = "---\nid: UXF-900\ntitle: t\nclass: layout\nseverity: S2\nmetric: m\nstatus: closed\nfound_by: L2\nevidence_recording: none\nevidence_screenshot: none\nevidence_lines: none\nregression_asset: README.md :: a test that is nowhere\n---\n\nbody\n";
    let exists = |path: &str| workspace().join(path).exists();
    let errors = finding_errors("UXF-900.md", record, &exists);
    assert!(
        !errors.is_empty(),
        "a closed S2 finding whose asset is README.md passes the findings check"
    );
}

fn adversary2_record(asset: &str) -> String {
    format!(
        "---\nid: UXF-900\ntitle: t\nclass: layout\nseverity: S2\nmetric: m\nstatus: closed\nfound_by: L2\nevidence_recording: none\nevidence_screenshot: none\nevidence_lines: none\nregression_asset: {asset}\n---\n\nbody\n"
    )
}

/// Adversary, pass 2: the named test must be a test, not words that occur
/// in a comment of the file.
#[test]
fn adversary2_a_test_name_found_only_in_a_comment_is_refused() {
    let exists = |path: &str| workspace().join(path).exists();
    let asset = "ux/tests/regressions.spec.ts :: test failed on the site before the finding's fix";
    let errors = finding_errors("UXF-900.md", &adversary2_record(asset), &exists);
    assert!(
        !errors.is_empty(),
        "a regression asset naming words of a comment, not a test, passes the findings check"
    );
}

/// Adversary, pass 2: a fragment of several test titles names no one test.
#[test]
fn adversary2_a_fragment_of_test_titles_is_refused() {
    let exists = |path: &str| workspace().join(path).exists();
    let errors = finding_errors(
        "UXF-900.md",
        &adversary2_record("ux/tests/regressions.spec.ts :: UXF"),
        &exists,
    );
    assert!(
        !errors.is_empty(),
        "`UXF`, a fragment of seven test titles, passes as the name of a regression test"
    );
}

/// Adversary, pass 2: a source file under crates/ that holds no test is
/// no regression asset.
#[test]
fn adversary2_a_source_file_that_is_no_test_is_refused() {
    let exists = |path: &str| workspace().join(path).exists();
    let errors = finding_errors(
        "UXF-900.md",
        &adversary2_record("crates/ignite/src/main.rs :: fn main"),
        &exists,
    );
    assert!(
        !errors.is_empty(),
        "crates/ignite/src/main.rs :: fn main passes as a regression asset"
    );
}
