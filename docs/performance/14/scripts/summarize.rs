//! Summaries of the slice-14 gate-time logs (`docs/slice.md` 18.18).
//!
//! `pairs FILE…`: reads the `STEP tree=… label=… pair=… secs=… load_before=… load_after=… exit=…`
//! lines of the summary files, pairs each label's `before` and `after` run of one pass, drops a
//! pair whose start loads differ by more than 5 or whose exit is not 0, and prints one Markdown
//! row per label: the medians of the kept absolute times, the median of the kept pair ratios
//! (after / before) and the kept/discarded counts, then every pair.
//!
//! `counts LOG`: sums cargo's `test result:` lines (passed, failed, ignored), prints the
//! `crates/ignite` end-to-end binary's line (the block that runs `cli::` tests) and the
//! `ignite test` summary lines of the inline tests.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::process::ExitCode;

#[derive(Clone, Copy)]
struct Run {
    secs: f64,
    load: f64,
    exit: i64,
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|w| w.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

fn fmt(v: Option<f64>, digits: usize) -> String {
    v.map_or("-".to_string(), |x| format!("{x:.digits$}"))
}

fn pairs(files: &[String]) -> ExitCode {
    let mut order: Vec<String> = Vec::new();
    let mut runs: BTreeMap<String, BTreeMap<String, BTreeMap<String, Run>>> = BTreeMap::new();
    for file in files {
        let Ok(text) = fs::read_to_string(file) else {
            eprintln!("cannot read {file}");
            return ExitCode::from(2);
        };
        for line in text.lines().filter(|l| l.starts_with("STEP ")) {
            let (Some(tree), Some(label), Some(pair), Some(secs), Some(load), Some(exit)) = (
                field(line, "tree"),
                field(line, "label"),
                field(line, "pair"),
                field(line, "secs"),
                field(line, "load_before"),
                field(line, "exit"),
            ) else {
                eprintln!("malformed: {line}");
                return ExitCode::from(2);
            };
            let run = Run {
                secs: secs.parse().unwrap(),
                load: load.parse().unwrap(),
                exit: exit.parse().unwrap(),
            };
            if !runs.contains_key(label) {
                order.push(label.to_string());
            }
            runs.entry(label.to_string())
                .or_default()
                .entry(pair.to_string())
                .or_default()
                .insert(tree.to_string(), run);
        }
    }
    println!("| unit | before s | after s | ratio | kept/discarded | pairs (before/after s, loads) |");
    println!("|---|---|---|---|---|---|");
    for label in &order {
        let mut kept_b = Vec::new();
        let mut kept_a = Vec::new();
        let mut ratios = Vec::new();
        let mut discarded = 0;
        let mut detail = Vec::new();
        for (pair, trees) in &runs[label] {
            match (trees.get("before"), trees.get("after")) {
                (Some(b), Some(a)) => {
                    let ok = (b.load - a.load).abs() <= 5.0 && b.exit == 0 && a.exit == 0;
                    let mark = if ok { "" } else { " dropped" };
                    detail.push(format!(
                        "p{pair} {:.3}/{:.3} ({:.2}, {:.2}; exit {}, {}){mark}",
                        b.secs, a.secs, b.load, a.load, b.exit, a.exit
                    ));
                    if ok {
                        kept_b.push(b.secs);
                        kept_a.push(a.secs);
                        ratios.push(a.secs / b.secs);
                    } else {
                        discarded += 1;
                    }
                }
                (b, a) => {
                    for (tree, r) in [("before", b), ("after", a)] {
                        if let Some(r) = r {
                            detail.push(format!(
                                "p{pair} {tree} only {:.3} (load {:.2}, exit {})",
                                r.secs, r.load, r.exit
                            ));
                        }
                    }
                }
            }
        }
        println!(
            "| {label} | {} | {} | {} | {}/{discarded} | {} |",
            fmt(median(kept_b), 3),
            fmt(median(kept_a), 3),
            fmt(median(ratios.clone()), 4),
            ratios.len(),
            detail.join("; ")
        );
    }
    ExitCode::SUCCESS
}

fn counts(log: &str) -> ExitCode {
    let Ok(text) = fs::read_to_string(log) else {
        eprintln!("cannot read {log}");
        return ExitCode::from(2);
    };
    let (mut passed, mut failed, mut ignored, mut filtered) = (0u64, 0u64, 0u64, 0u64);
    let mut in_e2e = false;
    let mut inline = 0;
    let num = |line: &str, word: &str| -> u64 {
        let words: Vec<&str> = line.split_whitespace().collect();
        words
            .windows(2)
            .find(|w| w[1].trim_end_matches([';', ',']) == word)
            .and_then(|w| w[0].parse().ok())
            .unwrap_or(0)
    };
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("Running ") {
            in_e2e = false;
        }
        if t.starts_with("test cli::") {
            in_e2e = true;
        }
        if let Some(rest) = t.strip_prefix("test result:") {
            if !rest.contains(" measured") {
                inline += 1;
                println!("INLINE target {inline} (rust, then llvm per run): {t}");
                continue;
            }
            passed += num(rest, "passed");
            failed += num(rest, "failed");
            ignored += num(rest, "ignored");
            filtered += num(rest, "filtered");
            if in_e2e {
                println!("E2E {t}");
                in_e2e = false;
            }
        }
    }
    println!("CARGO passed={passed} failed={failed} ignored={ignored} filtered_out={filtered}");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.split_first() {
        Some((mode, rest)) if mode == "pairs" => pairs(rest),
        Some((mode, rest)) if mode == "counts" && rest.len() == 1 => counts(&rest[0]),
        _ => {
            eprintln!("usage: s14-summarize pairs FILE… | counts LOG");
            ExitCode::from(2)
        }
    }
}
