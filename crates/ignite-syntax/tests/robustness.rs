//! Lexer messages, recovery, nesting limits and rendering cost: the cases
//! of the 2026-11-01 review (`docs/slice.md` sections 2, 3 and 8).

use std::time::{Duration, Instant};

use ignite_syntax::ast::{Decl, ExprKind, StmtKind, Type};
use ignite_syntax::{Diagnostic, SourceFile, parse};

fn diagnostics(text: &str) -> Vec<Diagnostic> {
    match parse(&SourceFile::new("t.ignite", text)) {
        Ok(program) => panic!("expected diagnostics, parsed {program:?}"),
        Err(diagnostics) => diagnostics,
    }
}

/// `code@line:col message` of every diagnostic.
fn errors(text: &str) -> Vec<String> {
    let source = SourceFile::new("t.ignite", text);
    diagnostics(text)
        .iter()
        .map(|d| {
            let (line, col) = source.line_col(d.span.start);
            format!("{}@{line}:{col} {}", d.code, d.message)
        })
        .collect()
}

fn parses(text: &str) -> ignite_syntax::Program {
    let source = SourceFile::new("t.ignite", text);
    parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)))
}

/// Run `f` on a thread with a large stack: the debug parser needs more than
/// the 2 MiB of a test thread for 256 levels.
fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .expect("thread starts")
        .join()
        .expect("thread finishes")
}

#[test]
fn unclosed_brace_in_interpolation() {
    assert_eq!(
        errors("const G: Text = \"Hello {name\"\n"),
        ["E001@1:24 unclosed `{` in interpolation; write `{{` for a literal brace"]
    );
    assert_eq!(
        errors("const G: Text = \"{\"\n"),
        ["E001@1:18 unclosed `{` in interpolation; write `{{` for a literal brace"]
    );
}

#[test]
fn backslash_at_end_of_line_is_an_unterminated_string() {
    assert_eq!(
        errors("const G: Text = \"abc\\\n\"\n"),
        ["E001@1:17 unterminated string"]
    );
    assert_eq!(
        errors("const G: Bytes = b\"abc\\\n\"\n"),
        ["E001@1:18 unterminated byte string"]
    );
}

#[test]
fn unknown_escape_shows_one_backslash() {
    assert_eq!(
        errors("const G: Text = \"a\\q\"\n"),
        ["E001@1:19 unknown escape `\\q` in string"]
    );
    assert_eq!(
        errors("const G: Text = \"it\\'s\"\n"),
        ["E001@1:20 unknown escape `\\'` in string"]
    );
    assert_eq!(
        errors("const G: Bytes = b\"a\\q\"\n"),
        ["E001@1:21 unknown escape `\\q` in byte string"]
    );
}

#[test]
fn byte_strings_take_an_escaped_single_quote() {
    let program = parses("const G: Bytes = b\"it\\'s\"\n");
    let Decl::Const(c) = &program.decls[0] else {
        panic!("const")
    };
    let ExprKind::Method { args, .. } = &c.value.kind else {
        panic!("Bytes.of")
    };
    let ExprKind::List(items) = &args[0].kind else {
        panic!("list")
    };
    assert_eq!(items.len(), 4);
    assert!(matches!(items[2].kind, ExprKind::Int(39)));
}

#[test]
fn byte_literals_with_brace_or_quote_inside_interpolation() {
    parses("fn f() -> Text = \"{b'}'}\"\n");
    parses("fn f() -> Text = \"{b'\"'}\"\n");
    parses("fn f() -> Text = \"{b'\\''} and {b'{'}\"\n");
}

#[test]
fn smallest_integer_is_a_literal() {
    let program = parses("const MIN: Int = -9223372036854775808\n");
    let Decl::Const(c) = &program.decls[0] else {
        panic!("const")
    };
    assert!(matches!(c.value.kind, ExprKind::Int(i64::MIN)));
    let program = parses("fn f(x: Int) -> Int = x - -9223372036854775808\n");
    assert_eq!(program.decls.len(), 1);
    assert_eq!(
        errors("const MAX: Int = 9223372036854775808\n"),
        [
            "E001@1:18 integer literal `9223372036854775808` does not fit in 64 bits; only `-9223372036854775808` does"
        ]
    );
    assert_eq!(
        errors("const MAX: Int = -9223372036854775809\n"),
        ["E001@1:19 integer literal `9223372036854775809` does not fit in 64 bits"]
    );
}

#[test]
fn lexer_error_does_not_hide_earlier_parser_errors() {
    let text = "fn g( {\n}\n\nconst A: Text = \"abc\n";
    assert_eq!(
        errors(text),
        [
            "E001@1:7 expected identifier, found `{`",
            "E001@4:17 unterminated string"
        ]
    );
}

#[test]
fn greater_equal_after_type_arguments() {
    let program =
        parses("type Box<T>= T\nfn f() -> Int {\n  let x: Option<Int>= None\n  return 1\n}\n");
    assert_eq!(program.decls.len(), 2);
    let program = parses("fn f(x: List<List<Int>>, y: Int) -> Bool = y >= 1\n");
    let Decl::Fn(f) = &program.decls[0] else {
        panic!("fn")
    };
    let shown = |t: &Type| format!("{}<{}>", t.name.name, t.args.len());
    assert_eq!(shown(&f.params[0].ty), "List<1>");
    assert_eq!(shown(&f.params[0].ty.args[0]), "List<1>");
    let StmtKind::Expr(body) = &f.body.stmts[0].kind else {
        panic!("expression body")
    };
    assert!(matches!(
        body.kind,
        ExprKind::Binary {
            op: ignite_syntax::ast::BinaryOp::Ge,
            ..
        }
    ));
    assert_eq!(
        errors("fn f(a: Int) -> Bool = a > = 1\n"),
        ["E001@1:28 expected expression, found `=`"]
    );
}

#[test]
fn postfix_and_else_do_not_cross_a_line_inside_brackets() {
    assert_eq!(
        errors("fn f(first: Int, second: Int) -> List<Int> = [first\n  (second)]\n"),
        ["E001@2:3 expected `]`, found `(`"]
    );
    assert_eq!(
        errors("fn f(a: List<Int>, i: Int) -> List<Int> = [a\n  [i]]\n"),
        ["E001@2:3 expected `]`, found `[`"]
    );
    assert_eq!(
        errors("fn f(c: Bool) -> Int = (if c { 1 }\n  else { 2 })\n"),
        ["E001@2:3 expected `)`, found `else`"]
    );
    parses("fn f(first: Int, second: Int) -> List<Int> = [first,\n  second]\n");
}

#[test]
fn one_element_tuple_type_is_e001() {
    assert_eq!(
        errors("fn f(x: (Int,)) -> Int = x\n"),
        ["E001@1:9 a tuple type has at least two elements; drop the `,` to group a type"]
    );
}

#[test]
fn nested_fn_gives_one_error() {
    assert_eq!(
        errors(
            "fn main() -> Unit {\n  fn helper() -> Int = 1\n  helper()\n}\n\nfn after() -> Int = 2\n"
        ),
        ["E001@2:3 `fn` declarations are only allowed at the top level; move it out of the block"]
    );
    assert_eq!(
        errors("fn main() -> Unit {\n  if true {\n    fn helper() { }\n  }\n}\nfn bad( {\n}\n")
            .len(),
        2
    );
}

fn nested_parens(levels: usize) -> String {
    format!(
        "fn f() -> Int {{\n  return {}1{}\n}}\n",
        "(".repeat(levels),
        ")".repeat(levels)
    )
}

#[test]
fn nesting_is_limited_to_256_levels() {
    big_stack(|| {
        parses(&nested_parens(255));
        let deep = errors(&nested_parens(256));
        assert_eq!(
            deep,
            ["E054@2:265 nesting deeper than 256; split the expression"]
        );
        let deep = errors(&nested_parens(100_000));
        assert_eq!(deep.len(), 1);
        assert!(deep[0].starts_with("E054@2:265 "), "{deep:?}");
        let prefix = format!("fn f() -> Int = {}1\n", "-".repeat(100_000));
        assert_eq!(
            errors(&prefix),
            ["E054@1:273 nesting deeper than 256; split the expression"]
        );
        let lists = format!("const L: Int = {}1{}\n", "[".repeat(300), "]".repeat(300));
        assert_eq!(
            errors(&lists),
            ["E054@1:272 nesting deeper than 256; split the expression"]
        );
        let ty = format!(
            "fn f(x: {}Int{}) -> Int = 1\n",
            "List<".repeat(300),
            ">".repeat(300)
        );
        assert_eq!(errors(&ty).len(), 1);
        assert!(errors(&ty)[0].starts_with("E054@"));
        let blocks = format!(
            "fn f() {{\n{}{}\n}}\n",
            "if true { ".repeat(300),
            "} ".repeat(300)
        );
        assert!(errors(&blocks)[0].starts_with("E054@"));
    });
}

#[test]
fn long_chains_parse() {
    big_stack(|| {
        let sum = format!("fn f() -> Int = {}1\n", "1 + ".repeat(10_000));
        parses(&sum);
        let interpolation = format!("fn f(a: Int) -> Text = \"{}\"\n", "{a}".repeat(10_000));
        parses(&interpolation);
    });
}

#[test]
fn rendering_many_diagnostics_is_linear() {
    let text = "fn () {}\n".repeat(20_000);
    let source = SourceFile::new("t.ignite", text);
    let start = Instant::now();
    let diagnostics = parse(&source).expect_err("every line is an error");
    assert_eq!(diagnostics.len(), 20_000);
    let rendered = source.render_all(&diagnostics);
    let elapsed = start.elapsed();
    assert_eq!(rendered.lines().count(), 20_000);
    assert!(
        rendered
            .lines()
            .last()
            .unwrap()
            .starts_with("t.ignite:20000:4: "),
        "{}",
        rendered.lines().last().unwrap()
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "20,000 diagnostics took {elapsed:?}"
    );
}

/// The line table is computed once per file: 100,000 lookups on a
/// 100,000-line file cost one scan of the text, not one per lookup.
#[test]
fn line_col_computes_the_line_table_once() {
    let text = "fn f() {}\n".repeat(100_000);
    let source = SourceFile::new("t.ignite", text);
    let start = Instant::now();
    let mut lines = 0;
    for offset in (0..source.text.len()).step_by(10) {
        let (line, col) = source.line_col(offset);
        assert_eq!(col, 1);
        lines = line;
    }
    let elapsed = start.elapsed();
    assert_eq!(lines, 100_000);
    assert!(
        elapsed < Duration::from_secs(1),
        "100,000 line_col calls took {elapsed:?}"
    );
}
