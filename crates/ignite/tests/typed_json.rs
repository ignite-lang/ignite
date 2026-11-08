//! `ignite emit --typed-json` (`docs/slice.md` section 24): every file of
//! `std/` and `examples/` gives one document of format `ignite-typed/0.1`
//! whose declaration, statement and expression objects each carry a span,
//! whose expressions each carry a type, and whose declaration, statement and
//! expression counts are those of the parsed file; the same file gives the
//! same bytes twice; the document `examples/typed/reader.ignite` reads is the
//! one the compiler prints for `examples/typed/sample.ignite`.
//!
//! `IGNITE_BLESS=1 cargo test -p ignite --test main typed_json::` rewrites
//! the reader's fixture from the current output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ignite_eval::Json;
use ignite_syntax::ast::{
    Arm, Block, Decl, Expr, ExprKind, FieldInit, FnDecl, Pattern, PatternKind, Stmt, StmtKind,
};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn emit(file: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["emit", "--typed-json", file])
        .current_dir(workspace())
        .output()
        .unwrap()
}

fn ignite_files(dir: &Path, root: &Path, files: &mut Vec<String>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            ignite_files(&entry, root, files);
        } else if entry.extension().is_some_and(|ext| ext == "ignite") {
            let relative = entry.strip_prefix(root).unwrap();
            files.push(relative.to_str().unwrap().to_string());
        }
    }
}

/// Statements, expressions and patterns of a parsed file, counted by a
/// walk of the AST independent of the serialiser.
#[derive(Default, Debug, PartialEq, Eq)]
struct Counts {
    stmts: usize,
    exprs: usize,
    patterns: usize,
}

impl Counts {
    fn decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Fn(f) => self.function(f),
            Decl::Record(r) => r
                .fields
                .iter()
                .filter_map(|f| f.default.as_ref())
                .for_each(|e| self.expr(e)),
            Decl::Enum(_) | Decl::Alias(_) => {}
            Decl::Const(c) => self.expr(&c.value),
            Decl::Test(t) => self.block(&t.body),
            Decl::Protocol(p) => p.fns.iter().for_each(|f| self.function(f)),
            Decl::System(s) => {
                for r in &s.resources {
                    self.fields(&r.properties);
                }
                for c in s.components.iter().chain(&s.contracts).chain(&s.policies) {
                    self.fields(&c.properties);
                }
            }
            Decl::Assembly(a) => self.fields(&a.properties),
        }
    }

    fn function(&mut self, f: &FnDecl) {
        self.block(&f.body);
    }

    fn fields(&mut self, fields: &[FieldInit]) {
        fields.iter().for_each(|f| self.expr(&f.value));
    }

    fn block(&mut self, block: &Block) {
        block.stmts.iter().for_each(|s| self.stmt(s));
    }

    fn stmt(&mut self, stmt: &Stmt) {
        self.stmts += 1;
        match &stmt.kind {
            StmtKind::Let { value, .. } => self.expr(value),
            StmtKind::LetPattern { pattern, value, .. } => {
                self.pattern(pattern);
                self.expr(value);
            }
            StmtKind::Assign { value, .. } => self.expr(value),
            StmtKind::Return(value) => value.iter().for_each(|e| self.expr(e)),
            StmtKind::While { cond, body } => {
                self.expr(cond);
                self.block(body);
            }
            StmtKind::Loop { body } => self.block(body),
            StmtKind::For { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                self.pattern(pattern);
                self.expr(iter);
                self.block(body);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(e) => self.expr(e),
            StmtKind::Assert { cond, message } => {
                self.expr(cond);
                message.iter().for_each(|e| self.expr(e));
            }
        }
    }

    fn arm(&mut self, arm: &Arm) {
        self.pattern(&arm.pattern);
        self.expr(&arm.body);
    }

    fn expr(&mut self, expr: &Expr) {
        self.exprs += 1;
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::Str(_)
            | ExprKind::Bool(_)
            | ExprKind::Name(_)
            | ExprKind::Break
            | ExprKind::Continue
            | ExprKind::Quantity { .. } => {}
            ExprKind::Record { fields, .. } => self.fields(fields),
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                items.iter().for_each(|e| self.expr(e))
            }
            ExprKind::Return(value) => value.iter().for_each(|e| self.expr(e)),
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                args.iter().for_each(|e| self.expr(e));
            }
            ExprKind::Method { receiver, args, .. } => {
                self.expr(receiver);
                args.iter().for_each(|e| self.expr(e));
            }
            ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                self.expr(receiver)
            }
            ExprKind::Index { receiver, index } => {
                self.expr(receiver);
                self.expr(index);
            }
            ExprKind::Try(e) | ExprKind::Await(e) | ExprKind::Spawn(e) => self.expr(e),
            ExprKind::Unary { operand, .. } => self.expr(operand),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                arms.iter().for_each(|a| self.arm(a));
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                self.block(then);
                otherwise.iter().for_each(|e| self.expr(e));
            }
            ExprKind::Block(block) => self.block(block),
            ExprKind::Closure(closure) => self.expr(&closure.body),
            ExprKind::RecordUpdate { fields, base, .. } => {
                self.fields(fields);
                self.expr(base);
            }
            ExprKind::NamedArg { value, .. } => self.expr(value),
            ExprKind::Brace { entries } => entries.iter().for_each(|e| self.expr(&e.value)),
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        self.patterns += 1;
        match &pattern.kind {
            PatternKind::Or(items) | PatternKind::Tuple(items) | PatternKind::List(items) => {
                items.iter().for_each(|p| self.pattern(p))
            }
            PatternKind::Variant { args, .. } => args.iter().for_each(|p| self.pattern(p)),
            PatternKind::Range { start, end } => {
                self.pattern(start);
                self.pattern(end);
            }
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_)
            | PatternKind::Rest(_)
            | PatternKind::Binding(_) => {}
        }
    }
}

fn parsed_counts(file: &str) -> (usize, Counts) {
    let text = std::fs::read_to_string(workspace().join(file)).unwrap();
    let source = ignite_syntax::SourceFile::new(file, text);
    let program = ignite_syntax::parse(&source).unwrap();
    let mut counts = Counts::default();
    program.decls.iter().for_each(|d| counts.decl(d));
    (program.decls.len(), counts)
}

fn member<'a>(value: &'a Json, key: &str) -> Option<&'a Json> {
    match value {
        Json::Object(members) => members.get(key),
        _ => None,
    }
}

fn array<'a>(document: &'a Json, key: &str, file: &str) -> &'a [Json] {
    match member(document, key) {
        Some(Json::Array(items)) => items,
        other => panic!("{file}: `{key}` is no array: {other:?}"),
    }
}

/// `span` of `object` is `{file, line, column, ..}` with `file` an index of
/// `files`.
fn assert_span(object: &Json, files: usize, what: &str) {
    let span = member(object, "span").unwrap_or_else(|| panic!("{what} has no span: {object:?}"));
    match member(span, "file") {
        Some(Json::Int(n)) if (*n as usize) < files => {}
        other => panic!("{what}: span file {other:?} is no index of `files`"),
    }
    for key in ["line", "column", "endLine", "endColumn", "start", "end"] {
        match member(span, key) {
            Some(Json::Int(n)) if *n >= 0 => {}
            other => panic!("{what}: span {key} is {other:?}"),
        }
    }
}

/// A type object: `kind` and `text` strings.
fn assert_type(ty: &Json, what: &str) {
    for key in ["kind", "text"] {
        match member(ty, key) {
            Some(Json::Str(_)) => {}
            other => panic!("{what}: type {key} is {other:?} in {ty:?}"),
        }
    }
}

/// Every member `type`, `ret` and `bindingType` anywhere in the document
/// (declarations, parameters, fields, closures, bindings) is a type
/// object: none of the files is left with a type the typer did not keep.
fn assert_types_everywhere(value: &Json, file: &str) {
    match value {
        Json::Object(members) => {
            for (key, member) in members {
                if matches!(key.as_str(), "type" | "ret" | "bindingType") {
                    assert_type(member, &format!("{file}: `{key}`"));
                }
                assert_types_everywhere(member, file);
            }
        }
        Json::Array(items) => items.iter().for_each(|i| assert_types_everywhere(i, file)),
        _ => {}
    }
}

#[test]
fn every_file_of_std_and_examples_gives_a_complete_document() {
    let root = workspace();
    let mut files = Vec::new();
    ignite_files(&root.join("std"), &root, &mut files);
    ignite_files(&root.join("examples"), &root, &mut files);
    assert!(files.len() > 30, "found {files:?}");
    for file in &files {
        let output = emit(file);
        assert!(
            output.status.success(),
            "{file}: exit {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.ends_with('\n'), "{file}: no final newline");
        let document = Json::parse(text.trim_end()).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(
            member(&document, "formatVersion"),
            Some(&Json::Str("ignite-typed/0.1".into())),
            "{file}"
        );
        let file_count = array(&document, "files", file).len();
        assert_eq!(
            array(&document, "files", file).first(),
            Some(&Json::Str(file.clone())),
            "{file}"
        );
        let decls = array(&document, "decls", file);
        for (i, decl) in decls.iter().enumerate() {
            assert_span(decl, file_count, &format!("{file}: decl {i}"));
            if let Some(Json::Array(roles)) = member(decl, "roles") {
                for role in roles {
                    assert_span(role, file_count, &format!("{file}: role of decl {i}"));
                }
            }
        }
        let stmts = array(&document, "stmts", file);
        for (i, stmt) in stmts.iter().enumerate() {
            assert_span(stmt, file_count, &format!("{file}: stmt {i}"));
        }
        let exprs = array(&document, "exprs", file);
        for (i, expr) in exprs.iter().enumerate() {
            let what = format!("{file}: expr {i}");
            assert_span(expr, file_count, &what);
            let ty = member(expr, "type").unwrap_or_else(|| panic!("{what} has no type"));
            assert_type(ty, &what);
        }
        let patterns = array(&document, "patterns", file);
        for (i, pattern) in patterns.iter().enumerate() {
            assert_span(pattern, file_count, &format!("{file}: pattern {i}"));
        }
        assert_types_everywhere(&document, file);
        let (decl_count, counts) = parsed_counts(file);
        assert_eq!(decls.len(), decl_count, "{file}: declarations");
        assert_eq!(
            Counts {
                stmts: stmts.len(),
                exprs: exprs.len(),
                patterns: patterns.len()
            },
            counts,
            "{file}: statements, expressions, patterns"
        );
    }
}

#[test]
fn the_same_file_gives_the_same_bytes_twice() {
    let first = emit("examples/todo/todo.ignite");
    let second = emit("examples/todo/todo.ignite");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(!first.stdout.is_empty());
    assert!(first.stdout == second.stdout, "the two documents differ");
}

#[test]
fn a_fn_carries_its_parameters_and_a_ufcs_call_its_callee() {
    let output = emit("examples/typed/sample.ignite");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = Json::parse(String::from_utf8(output.stdout).unwrap().trim_end()).unwrap();
    let file = "sample";
    let describe = array(&document, "decls", file)
        .iter()
        .find(|d| member(d, "name") == Some(&Json::Str("describe".into())))
        .expect("the fn describe");
    let params: Vec<(String, String)> = array(describe, "params", file)
        .iter()
        .map(|p| {
            let text = |v: Option<&Json>| match v {
                Some(Json::Str(s)) => s.clone(),
                other => panic!("{other:?}"),
            };
            (
                text(member(p, "name")),
                text(member(member(p, "type").unwrap(), "text")),
            )
        })
        .collect();
    assert_eq!(
        params,
        [
            ("box".to_string(), "Box".to_string()),
            ("scale".to_string(), "Int".to_string()),
            ("separator".to_string(), "Text".to_string())
        ]
    );
    let split = array(&document, "exprs", file)
        .iter()
        .find(|e| member(e, "kind") == Some(&Json::Str("method".into())))
        .expect("the method call");
    assert_eq!(member(split, "ufcs"), Some(&Json::Bool(true)));
    let callee = member(split, "callee").unwrap();
    assert_eq!(member(callee, "kind"), Some(&Json::Str("fn".into())));
    assert_eq!(
        member(callee, "module"),
        Some(&Json::Str("std.text".into()))
    );
    assert_eq!(member(callee, "name"), Some(&Json::Str("split".into())));
    let ty = member(split, "type").unwrap();
    assert_eq!(member(ty, "text"), Some(&Json::Str("List<Text>".into())));
}

/// `docs/slice.md` 32.6: a `U32` parameter and a `U32` expression carry
/// the type `kind` `U32`, with no further keys.
#[test]
fn a_u32_is_named_in_the_document() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("typed-json-u32");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("words.ignite");
    std::fs::write(
        &file,
        "fn mix(x: U32, n: Int) -> U32 = x.rotate_left(n) + 0x5A827999\n",
    )
    .unwrap();
    let output = emit(file.to_str().unwrap());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = Json::parse(String::from_utf8(output.stdout).unwrap().trim_end()).unwrap();
    let mix = array(&document, "decls", "words")
        .iter()
        .find(|d| member(d, "name") == Some(&Json::Str("mix".into())))
        .expect("the fn mix");
    let x = &array(mix, "params", "words")[0];
    let word = Json::Object(
        [
            ("kind".to_string(), Json::Str("U32".into())),
            ("text".to_string(), Json::Str("U32".into())),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(member(x, "type"), Some(&word));
    let rotate = array(&document, "exprs", "words")
        .iter()
        .find(|e| member(e, "kind") == Some(&Json::Str("method".into())))
        .expect("the rotate");
    assert_eq!(member(rotate, "type"), Some(&word));
    let literal = array(&document, "exprs", "words")
        .iter()
        .find(|e| member(e, "kind") == Some(&Json::Str("int".into())))
        .expect("the literal");
    assert_eq!(member(literal, "type"), Some(&word));
}

const READER: &str = "examples/typed/reader.ignite";
const OPEN: &str = "test const SAMPLE: Text = r##\"";

#[test]
fn the_reader_fixture_is_the_sample_document() {
    let output = emit("examples/typed/sample.ignite");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = String::from_utf8(output.stdout).unwrap();
    let document = document.trim_end();
    assert!(!document.contains("\"##"), "the raw string cannot hold it");
    let path = workspace().join(READER);
    let reader = std::fs::read_to_string(&path).unwrap();
    let start = reader.find(OPEN).expect("the SAMPLE constant") + OPEN.len();
    let end = start + reader[start..].find("\"##").expect("its end");
    if std::env::var_os("IGNITE_BLESS").is_some() {
        let rewritten = format!("{}{document}{}", &reader[..start], &reader[end..]);
        std::fs::write(&path, rewritten).unwrap();
        return;
    }
    assert!(
        &reader[start..end] == document,
        "{READER}: SAMPLE differs from `ignite emit --typed-json examples/typed/sample.ignite`; IGNITE_BLESS=1 rewrites it"
    );
}
