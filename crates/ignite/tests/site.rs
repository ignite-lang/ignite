//! The website under `site/`: every Ignite code block in the book's
//! Markdown and on the landing page passes `ignite check`, so the site
//! cannot show a program the compiler rejects.
//!
//! A Markdown fence ```` ```ignite ```` is checked as a whole program. Its
//! info string may add comma-separated attributes: `fragment` skips the
//! block, `fails=E040` expects `ignite check` to fail with that code, and
//! `dir=examples/terminal` checks the block next to copies of that
//! directory's `.ignite` files, so `module("…")` finds its siblings. On the
//! landing page the same attributes are `data-fragment`, `data-fails` and
//! `data-dir` on a `<code class="language-ignite">` element.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, PartialEq)]
struct Block {
    origin: String,
    line: usize,
    source: String,
    fragment: bool,
    fails: Option<String>,
    dir: Option<String>,
    unknown: Vec<String>,
}

impl Block {
    fn new(origin: &str, line: usize, attributes: &[(&str, Option<&str>)]) -> Block {
        let mut block = Block {
            origin: origin.to_string(),
            line,
            source: String::new(),
            fragment: false,
            fails: None,
            dir: None,
            unknown: Vec::new(),
        };
        for (name, value) in attributes {
            match (*name, value) {
                ("fragment", None) => block.fragment = true,
                ("fails", Some(code)) => block.fails = Some(code.to_string()),
                ("dir", Some(dir)) => block.dir = Some(dir.to_string()),
                (name, Some(value)) => block.unknown.push(format!("{name}={value}")),
                (name, None) => block.unknown.push(name.to_string()),
            }
        }
        block
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The fenced ```` ```ignite ```` blocks of one Markdown text. A fence may
/// be indented (inside a list item); its indentation is removed from the
/// block's lines.
fn markdown_blocks(origin: &str, text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((number, line)) = lines.next() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();
        let ticks = trimmed.chars().take_while(|c| *c == '`').count();
        if ticks < 3 {
            continue;
        }
        let fence = &trimmed[..ticks];
        let info = trimmed[ticks..].trim();
        let mut parts = info.split(',').map(str::trim);
        let language = parts.next().unwrap_or("");
        let attributes: Vec<(&str, Option<&str>)> = parts
            .filter(|part| !part.is_empty())
            .map(|part| match part.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (part, None),
            })
            .collect();
        let mut body = Vec::new();
        for (_, inner) in lines.by_ref() {
            if inner.trim_start().starts_with(fence) && inner.trim() == fence {
                break;
            }
            let strip = inner.len() - inner.trim_start().len();
            body.push(&inner[strip.min(indent)..]);
        }
        if language == "ignite" {
            let mut block = Block::new(origin, number + 1, &attributes);
            block.source = body.join("\n") + "\n";
            blocks.push(block);
        }
    }
    blocks
}

fn unescape_html(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The `<code class="language-ignite" …>` elements of one HTML text.
fn html_blocks(origin: &str, text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = text;
    let mut offset = 0;
    while let Some(at) = rest.find("<code") {
        let start = offset + at;
        let tag_end = match text[start..].find('>') {
            Some(end) => start + end,
            None => break,
        };
        let tag = &text[start..tag_end];
        let close = text[tag_end..]
            .find("</code>")
            .map(|end| tag_end + end)
            .unwrap_or(text.len());
        if tag.contains("class=\"language-ignite\"") {
            let attribute = |name: &str| {
                let key = format!("{name}=\"");
                tag.find(&key).map(|at| {
                    let value = &tag[at + key.len()..];
                    &value[..value.find('"').unwrap_or(value.len())]
                })
            };
            let mut attributes = Vec::new();
            if tag.contains("data-fragment") {
                attributes.push(("fragment", None));
            }
            if let Some(code) = attribute("data-fails") {
                attributes.push(("fails", Some(code)));
            }
            if let Some(dir) = attribute("data-dir") {
                attributes.push(("dir", Some(dir)));
            }
            let line = text[..start].matches('\n').count() + 1;
            let mut block = Block::new(origin, line, &attributes);
            block.source = unescape_html(&text[tag_end + 1..close]) + "\n";
            blocks.push(block);
        }
        offset = close.min(text.len());
        rest = &text[offset..];
        if rest.is_empty() {
            break;
        }
        offset += 1;
        rest = &text[offset..];
    }
    blocks
}

fn site_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            site_files(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("md") | Some("html")
        ) {
            out.push(path);
        }
    }
}

fn site_blocks() -> Vec<Block> {
    let root = workspace();
    let mut files = Vec::new();
    site_files(&root.join("site"), &mut files);
    let mut blocks = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let origin = path.strip_prefix(&root).unwrap().display().to_string();
        if origin.ends_with(".md") {
            blocks.extend(markdown_blocks(&origin, &text));
        } else {
            blocks.extend(html_blocks(&origin, &text));
        }
    }
    blocks
}

/// `None` when the block behaves as its attributes say, else the reason.
fn check(block: &Block, index: usize) -> Option<String> {
    let root = workspace();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("site")
        .join(index.to_string());
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(siblings) = &block.dir {
        let source = root.join(siblings);
        let entries = match std::fs::read_dir(&source) {
            Ok(entries) => entries,
            Err(error) => return Some(format!("dir={siblings}: {error}")),
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("ignite") {
                std::fs::copy(&path, dir.join(path.file_name().unwrap())).unwrap();
            }
        }
    }
    let file = dir.join("site_block.ignite");
    std::fs::write(&file, &block.source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg("--std")
        .arg(root.join("std"))
        .arg(&file)
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    match &block.fails {
        None if output.status.success() && report.is_empty() => None,
        None => Some(format!("expected a clean check, got:\n{report}")),
        Some(code) if !output.status.success() && report.contains(&format!("error[{code}]")) => {
            None
        }
        Some(code) => Some(format!("expected error[{code}], got:\n{report}")),
    }
}

#[test]
fn the_extractor_reads_fences_attributes_and_html() {
    let markdown = "text\n\n```ignite\nfn a() -> Int = 1\n```\n\n- item\n\n  ```ignite,fails=E003\n  fn b() -> Int = c\n  ```\n\n```ignite,fragment\nx\n```\n\n```text\nignored\n```\n\n````ignite,dir=examples/todo\n```\n````\n";
    let blocks = markdown_blocks("a.md", markdown);
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[0].source, "fn a() -> Int = 1\n");
    assert_eq!(blocks[0].line, 3);
    assert_eq!(blocks[1].source, "fn b() -> Int = c\n");
    assert_eq!(blocks[1].fails.as_deref(), Some("E003"));
    assert!(blocks[2].fragment);
    assert_eq!(blocks[3].dir.as_deref(), Some("examples/todo"));
    assert_eq!(blocks[3].source, "```\n");

    let html = "<p>x</p>\n<pre><code class=\"language-ignite\" data-dir=\"examples/terminal\">fn a() -&gt; <b>Int</b> = 1</code></pre>\n<pre><code class=\"language-text\">no</code></pre><pre><code class=\"language-ignite\" data-fragment>y</code></pre>";
    let blocks = html_blocks("index.html", html);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].source, "fn a() -> Int = 1\n");
    assert_eq!(blocks[0].dir.as_deref(), Some("examples/terminal"));
    assert_eq!(blocks[0].line, 2);
    assert!(blocks[1].fragment);

    let odd = markdown_blocks("b.md", "```ignite,norun\nfn a() -> Int = 1\n```\n");
    assert_eq!(odd[0].unknown, vec!["norun".to_string()]);
}

#[test]
fn every_ignite_block_on_the_site_checks() {
    let blocks = site_blocks();
    let landing = blocks
        .iter()
        .filter(|block| block.origin == "site/index.html" && !block.fragment)
        .count();
    assert!(landing > 0, "no checked Ignite block on site/index.html");
    let checked: Vec<(usize, &Block)> = blocks
        .iter()
        .filter(|block| !block.fragment)
        .enumerate()
        .collect();
    assert!(
        checked.len() > landing,
        "no checked Ignite block in the book"
    );

    let mut failures: Vec<String> = blocks
        .iter()
        .filter(|block| !block.unknown.is_empty())
        .map(|block| {
            format!(
                "{}:{}: unknown attribute {}; use fragment, fails=Ennn or dir=PATH",
                block.origin,
                block.line,
                block.unknown.join(", ")
            )
        })
        .collect();
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let results: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let checked = &checked;
                scope.spawn(move || {
                    checked
                        .iter()
                        .skip(worker)
                        .step_by(workers)
                        .filter_map(|(index, block)| {
                            check(block, *index)
                                .map(|reason| format!("{}:{}: {reason}", block.origin, block.line))
                        })
                        .collect::<Vec<String>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    failures.extend(results);
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} of {} checked Ignite blocks on the site fail:\n\n{}",
        failures.len(),
        checked.len(),
        failures.join("\n\n")
    );
}

#[test]
fn pages_name_the_installed_command_not_a_build_path() {
    let root = workspace();
    let mut files = Vec::new();
    site_files(&root.join("site"), &mut files);
    let mut found = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        let origin = path.strip_prefix(&root).unwrap().display().to_string();
        for (number, line) in text.lines().enumerate() {
            for path in ["target/debug/ignite", "target/release/ignite"] {
                if line.contains(path) {
                    found.push(format!("{origin}:{}: {path}", number + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "pages name a build path instead of the installed `ignite`:\n{}",
        found.join("\n")
    );
}

mod ux;
