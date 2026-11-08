//! One test per grammar rule of `docs/slice.md` section 3. Shapes are
//! compared as S-expressions printed by `show`.

use ignite_syntax::ast::{
    Block, Decl, Expr, ExprKind, Pattern, PatternKind, Program, Stmt, StmtKind, Type, UnaryOp,
};
use ignite_syntax::{SourceFile, parse};

fn program(text: &str) -> Program {
    let source = SourceFile::new("t.ignite", text);
    match parse(&source) {
        Ok(program) => program,
        Err(diagnostics) => panic!(
            "{}\n--- source ---\n{text}",
            source.render_all(&diagnostics)
        ),
    }
}

/// `code@line:col` of every diagnostic, in reported order.
fn errors(text: &str) -> Vec<String> {
    let source = SourceFile::new("t.ignite", text);
    match parse(&source) {
        Ok(program) => panic!("expected diagnostics, parsed {program:?}"),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|d| {
                let (line, col) = source.line_col(d.span.start);
                format!("{}@{line}:{col}", d.code)
            })
            .collect(),
    }
}

/// Statements of `fn f() { <body> }`.
fn stmts(body: &str) -> Vec<Stmt> {
    let program = program(&format!("fn f() {{\n{body}\n}}"));
    match program.decls.into_iter().next() {
        Some(Decl::Fn(f)) => f.body.stmts,
        other => panic!("expected a fn, got {other:?}"),
    }
}

/// Statements of `fn f() { <body> }`, printed.
fn shown(body: &str) -> Vec<String> {
    stmts(body).iter().map(show_stmt).collect()
}

/// The single expression statement of `fn f() { <expr> }`, printed.
fn expr(text: &str) -> String {
    let all = shown(text);
    assert_eq!(all.len(), 1, "one statement expected: {all:?}");
    all.into_iter().next().unwrap_or_default()
}

fn show_type(ty: &Type) -> String {
    if ty.args.is_empty() {
        ty.name.name.clone()
    } else {
        let args: Vec<String> = ty.args.iter().map(show_type).collect();
        format!("{}<{}>", ty.name.name, args.join(", "))
    }
}

fn show_block(block: &Block) -> String {
    let stmts: Vec<String> = block.stmts.iter().map(show_stmt).collect();
    format!("{{{}}}", stmts.join("; "))
}

fn show_stmt(stmt: &Stmt) -> String {
    match &stmt.kind {
        StmtKind::LetPattern {
            pattern,
            value,
            mutable,
        } => {
            let keyword = if *mutable { "var" } else { "let" };
            format!("({keyword} {} {})", show_pattern(pattern), show(value))
        }
        StmtKind::Let {
            name,
            ty,
            value,
            mutable,
        } => {
            let keyword = if *mutable { "var" } else { "let" };
            let ty = ty
                .as_ref()
                .map(|t| format!(": {}", show_type(t)))
                .unwrap_or_default();
            format!("({keyword} {}{ty} {})", name.name, show(value))
        }
        StmtKind::Assign { target, value } => {
            let mut place = target.base.name.clone();
            for field in &target.fields {
                place.push('.');
                place.push_str(&field.name);
            }
            format!("(= {place} {})", show(value))
        }
        StmtKind::Return(None) => "(return)".to_string(),
        StmtKind::Return(Some(e)) => format!("(return {})", show(e)),
        StmtKind::While { cond, body } => format!("(while {} {})", show(cond), show_block(body)),
        StmtKind::Loop { body } => format!("(loop {})", show_block(body)),
        StmtKind::For { name, iter, body } => {
            format!("(for {} {} {})", name.name, show(iter), show_block(body))
        }
        StmtKind::ForPattern {
            pattern,
            iter,
            body,
        } => format!(
            "(for {} {} {})",
            show_pattern(pattern),
            show(iter),
            show_block(body)
        ),
        StmtKind::Break => "(break)".to_string(),
        StmtKind::Continue => "(continue)".to_string(),
        StmtKind::Expr(e) => show(e),
        StmtKind::Assert {
            cond,
            message: None,
        } => format!("(assert {})", show(cond)),
        StmtKind::Assert {
            cond,
            message: Some(m),
        } => format!("(assert {} {})", show(cond), show(m)),
    }
}

fn show_all(exprs: &[Expr]) -> String {
    exprs.iter().map(|e| format!(" {}", show(e))).collect()
}

fn show(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Tuple(items) => format!("(tuple{})", show_all(items)),
        ExprKind::Return(None) => "(return)".to_string(),
        ExprKind::Return(Some(e)) => format!("(return {})", show(e)),
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Quantity { literal, unit } => format!("(quantity {literal} {})", unit.name),
        ExprKind::NamedArg { name, value } => format!("(= {} {})", name.name, show(value)),
        ExprKind::Brace { entries } => {
            let entries: String = entries
                .iter()
                .map(|e| format!(" ({} {})", e.key.dotted(), show(&e.value)))
                .collect();
            format!("(brace{entries})")
        }
        ExprKind::Spawn(e) => format!("(spawn {})", show(e)),
        ExprKind::Str(s) => format!("{s:?}"),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Name(n) => n.name.clone(),
        ExprKind::Record { name, fields } => {
            let fields: String = fields
                .iter()
                .map(|f| format!(" ({} {})", f.name.name, show(&f.value)))
                .collect();
            format!("(record {}{fields})", name.name)
        }
        ExprKind::List(items) => format!("(list{})", show_all(items)),
        ExprKind::Call { callee, args } => format!("(call {}{})", show(callee), show_all(args)),
        ExprKind::Method {
            receiver,
            name,
            args,
        } => format!(
            "(method {} {}{})",
            show(receiver),
            name.name,
            show_all(args)
        ),
        ExprKind::Field { receiver, name } => format!("(. {} {})", show(receiver), name.name),
        ExprKind::Index { receiver, index } => {
            format!("(index {} {})", show(receiver), show(index))
        }
        ExprKind::Try(e) => format!("(? {})", show(e)),
        ExprKind::Await(_) => unreachable!("the parser no longer produces ExprKind::Await"),
        ExprKind::Unary { op, operand } => {
            let op = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "neg",
                UnaryOp::BitNot => "~",
            };
            format!("({op} {})", show(operand))
        }
        ExprKind::Binary { op, left, right } => {
            format!("({} {} {})", op.symbol(), show(left), show(right))
        }
        ExprKind::Match { scrutinee, arms } => {
            let arms: String = arms
                .iter()
                .map(|a| format!(" ({} => {})", show_pattern(&a.pattern), show(&a.body)))
                .collect();
            format!("(match {}{arms})", show(scrutinee))
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => match otherwise {
            Some(o) => format!("(if {} {} {})", show(cond), show_block(then), show(o)),
            None => format!("(if {} {})", show(cond), show_block(then)),
        },
        ExprKind::Block(b) => show_block(b),
        ExprKind::Closure(c) => {
            let params: Vec<String> = c
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => format!("{}: {}", p.name.name, show_type(t)),
                    None => p.name.name.clone(),
                })
                .collect();
            format!("(closure [{}] {})", params.join(", "), show(&c.body))
        }
        ExprKind::RecordUpdate { name, fields, base } => {
            let fields: String = fields
                .iter()
                .map(|f| format!(" ({} {})", f.name.name, show(&f.value)))
                .collect();
            format!("(update {}{fields} ..{})", name.name, show(base))
        }
        ExprKind::TupleField { receiver, index } => format!("(. {} {index})", show(receiver)),
        ExprKind::Break => "(break)".to_string(),
        ExprKind::Continue => "(continue)".to_string(),
    }
}

fn show_pattern(pattern: &Pattern) -> String {
    match &pattern.kind {
        PatternKind::Bool(b) => b.to_string(),
        PatternKind::Or(alternatives) => {
            let alternatives: Vec<String> = alternatives.iter().map(show_pattern).collect();
            format!("(| {})", alternatives.join(" "))
        }
        PatternKind::Tuple(items) => {
            let items: Vec<String> = items.iter().map(show_pattern).collect();
            format!("({})", items.join(", "))
        }
        PatternKind::List(items) => {
            let items: Vec<String> = items.iter().map(show_pattern).collect();
            format!("[{}]", items.join(", "))
        }
        PatternKind::Wildcard => "_".to_string(),
        PatternKind::Rest(None) => "..".to_string(),
        PatternKind::Rest(Some(name)) => format!("..${}", name.name),
        PatternKind::Range { start, end } => {
            format!("{}..={}", show_pattern(start), show_pattern(end))
        }
        PatternKind::Int(n) => n.to_string(),
        PatternKind::Str(s) => format!("{s:?}"),
        PatternKind::Binding(b) => format!("${}", b.name),
        PatternKind::Variant { path, args } => {
            let path: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
            if args.is_empty() {
                path.join(".")
            } else {
                let args: Vec<String> = args.iter().map(show_pattern).collect();
                format!("{}({})", path.join("."), args.join(", "))
            }
        }
    }
}

// statements

#[test]
fn stmt_let() {
    assert_eq!(shown("let x = 1"), ["(let x 1)"]);
}

#[test]
fn stmt_let_with_type_annotation() {
    assert_eq!(
        shown("let x: List<Text> = []"),
        ["(let x: List<Text> (list))"]
    );
    assert_eq!(
        shown("let r: Result<Unit, IoError> = Ok(Unit)"),
        ["(let r: Result<Unit, IoError> (call Ok Unit))"]
    );
}

#[test]
fn stmt_var() {
    assert_eq!(shown("var count: Int = 0"), ["(var count: Int 0)"]);
}

#[test]
fn stmt_assign_to_field_path() {
    assert_eq!(shown("a.b.c = 1"), ["(= a.b.c 1)"]);
    assert_eq!(shown("a = a + 1"), ["(= a (+ a 1))"]);
}

#[test]
fn stmt_assign_to_call_is_e001() {
    assert_eq!(errors("fn f() {\n  g() = 1\n}"), ["E001@2:3"]);
}

#[test]
fn stmt_return() {
    assert_eq!(shown("return"), ["(return)"]);
    assert_eq!(shown("return 1 + 2"), ["(return (+ 1 2))"]);
    assert_eq!(shown("return\nx"), ["(return)", "x"]);
}

#[test]
fn stmt_while() {
    assert_eq!(
        shown("while i < n { i = i + 1 }"),
        ["(while (< i n) {(= i (+ i 1))})"]
    );
}

#[test]
fn stmt_loop() {
    assert_eq!(shown("loop { break }"), ["(loop {(break)})"]);
}

#[test]
fn stmt_for() {
    assert_eq!(
        shown("for line in reply.lines { out.append(line) }"),
        ["(for line (. reply lines) {(method out append line)})"]
    );
}

#[test]
fn stmt_break_and_continue() {
    assert_eq!(shown("break\ncontinue"), ["(break)", "(continue)"]);
}

#[test]
fn stmt_expression() {
    assert_eq!(shown("f(x)"), ["(call f x)"]);
}

// precedence

#[test]
fn precedence_full_ladder() {
    assert_eq!(
        expr("a || b && c == d < e + f * g"),
        "(|| a (&& b (== c (< d (+ e (* f g))))))"
    );
}

#[test]
fn precedence_unary_operators() {
    assert_eq!(expr("!a && -b"), "(&& (! a) (neg b))");
}

/// `docs/slice.md` 29.1: shifts bind looser than `+`, then `&`, `^`, `|`,
/// all tighter than the comparisons, as in Rust.
#[test]
fn precedence_bitwise_ladder() {
    assert_eq!(
        expr("a == b | c ^ d & e << f + g"),
        "(== a (| b (^ c (& d (<< e (+ f g))))))"
    );
    assert_eq!(expr("a >> b - c"), "(>> a (- b c))");
    assert_eq!(expr("a & b == c"), "(== (& a b) c)");
    assert_eq!(expr("a | b < c"), "(< (| a b) c)");
}

#[test]
fn bitwise_operators_are_left_associative() {
    assert_eq!(expr("a << b >> c"), "(>> (<< a b) c)");
    assert_eq!(expr("a | b | c"), "(| (| a b) c)");
    assert_eq!(expr("a ^ b ^ c"), "(^ (^ a b) c)");
    assert_eq!(expr("a & b & c"), "(& (& a b) c)");
}

#[test]
fn bitwise_not_is_a_prefix_operator() {
    assert_eq!(expr("~a.b & ~-c"), "(& (~ (. a b)) (~ (neg c)))");
}

/// `>>` and `<<` are two touching `>` or `<`; with a space between them
/// they are no shift.
#[test]
fn shifts_need_touching_angles() {
    assert_eq!(expr("a>>b"), "(>> a b)");
    assert_eq!(expr("a<<b"), "(<< a b)");
    assert_eq!(errors("fn f() {\na > > b\n}"), ["E001@2:5"]);
    assert_eq!(expr("a >> b >= c"), "(>= (>> a b) c)");
    assert_eq!(
        shown("let x: List<List<Int>> = y >> 1"),
        ["(let x: List<List<Int>> (>> y 1))"]
    );
}

/// A closure and an or-pattern still read `|` as before.
#[test]
fn pipe_stays_a_closure_and_a_pattern_separator() {
    assert_eq!(expr("f(|x| x | 1)"), "(call f (closure [x] (| x 1)))");
    assert_eq!(
        expr("match a {\n  1 | 2 => b | c\n}"),
        "(match a ((| 1 2) => (| b c)))"
    );
}

#[test]
fn newline_after_a_bitwise_operator_continues() {
    assert_eq!(shown("let x = a |\n  b ^\n  c"), ["(let x (| a (^ b c)))"]);
    assert_eq!(shown("let y = a <<\n  b"), ["(let y (<< a b))"]);
}

#[test]
fn additive_is_left_associative() {
    assert_eq!(expr("a - b - c"), "(- (- a b) c)");
    assert_eq!(expr("a / b % c"), "(% (/ a b) c)");
}

#[test]
fn parentheses_produce_no_node() {
    assert_eq!(expr("(a + b) * c"), "(* (+ a b) c)");
}

// postfix

#[test]
fn postfix_chain() {
    assert_eq!(expr("a.b(c)[d]?.e"), "(. (? (index (method a b c) d)) e)");
}

#[test]
fn call_on_a_call() {
    assert_eq!(expr("f(x)(y)"), "(call (call f x) y)");
}

#[test]
fn path_calls() {
    assert_eq!(expr("Action.Continue"), "(. Action Continue)");
    assert_eq!(expr("Bytes.empty()"), "(method Bytes empty)");
}

// newlines

#[test]
fn newline_after_binary_operator_continues() {
    assert_eq!(shown("let x = a +\n  b"), ["(let x (+ a b))"]);
    assert_eq!(shown("let y = a &&\n\n  b"), ["(let y (&& a b))"]);
}

#[test]
fn newline_after_comma_continues() {
    assert_eq!(
        shown("let r = Reply { a = 1,\n  b = 2 }"),
        ["(let r (record Reply (a 1) (b 2)))"]
    );
    let program = program("enum E { A,\n  B }");
    let Decl::Enum(e) = &program.decls[0] else {
        panic!("enum")
    };
    assert_eq!(e.variants.len(), 2);
}

#[test]
fn newline_after_assign_continues() {
    assert_eq!(shown("let x =\n  1"), ["(let x 1)"]);
    assert_eq!(shown("x =\n  1"), ["(= x 1)"]);
}

#[test]
fn newlines_inside_parentheses_are_skipped() {
    assert_eq!(shown("f(\n  a\n  ,\n  b\n)"), ["(call f a b)"]);
    assert_eq!(shown("let x = (a\n  + b)"), ["(let x (+ a b))"]);
}

#[test]
fn newlines_inside_brackets_are_skipped() {
    assert_eq!(shown("let x = [\n  1\n  , 2\n]"), ["(let x (list 1 2))"]);
}

#[test]
fn postfix_paren_on_next_line_starts_a_statement() {
    assert_eq!(shown("a\n(b)"), ["a", "b"]);
}

#[test]
fn statement_cannot_start_with_dot() {
    assert_eq!(errors("fn f() {\n  a\n  .b()\n}"), ["E001@3:3"]);
}

#[test]
fn else_on_next_line_is_e001() {
    assert_eq!(
        errors("fn f() {\n  if a { 1 }\n  else { 2 }\n}"),
        ["E001@3:3"]
    );
}

#[test]
fn newline_before_binary_operator_ends_the_statement() {
    assert_eq!(shown("a\n- b"), ["a", "(neg b)"]);
}

// record literals

#[test]
fn record_literal_with_commas() {
    assert_eq!(expr("Reply { a = 1, b = 2 }"), "(record Reply (a 1) (b 2))");
}

#[test]
fn record_literal_with_newlines() {
    assert_eq!(
        expr("Reply {\n  a = 1\n  b = 2\n}"),
        "(record Reply (a 1) (b 2))"
    );
}

#[test]
fn lowercase_name_before_brace_is_not_a_record() {
    assert_eq!(expr("if x { 1 }"), "(if x {1})");
    assert_eq!(shown("while x { 1 }"), ["(while x {1})"]);
    assert_eq!(expr("match x { _ => 1 }"), "(match x (_ => 1))");
}

// match

#[test]
fn match_arms_separated_by_newlines() {
    assert_eq!(
        expr("match c {\n  A => 1\n  B => 2\n}"),
        "(match c (A => 1) (B => 2))"
    );
}

#[test]
fn match_arms_separated_by_commas() {
    assert_eq!(
        expr("match c { A => 1, B => 2, }"),
        "(match c (A => 1) (B => 2))"
    );
}

#[test]
fn match_block_and_expression_bodies() {
    assert_eq!(
        expr("match c {\n  A => { f()\n 1 }\n  B => 2\n}"),
        "(match c (A => {(call f); 1}) (B => 2))"
    );
}

#[test]
fn match_literal_and_wildcard_patterns() {
    assert_eq!(
        expr("match c { 1 => a, \"x\" => b, _ => d }"),
        "(match c (1 => a) (\"x\" => b) (_ => d))"
    );
}

#[test]
fn match_binding_pattern() {
    assert_eq!(
        expr("match c { other => other }"),
        "(match c ($other => other))"
    );
}

#[test]
fn match_nested_patterns() {
    assert_eq!(
        expr("match c { Some(Echo(t)) => t, None => d }"),
        "(match c (Some(Echo($t)) => t) (None => d))"
    );
}

#[test]
fn match_qualified_pattern() {
    let stmts = stmts("match c { Command.Echo(t) => t }");
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression statement")
    };
    let ExprKind::Match { arms, .. } = &e.kind else {
        panic!("match")
    };
    let PatternKind::Variant { path, args } = &arms[0].pattern.kind else {
        panic!("variant pattern")
    };
    let names: Vec<&str> = path.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Command", "Echo"]);
    assert_eq!(args.len(), 1);
}

// if

#[test]
fn if_else_if_else_nests() {
    assert_eq!(
        expr("if a { 1 } else if b { 2 } else { 3 }"),
        "(if a {1} (if b {2} {3}))"
    );
    let stmts = stmts("if a { 1 } else if b { 2 } else { 3 }");
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression statement")
    };
    let ExprKind::If {
        otherwise: Some(nested),
        ..
    } = &e.kind
    else {
        panic!("if with else")
    };
    let ExprKind::If {
        otherwise: Some(last),
        ..
    } = &nested.kind
    else {
        panic!("else if nests an If")
    };
    assert!(matches!(last.kind, ExprKind::Block(_)));
}

#[test]
fn else_if_chain_across_lines() {
    assert_eq!(
        expr("if a {\n  1\n} else if b {\n  2\n} else {\n  3\n}"),
        "(if a {1} (if b {2} {3}))"
    );
}

#[test]
fn if_as_let_initializer() {
    assert_eq!(
        shown("let x = if a { 1 } else { 2 }"),
        ["(let x (if a {1} {2}))"]
    );
}

// separators

#[test]
fn semicolon_separates_statements() {
    assert_eq!(shown("a; b"), ["a", "b"]);
    assert_eq!(
        shown("let x = 1; x = 2;; f(x)"),
        ["(let x 1)", "(= x 2)", "(call f x)"]
    );
}

#[test]
fn trailing_semicolon() {
    assert_eq!(shown("a;"), ["a"]);
    assert_eq!(shown("a;\nb;"), ["a", "b"]);
}

#[test]
fn two_statements_on_one_line_need_a_separator() {
    assert_eq!(errors("fn f() {\n  a b\n}"), ["E001@2:5"]);
}

// comments

#[test]
fn comments_everywhere() {
    let text = "// leading\npackage p // after package\n\n// own line\nfn f() { // after brace\n  // inside block\n  let r = Reply { // after brace\n    // inside literal\n    a = 1 // after field\n    // before brace\n  }\n  r // tail\n  // last\n}\n// trailing";
    let program = program(text);
    assert_eq!(
        program.package.as_ref().map(|p| p.dotted()).as_deref(),
        Some("p")
    );
    let Decl::Fn(f) = &program.decls[0] else {
        panic!("fn")
    };
    let shown: Vec<String> = f.body.stmts.iter().map(show_stmt).collect();
    assert_eq!(shown, ["(let r (record Reply (a 1)))", "r"]);
}

// literals

#[test]
fn string_escapes() {
    assert_eq!(
        shown(r#"let s = "a\n\r\t\0\\\"\x41""#),
        [format!("(let s {:?})", "a\n\r\t\0\\\"A")]
    );
}

#[test]
fn vertical_tab_and_form_feed_escapes() {
    assert_eq!(expr(r#""a\vb\fc""#), format!("{:?}", "a\x0Bb\x0Cc"));
    assert_eq!(expr(r"b'\v'"), "11");
    assert_eq!(expr(r"b'\f'"), "12");
    assert_eq!(expr(r#"b"\v\f""#), "(method Bytes of (list 11 12))");
}

#[test]
fn hex_escape_produces_character() {
    assert_eq!(expr(r#""\x41""#), "\"A\"");
}

#[test]
fn unknown_escape_is_e001() {
    assert_eq!(errors("fn f() {\n  \"a\\q\"\n}"), ["E001@2:5"]);
}

#[test]
fn hex_integer() {
    assert_eq!(expr("0xFF"), "255");
}

#[test]
fn integer_with_separators() {
    assert_eq!(expr("1_000"), "1000");
}

#[test]
fn largest_integer_is_accepted() {
    assert_eq!(expr("9223372036854775807"), "9223372036854775807");
}

#[test]
fn integer_beyond_i64_is_e001() {
    assert_eq!(errors("fn f() {\n  9223372036854775808\n}"), ["E001@2:3"]);
}

// errors

#[test]
fn unterminated_string_is_e001() {
    assert_eq!(errors("fn f() {\n  \"abc\n}"), ["E001@2:3"]);
    assert_eq!(errors("fn f() {\n  \"abc"), ["E001@2:3"]);
}

#[test]
fn eof_inside_block_is_e002() {
    assert_eq!(errors("fn f() {\n  let x = 1\n"), ["E002@3:1"]);
}

#[test]
fn chained_equality_is_e001() {
    assert_eq!(errors("fn f() {\n  a == b == c\n}"), ["E001@2:10"]);
}

#[test]
fn chained_comparison_is_e001() {
    assert_eq!(errors("fn f() {\n  a < b < c\n}"), ["E001@2:9"]);
}

#[test]
fn recovery_reports_each_bad_declaration_once() {
    let text = "fn bad() {\n  let = 1\n  more tokens here\n}\n\nenum Good { A, B }\n\ntype Bad {\n  x Int\n}\n";
    assert_eq!(errors(text), ["E001@2:7", "E001@9:5"]);
}

#[test]
fn recovery_keeps_good_declarations_parseable() {
    let good = "enum Good { A, B }\n";
    program(good);
    let text = format!("fn bad( {{\n}}\n{good}fn worse() {{ ) }}\n");
    assert_eq!(errors(&text), ["E001@1:9", "E001@4:14"]);
}

#[test]
fn declarations_need_a_newline_between_them() {
    assert_eq!(errors("enum A { X } enum B { Y }"), ["E001@1:14"]);
}

#[test]
fn import_forms() {
    let program = program("import a.b.{X, Y}\nimport a.b.C");
    assert_eq!(program.imports[0].path.dotted(), "a.b");
    assert_eq!(program.imports[0].names.len(), 2);
    assert_eq!(program.imports[1].path.dotted(), "a.b.C");
    assert!(program.imports[1].names.is_empty());
}

#[test]
fn unary_neg_on_literal() {
    let stmts = stmts("-1");
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression")
    };
    assert!(matches!(
        e.kind,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn spans_cover_first_to_last_token() {
    let text = "fn f() {\n  (x)?\n}";
    let stmts = stmts_of(text);
    let start = text.find('(').expect("paren");
    let start = text[start + 1..].find('(').expect("second paren") + start + 1;
    let end = text.find('?').expect("question") + 1;
    assert_eq!((stmts[0].span.start, stmts[0].span.end), (start, end));
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression")
    };
    assert_eq!((e.span.start, e.span.end), (start, end));
}

fn stmts_of(text: &str) -> Vec<Stmt> {
    match program(text).decls.into_iter().next() {
        Some(Decl::Fn(f)) => f.body.stmts,
        other => panic!("expected a fn, got {other:?}"),
    }
}

// function types

/// The type of the single field of `type R { f: <ty> }`.
fn field_type(ty: &str) -> Type {
    let program = program(&format!("type R {{ f: {ty} }}"));
    match program.decls.into_iter().next() {
        Some(Decl::Record(r)) => r.fields.into_iter().next().expect("one field").ty,
        other => panic!("expected a record, got {other:?}"),
    }
}

#[test]
fn fn_type_lists_params_then_return() {
    let ty = field_type("fn(Text, Int) -> Reply");
    assert_eq!(show_type(&ty), "fn<Text, Int, Reply>");
    assert!(ty.is_fn());
    let params: Vec<String> = ty.fn_params().iter().map(show_type).collect();
    assert_eq!(params, ["Text", "Int"]);
    assert_eq!(ty.fn_ret().map(show_type).as_deref(), Some("Reply"));
}

#[test]
fn fn_type_without_return_is_unit() {
    let ty = field_type("fn()");
    assert_eq!(show_type(&ty), "fn<Unit>");
    assert!(ty.fn_params().is_empty());
    let text = "type R { f: fn(Int) }";
    let program = program(text);
    let Decl::Record(r) = &program.decls[0] else {
        panic!("record")
    };
    let ty = &r.fields[0].ty;
    let start = text.find("fn(").expect("fn type");
    assert_eq!(
        (ty.span.start, ty.span.end),
        (start, start + "fn(Int)".len())
    );
}

#[test]
fn fn_type_nests() {
    assert_eq!(
        show_type(&field_type("List<fn(Text) -> Option<Int>>")),
        "List<fn<Text, Option<Int>>>"
    );
    assert_eq!(
        show_type(&field_type("fn(fn(Int) -> Int) -> fn() -> Bool")),
        "fn<fn<Int, Int>, fn<Bool>>"
    );
}

#[test]
fn fn_type_in_param_and_return_position() {
    let program = program("fn apply(f: fn(Int) -> Int, x: Int) -> fn(Int) -> Int {\n  f\n}");
    let Decl::Fn(f) = &program.decls[0] else {
        panic!("fn")
    };
    assert_eq!(show_type(&f.params[0].ty), "fn<Int, Int>");
    assert_eq!(
        f.ret.as_ref().map(show_type).as_deref(),
        Some("fn<Int, Int>")
    );
    assert_eq!(
        shown("let g: fn(Text) = log"),
        ["(let g: fn<Text, Unit> log)"]
    );
}

#[test]
fn fn_type_missing_paren_is_e001() {
    assert_eq!(errors("type R { f: fn Int }"), ["E001@1:16"]);
}

// protocols

#[test]
fn protocol_declares_roles() {
    let text = "protocol Echo {\n\n  fn serve(stream: TcpStream, app: App) -> Result<Unit, IoError> requires [net] {\n    return Ok(Unit)\n  }\n\n  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [net] {\n    return Ok(Unit)\n  }\n}\n";
    let program = program(text);
    let Decl::Protocol(protocol) = &program.decls[0] else {
        panic!("protocol")
    };
    assert_eq!(protocol.name.name, "Echo");
    let roles: Vec<&str> = protocol.fns.iter().map(|f| f.name.name.as_str()).collect();
    assert_eq!(roles, ["serve", "connect"]);
    assert_eq!(protocol.role("serve").map(|f| f.params.len()), Some(2));
    assert!(protocol.fns.iter().all(|f| f.is_effectful()));
    assert_eq!(program.decls[0].name().name, "Echo");
    assert_eq!(
        protocol.span,
        ignite_syntax::ast::Span::in_file(
            0,
            text.trim_end().len(),
            SourceFile::new("t.ignite", "").id()
        )
    );
}

#[test]
fn role_requires_breaks_across_lines() {
    let text = "protocol P {\n  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [\n    stream.read,\n    stream.write,\n    stdin.read\n  ] {\n    return Ok(Unit)\n  }\n  fn serve(stream: TcpStream, a: A) -> Result<Unit, IoError> requires [stream.read,\n      stream.write] = Ok(Unit)\n}\n";
    let program = program(text);
    let Decl::Protocol(p) = &program.decls[0] else {
        panic!("protocol")
    };
    let effects: Vec<Vec<String>> = p
        .fns
        .iter()
        .map(|f| f.requires.iter().flatten().map(|e| e.dotted()).collect())
        .collect();
    assert_eq!(
        effects,
        [
            vec!["stream.read", "stream.write", "stdin.read"],
            vec!["stream.read", "stream.write"]
        ]
    );
}

#[test]
fn protocol_may_be_empty_and_on_one_line() {
    let program =
        program("protocol P {}\nprotocol Q { fn serve(s: TcpStream, a: A) requires [] { } }");
    let Decl::Protocol(p) = &program.decls[0] else {
        panic!("protocol")
    };
    assert!(p.fns.is_empty());
    let Decl::Protocol(q) = &program.decls[1] else {
        panic!("protocol")
    };
    assert_eq!(q.fns.len(), 1);
}

#[test]
fn protocol_member_must_be_fn() {
    assert_eq!(errors("protocol P {\n  let x = 1\n}"), ["E001@2:3"]);
    assert_eq!(errors("protocol P {\n  fn serve() {}\n"), ["E002@3:1"]);
}

#[test]
fn recovery_resumes_at_protocol() {
    let text = "fn bad( {\n}\nprotocol P {\n  fn serve() requires [] {\n    return\n  }\n}\nenum E { A }\n";
    assert_eq!(errors(text), ["E001@1:9"]);
}

#[test]
fn recovery_skips_the_rest_of_a_broken_protocol() {
    let text = "protocol P {\n  fn serve() requires [] {\n    let = 1\n  }\n  fn connect() requires [] {}\n}\nenum E { A }\nfn bad( {\n}\n";
    assert_eq!(errors(text), ["E001@3:9", "E001@8:9"]);
}

#[test]
fn keywords_are_path_segments() {
    let program = program(
        "package std.protocol.telnet\nimport std.protocol.telnet.{Telnet}\nimport std.protocol.http.Http\nfn f() requires [net.spawn] {}",
    );
    assert_eq!(
        program.package.as_ref().map(|p| p.dotted()).as_deref(),
        Some("std.protocol.telnet")
    );
    assert_eq!(program.imports[0].path.dotted(), "std.protocol.telnet");
    assert_eq!(program.imports[1].path.dotted(), "std.protocol.http.Http");
    let Decl::Fn(f) = &program.decls[0] else {
        panic!("fn")
    };
    let effects: Vec<String> = f.requires.iter().flatten().map(|p| p.dotted()).collect();
    assert_eq!(effects, ["net.spawn"]);
    assert_eq!(errors("import std.{protocol}"), ["E001@1:13"]);
}

#[test]
fn keywords_are_property_names() {
    let program = program(
        "system S {\n  component server: Service {\n    protocol = Telnet\n    application = Console { prompt = \"> \", respond = execute }\n  }\n}",
    );
    let Decl::System(system) = &program.decls[0] else {
        panic!("system")
    };
    let properties = &system.components[0].properties;
    assert_eq!(properties[0].name.name, "protocol");
    assert!(matches!(&properties[0].value.kind, ExprKind::Name(n) if n.name == "Telnet"));
    assert_eq!(
        show(&properties[1].value),
        "(record Console (prompt \"> \") (respond execute))"
    );
    assert_eq!(expr("R { spawn = 1 }"), "(record R (spawn 1))");
}

#[test]
fn protocol_and_spawn_are_keywords() {
    assert_eq!(errors("fn f() {\n  let spawn = 1\n}"), ["E001@2:7"]);
    assert_eq!(errors("fn protocol() {}"), ["E001@1:4"]);
}

// spawn

#[test]
fn spawn_is_a_prefix_operator() {
    assert_eq!(expr("spawn f(a, b)"), "(spawn (call f a b))");
    assert_eq!(
        expr("spawn Telnet.serve(s, app)"),
        "(spawn (method Telnet serve s app))"
    );
    assert_eq!(expr("spawn f()?"), "(spawn (? (call f)))");
    assert_eq!(expr("!spawn f()"), "(! (spawn (call f)))");
    assert_eq!(expr("spawn f() + 1"), "(+ (spawn (call f)) 1)");
}

#[test]
fn spawn_span_covers_operand() {
    let text = "fn f() {\n  spawn g(x)\n}";
    let stmts = stmts_of(text);
    let start = text.find("spawn").expect("spawn");
    let end = text.find("x)").expect("arg") + 2;
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression")
    };
    assert!(matches!(e.kind, ExprKind::Spawn(_)));
    assert_eq!((e.span.start, e.span.end), (start, end));
}

#[test]
fn spawn_without_operand_is_an_error() {
    assert_eq!(errors("fn f() {\n  spawn\n}"), ["E001@2:8"]);
}

// const

/// `(const NAME Type value)` for every constant of `text`.
fn consts(text: &str) -> Vec<String> {
    program(text)
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Const(c) => Some(format!(
                "(const {} {} {})",
                c.name.name,
                show_type(&c.ty),
                show(&c.value)
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn const_declarations() {
    assert_eq!(
        consts(
            "const MAXIMUM_LINE: Int = 4096\nconst PROMPT: Text =\n  \"> \"\nconst LIMITS: List<Int> = [1, LIMIT * 2]\nconst ZERO: Point = Point { x = 0, y = 0 }"
        ),
        [
            "(const MAXIMUM_LINE Int 4096)",
            "(const PROMPT Text \"> \")",
            "(const LIMITS List<Int> (list 1 (* LIMIT 2)))",
            "(const ZERO Point (record Point (x 0) (y 0)))",
        ]
    );
}

#[test]
fn const_span_covers_keyword_to_value() {
    let text = "const N: Int = f(1)\n";
    let Decl::Const(c) = &program(text).decls[0] else {
        panic!("const")
    };
    assert_eq!((c.span.start, c.span.end), (0, text.len() - 1));
    assert_eq!(c.name.span.start, 6);
}

#[test]
fn const_needs_type_and_value() {
    assert_eq!(errors("const N = 1"), ["E001@1:9"]);
    assert_eq!(errors("const N: Int"), ["E002@1:13"]);
    assert_eq!(errors("const N: Int = 1 fn f() {}"), ["E001@1:18"]);
}

#[test]
fn const_is_a_keyword_and_a_recovery_point() {
    assert_eq!(errors("fn f() {\n  let const = 1\n}"), ["E001@2:7"]);
    let text = "fn bad( {\n}\nconst N: Int = 1\ntype Bad { x Int }\n";
    assert_eq!(errors(text), ["E001@1:9", "E001@4:14"]);
}

// record literals in conditions

#[test]
fn no_record_literal_in_conditions() {
    assert_eq!(
        shown("while i < LIMIT { i = i + 1 }"),
        ["(while (< i LIMIT) {(= i (+ i 1))})"]
    );
    assert_eq!(expr("if mode == MODE { 1 }"), "(if (== mode MODE) {1})");
    assert_eq!(expr("if READY { 1 } else { 2 }"), "(if READY {1} {2})");
    assert_eq!(
        expr("match COMMANDS { _ => 1 }"),
        "(match COMMANDS (_ => 1))"
    );
    assert_eq!(
        shown("for x in ITEMS { f(x) }"),
        ["(for x ITEMS {(call f x)})"]
    );
}

#[test]
fn record_literal_in_brackets_inside_a_condition() {
    assert_eq!(
        expr("if (R { a = 1 }) == r { 1 }"),
        "(if (== (record R (a 1)) r) {1})"
    );
    assert_eq!(
        expr("if f(R { a = 1 }) { 1 }"),
        "(if (call f (record R (a 1))) {1})"
    );
    assert_eq!(
        shown("for r in [R { a = 1 }] { r }"),
        ["(for r (list (record R (a 1))) {r})"]
    );
    assert_eq!(
        expr("match (R { a = 1 }) { _ => 1 }"),
        "(match (record R (a 1)) (_ => 1))"
    );
}

#[test]
fn record_literal_allowed_in_bodies_and_after_conditions() {
    assert_eq!(
        expr("if c { R { a = 1 } } else { R { a = 2 } }"),
        "(if c {(record R (a 1))} {(record R (a 2))})"
    );
    assert_eq!(
        expr("match c { _ => R { a = 1 } }"),
        "(match c (_ => (record R (a 1))))"
    );
    assert_eq!(
        shown("while c { break }\nR { a = 1 }"),
        ["(while c {(break)})", "(record R (a 1))"]
    );
    assert_eq!(shown("let r = R { a = 1 }"), ["(let r (record R (a 1)))"]);
}

#[test]
fn record_literal_directly_in_a_condition_is_not_parsed() {
    assert_eq!(
        errors("fn f() {\n  if x == R { a = 1 } { 1 }\n}"),
        ["E001@2:23"]
    );
}

// qualified variant patterns

#[test]
fn qualified_patterns_with_payload() {
    assert_eq!(
        expr(
            "match c { Command.Echo(text) => text, Input.Data(Option.Some(b)) => b, Mode.Raw => 1, Result.Err(_) => 2, other => 3 }"
        ),
        "(match c (Command.Echo($text) => text) (Input.Data(Option.Some($b)) => b) (Mode.Raw => 1) (Result.Err(_) => 2) ($other => 3))"
    );
}

#[test]
fn lowercase_name_with_payload_or_path_is_a_variant_pattern() {
    assert_eq!(
        expr("match c { echo(t) => t, a.B => 1 }"),
        "(match c (echo($t) => t) (a.B => 1))"
    );
}

// slice 6 surface syntax (`docs/slice.md` section 10)

fn decls(text: &str) -> Vec<Decl> {
    program(text).decls
}

/// The message of the single diagnostic `text` produces.
fn message(text: &str) -> String {
    let source = SourceFile::new("t.ignite", text);
    match parse(&source) {
        Ok(program) => panic!("expected diagnostics, parsed {program:?}"),
        Err(diagnostics) => {
            assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
            diagnostics[0].message.clone()
        }
    }
}

#[test]
fn expression_body() {
    let Decl::Fn(f) = &decls("fn f(x: Int) -> Int = x + 1")[0] else {
        panic!("fn")
    };
    assert_eq!(show_block(&f.body), "{(+ x 1)}");
    assert_eq!(f.body.span, f.body.stmts[0].span);
    let Decl::Fn(g) = &decls("fn g(s: TcpStream) -> Int requires [s.read] =\n  s.len()")[0] else {
        panic!("fn")
    };
    assert!(g.requires.is_some());
    assert_eq!(show_block(&g.body), "{(method s len)}");
}

#[test]
fn tuple_values_and_unit() {
    assert_eq!(expr("(a, b)"), "(tuple a b)");
    assert_eq!(expr("(a, (b, c), d,)"), "(tuple a (tuple b c) d)");
    assert_eq!(expr("()"), "(tuple)");
    assert_eq!(expr("(a)"), "a");
    assert_eq!(errors("fn f() {\n  (a,)\n}"), ["E001@2:3"]);
}

#[test]
fn tuple_types() {
    assert_eq!(
        shown("let p: (Int, List<Text>) = (1, [])"),
        ["(let p: Tuple<Int, List<Text>> (tuple 1 (list)))"]
    );
    assert_eq!(shown("let u: () = ()"), ["(let u: Unit (tuple))"]);
    assert_eq!(shown("let n: (Int) = 1"), ["(let n: Int 1)"]);
    let Decl::Fn(f) = &decls("fn f() -> (Int, Bool) = (1, true)")[0] else {
        panic!("fn")
    };
    assert!(f.ret.as_ref().is_some_and(Type::is_tuple));
}

#[test]
fn let_patterns() {
    assert_eq!(shown("let (a, b) = p"), ["(let ($a, $b) p)"]);
    assert_eq!(shown("var (a, (_, c)) = p"), ["(var ($a, (_, $c)) p)"]);
    assert_eq!(shown("let _ = f()"), ["(let _ (call f))"]);
    assert_eq!(shown("_ = f()"), ["(let _ (call f))"]);
    assert_eq!(shown("let x = 1"), ["(let x 1)"]);
    assert_eq!(errors("fn f() {\n  let (a, 1) = p\n}"), ["E001@2:7"]);
    assert_eq!(errors("fn f() {\n  let [a] = p\n}"), ["E001@2:7"]);
}

#[test]
fn bool_tuple_list_and_or_patterns() {
    assert_eq!(
        expr("match x { true => 1, false => 0 }"),
        "(match x (true => 1) (false => 0))"
    );
    assert_eq!(
        expr("match p { (0, b) => b, (a, _) => a }"),
        "(match p ((0, $b) => b) (($a, _) => a))"
    );
    assert_eq!(
        expr("match l { [] => 0, [a] => a, [a, b] => b, _ => 9 }"),
        "(match l ([] => 0) ([$a] => a) ([$a, $b] => b) (_ => 9))"
    );
    assert_eq!(
        expr("match b { 1 | 2 |\n 3 => 0, Some(4 | 5) => 1, _ => 2 }"),
        "(match b ((| 1 2 3) => 0) (Some((| 4 5)) => 1) (_ => 2))"
    );
    assert_eq!(expr("match u { () => 0 }"), "(match u (() => 0))");
    assert_eq!(
        expr("match l { [..] => 0, [f, ..r] => f, [.., z] => z, [a, .., [b, ..]] => a }"),
        "(match l ([..] => 0) ([$f, ..$r] => f) ([.., $z] => z) ([$a, .., [$b, ..]] => a))"
    );
    assert_eq!(
        expr("match b { IAC | SE => 0, _ => 1 }"),
        "(match b ((| IAC SE) => 0) (_ => 1))"
    );
}

#[test]
fn field_defaults_and_aliases() {
    let ds = decls(
        "type Reply {\n  lines: List<Text>\n  action: Action = Continue\n}\ntype IoResult<T> = Result<T, IoError>\ntype Handler = fn(Text) -> Reply",
    );
    let Decl::Record(reply) = &ds[0] else {
        panic!("record")
    };
    assert!(reply.fields[0].default.is_none());
    assert_eq!(
        reply.fields[1].default.as_ref().map(show),
        Some("Continue".to_string())
    );
    let Decl::Alias(io) = &ds[1] else {
        panic!("alias")
    };
    assert_eq!(io.name.name, "IoResult");
    assert_eq!(io.params.len(), 1);
    assert_eq!(show_type(&io.ty), "Result<T, IoError>");
    let Decl::Alias(handler) = &ds[2] else {
        panic!("alias")
    };
    assert!(handler.params.is_empty() && handler.ty.is_fn());
}

#[test]
fn field_punning_and_commas() {
    assert_eq!(
        expr("Reply { lines, action = Close }"),
        "(record Reply (lines lines) (action Close))"
    );
    assert_eq!(expr("Reply { lines }"), "(record Reply (lines lines))");
    let Decl::Record(r) = &decls("type P { x: Int, y: Int = 0 }")[0] else {
        panic!("record")
    };
    assert_eq!(r.fields.len(), 2);
}

#[test]
fn interpolation_desugars_to_concatenation() {
    assert_eq!(
        expr("\"too long: {n} bytes\""),
        "(+ (+ \"too long: \" (method n to_text)) \" bytes\")"
    );
    assert_eq!(expr("\"{n}\""), "(method n to_text)");
    assert_eq!(
        expr("\"{a}{b.len()}\""),
        "(+ (method a to_text) (method (method b len) to_text))"
    );
    assert_eq!(expr("\"{{x}}\""), "\"{x}\"");
    assert_eq!(
        expr("\"{ R { a = 1 }.a } \""),
        "(+ (method (. (record R (a 1)) a) to_text) \" \")"
    );
}

#[test]
fn interpolated_expression_spans_point_into_the_string() {
    let text = "fn f() {\n  \"n={count}\"\n}";
    let stmts = stmts_of(text);
    let StmtKind::Expr(e) = &stmts[0].kind else {
        panic!("expression")
    };
    let ExprKind::Binary { right, .. } = &e.kind else {
        panic!("binary")
    };
    let ExprKind::Method { receiver, .. } = &right.kind else {
        panic!("method")
    };
    let start = text.find("count").expect("count");
    assert_eq!((receiver.span.start, receiver.span.end), (start, start + 5));
}

#[test]
fn interpolation_errors() {
    assert_eq!(
        message("const A: Text = \"x}\""),
        "unmatched `}` in string; write `}}` for a literal brace"
    );
    assert_eq!(
        message("const A: Text = \"{x\""),
        "unclosed `{` in interpolation; write `{{` for a literal brace"
    );
    assert_eq!(
        message("const A: Text = \"{x\n\""),
        "unclosed `{` in interpolation; write `{{` for a literal brace"
    );
    assert_eq!(
        message("const A: Text = \"{ }\""),
        "empty `{}` in string; write `{{}}` for literal braces"
    );
    assert_eq!(
        message("const A: Text = \"{a +}\""),
        "unexpected `}` in string, expected expression"
    );
    assert_eq!(errors("const A: Text = \"{a b}\""), ["E001@1:21"]);
}

// slice 9 (`docs/slice.md` section 13)

#[test]
fn interpolation_holds_string_literals() {
    assert_eq!(
        expr("\"a{f(\"x\")}b\""),
        expr("\"a\" + f(\"x\").to_text() + \"b\"")
    );
    assert_eq!(
        expr("\"{\"in {r\"raw}\"}\"}\""),
        expr("(\"in \" + \"raw}}\".to_text()).to_text()")
    );
    let deep = (0..40).fold("x".to_string(), |inner, _| format!("\"{{{inner}}}\""));
    assert!(
        errors(&format!("const A: Text = {deep}"))
            .iter()
            .any(|e| e.starts_with("E054"))
    );
}

#[test]
fn raw_strings_and_unicode_escapes() {
    assert_eq!(expr("r\"a\\n{b}\""), expr("\"a\\\\n{{b}}\""));
    assert_eq!(expr("r#\"say \"hi\"\"#"), expr("\"say \\\"hi\\\"\""));
    assert_eq!(expr("br\"ab\""), expr("b\"ab\""));
    assert_eq!(expr("\"\\u{48}\\u{1F600}\""), expr("\"H\u{1F600}\""));
    assert_eq!(errors("const A: Text = \"\\u{D800}\""), ["E001@1:18"]);
    assert_eq!(errors("const A: Text = \"\\u{}\""), ["E001@1:18"]);
}

#[test]
fn closures() {
    assert_eq!(expr("|x| x + 1"), "(closure [x] (+ x 1))");
    assert_eq!(expr("|a: Int, b| a"), "(closure [a: Int, b] a)");
    assert_eq!(expr("|| 1"), "(closure [] 1)");
    assert_eq!(
        expr("xs.map(|x| x * 2)"),
        "(method xs map (closure [x] (* x 2)))"
    );
    assert!(expr("|x| { let y = x\n y }").starts_with("(closure [x] {"));
}

#[test]
fn record_update() {
    assert_eq!(
        expr("Todo { done = true, ..t }"),
        "(update Todo (done true) ..t)"
    );
    assert_eq!(expr("Todo {\n ..t\n}"), "(update Todo ..t)");
    assert_eq!(errors("fn f() { T { ..t, a = 1 } }"), ["E001@1:19"]);
}

#[test]
fn tuple_fields() {
    assert_eq!(expr("t.0"), "(. t 0)");
    assert_eq!(expr("t.1.0"), "(. (. t 1) 0)");
    assert_eq!(expr("f().0.len()"), "(method (. (call f) 0) len)");
}

#[test]
fn break_and_continue_are_expressions() {
    assert_eq!(
        expr("match x { None => break, Some(v) => continue }"),
        "(match x (None => (break)) (Some($v) => (continue)))"
    );
}

#[test]
fn range_patterns() {
    assert_eq!(
        expr("match b { b'0'..=b'9' => 1, -5..=-1 => 2, LO..=HI => 3, _ => 4 }"),
        "(match b (48..=57 => 1) (-5..=-1 => 2) (LO..=HI => 3) (_ => 4))"
    );
    assert_eq!(errors("fn f() { match b { 1..2 => 1 } }"), ["E001@1:21"]);
}

#[test]
fn glob_import() {
    let program = program("import std.text.*\nimport a.{b}");
    assert!(program.imports[0].is_glob());
    assert_eq!(program.imports[0].module(), "std.text");
    assert!(program.imports[0].names.is_empty());
    assert!(!program.imports[1].is_glob());
}

#[test]
fn associated_fn() {
    match decls("fn Response.json(status: Int) -> Int = status").remove(0) {
        Decl::Fn(f) => {
            assert_eq!(f.owner.map(|o| o.name), Some("Response".to_string()));
            assert_eq!(f.name.name, "json");
        }
        other => panic!("expected a fn, got {other:?}"),
    }
}

#[test]
fn byte_literals() {
    assert_eq!(expr("b' '"), "32");
    assert_eq!(expr("b'\\n'"), "10");
    assert_eq!(expr("b'\\\\'"), "92");
    assert_eq!(expr("b'\\''"), "39");
    assert_eq!(expr("b'\\xFF'"), "255");
    assert_eq!(expr("b'\"'"), "34");
    assert_eq!(expr("b\"\\r\\n\""), "(method Bytes of (list 13 10))");
    assert_eq!(expr("b\"\""), "(method Bytes of (list))");
    assert_eq!(expr("b\"a\\xFF\""), "(method Bytes of (list 97 255))");
    assert_eq!(
        expr("match c { b'\\r' => 1, _ => 0 }"),
        "(match c (13 => 1) (_ => 0))"
    );
    assert_eq!(expr("b"), "b");
    assert_eq!(errors("fn f() {\n  b'ab'\n}"), ["E001@2:3"]);
    assert_eq!(errors("fn f() {\n  b''\n}"), ["E001@2:3"]);
    assert_eq!(errors("fn f() {\n  b\"\u{e9}\"\n}"), ["E001@2:5"]);
    assert_eq!(errors("fn f() {\n  \"\\xFF\"\n}"), ["E001@2:4"]);
}

#[test]
fn return_expression() {
    assert_eq!(
        expr("match i { Eof => return Ok(()), Data(b) => b }"),
        "(match i (Eof => (return (call Ok (tuple)))) (Data($b) => b))"
    );
    assert_eq!(
        shown("let v = match o { None => return, Some(v) => v }"),
        ["(let v (match o (None => (return)) (Some($v) => v)))"]
    );
    assert_eq!(expr("f(return 1)"), "(call f (return 1))");
    assert_eq!(shown("return 1"), ["(return 1)"]);
}

#[test]
fn generic_declarations() {
    let p = program(
        "type Pair<A, B> { first: A, second: B }\n\
         enum Tree<T> { Leaf, Node(T) }\n\
         fn swap<A, B>(p: Pair<A, B>) -> Pair<B, A> = Pair { first = p.second, second = p.first }\n\
         type Id<T> = T\n\
         type Plain { x: Int }",
    );
    let names = |ps: &[ignite_syntax::ast::Ident]| -> Vec<String> {
        ps.iter().map(|p| p.name.clone()).collect()
    };
    match &p.decls[..] {
        [
            Decl::Record(pair),
            Decl::Enum(tree),
            Decl::Fn(swap),
            Decl::Alias(id),
            Decl::Record(plain),
        ] => {
            assert_eq!(names(&pair.type_params), ["A", "B"]);
            assert_eq!(names(&tree.type_params), ["T"]);
            assert_eq!(names(&swap.type_params), ["A", "B"]);
            assert_eq!(show_type(swap.ret.as_ref().unwrap()), "Pair<B, A>");
            assert_eq!(names(&id.params), ["T"]);
            assert!(plain.type_params.is_empty());
        }
        other => panic!("unexpected decls {other:?}"),
    }
    let role = program(
        "protocol Http {\n  fn serve<S>(stream: TcpStream, app: App<S>) -> Result<Unit, IoError> requires [stream.read, app.*] {\n  }\n}",
    );
    let Decl::Protocol(http) = &role.decls[0] else {
        panic!("expected a protocol")
    };
    assert_eq!(names(&http.fns[0].type_params), ["S"]);
    let requires: Vec<String> = http.fns[0]
        .requires
        .iter()
        .flatten()
        .map(|p| p.dotted())
        .collect();
    assert_eq!(requires, ["stream.read", "app.*"]);
}

#[test]
fn effectful_fn_types() {
    let p = program(
        "type App<S> {\n  state: S\n  handle: fn(state: S, request: Text) -> Text requires [state.*, stdout.write]\n  pure: fn(Int) -> Int\n}\n\
         fn make() -> (fn(Int) -> Int requires []) requires [stdin.read] = make()",
    );
    let Decl::Record(app) = &p.decls[0] else {
        panic!("expected a record")
    };
    let handle = &app.fields[1].ty;
    assert!(handle.is_fn());
    assert_eq!(
        handle
            .fn_names
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>(),
        ["state", "request"]
    );
    assert_eq!(show_type(&handle.fn_params()[0]), "S");
    assert_eq!(show_type(handle.fn_ret().unwrap()), "Text");
    let requires: Vec<String> = handle
        .requires
        .iter()
        .flatten()
        .map(|p| p.dotted())
        .collect();
    assert_eq!(requires, ["state.*", "stdout.write"]);
    let pure = &app.fields[2].ty;
    assert!(pure.fn_names.is_empty() && pure.requires.is_none());
    let Decl::Fn(make) = &p.decls[1] else {
        panic!("expected a fn")
    };
    assert_eq!(make.ret.as_ref().unwrap().requires, Some(vec![]));
    assert_eq!(make.requires.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        errors("type R { f: fn(a: Int, Int) -> Int }"),
        ["E001@1:24"]
    );
    assert_eq!(errors("fn f() requires [a.*.b] {\n}"), ["E001@1:21"]);
}

#[test]
fn store_resource_without_properties() {
    let p = program(
        "system S {\n  resource todos: Store<Todo>\n  resource ingress: TcpListener { address = \"127.0.0.1\", port = 1 }\n  component server: Service { listener = ingress }\n}",
    );
    let Decl::System(system) = &p.decls[0] else {
        panic!("expected a system")
    };
    let todos = &system.resources[0];
    assert_eq!(todos.kind.name, "Store");
    assert_eq!(
        todos.args.iter().map(show_type).collect::<Vec<_>>(),
        ["Todo"]
    );
    assert!(todos.properties.is_empty());
    assert!(system.resources[1].args.is_empty());
    assert_eq!(system.resources[1].properties.len(), 2);
}

#[test]
fn test_declaration() {
    let text = "fn f() = 1\n\ntest \"f is \\\"one\\\"\" {\n  assert f() == 1\n}\n";
    let p = program(text);
    let Decl::Test(test) = &p.decls[1] else {
        panic!("expected a test")
    };
    assert_eq!(test.name.name, "f is \"one\"");
    assert_eq!(
        &text[test.name.span.start..test.name.span.end],
        "\"f is \\\"one\\\"\""
    );
    assert_eq!(&text[test.span.start..test.span.start + 4], "test");
    assert_eq!(text[..test.span.end].chars().last(), Some('}'));
    assert_eq!(show_block(&test.body), "{(assert (== (call f) 1))}");
    assert!(p.decls[1].is_test_only());
    assert!(!p.decls[0].is_test_only());
}

#[test]
fn test_only_items() {
    let p = program(
        "test fn helper() -> Int = 1\ntest type Fixture { a: Int }\ntest enum Kind { A, B }\ntest const N: Int = 2\nfn normal() = 3",
    );
    let flags: Vec<bool> = p.decls.iter().map(Decl::is_test_only).collect();
    assert_eq!(flags, [true, true, true, true, false]);
    let Decl::Fn(helper) = &p.decls[0] else {
        panic!("expected a fn")
    };
    assert!(helper.test_only);
    assert_eq!(helper.span.start, 0, "the span starts at `test`");
    let Decl::Fn(normal) = &p.decls[4] else {
        panic!("expected a fn")
    };
    assert!(!normal.test_only);
}

#[test]
fn assert_statements() {
    assert_eq!(shown("assert ok"), ["(assert ok)"]);
    assert_eq!(
        shown("assert a == b, \"a is {a}\""),
        ["(assert (== a b) (+ \"a is \" (method a to_text)))"]
    );
    assert_eq!(
        shown("assert a != b,\n  \"differ\""),
        ["(assert (!= a b) \"differ\")"]
    );
    assert_eq!(shown("assert x; assert y"), ["(assert x)", "(assert y)"]);
}

#[test]
fn test_and_assert_errors() {
    assert_eq!(errors("test \"\" {\n}"), ["E053@1:6"]);
    assert_eq!(errors("test \"n {x}\" {\n}"), ["E053@1:6"]);
    assert_eq!(errors("test type A = Int"), ["E001@1:1"]);
    assert_eq!(errors("test name {\n}"), ["E001@1:6"]);
    assert_eq!(errors("fn f() {\n  test \"t\" {\n  }\n}"), ["E001@2:3"]);
    assert_eq!(errors("fn f() {\n  assert\n}"), ["E001@2:9"]);
    assert_eq!(errors("fn f() {\n  let test = 1\n}"), ["E001@2:7"]);
}

#[test]
fn keywords_still_name_paths_and_fields() {
    let p = program("package std.test\nimport std.assert.{X}\nfn f() = R { test = 1, assert = 2 }");
    assert_eq!(
        p.package.as_ref().map(|p| p.dotted()).as_deref(),
        Some("std.test")
    );
    assert_eq!(p.imports[0].path.dotted(), "std.assert");
}

#[test]
fn try_span_ends_at_the_question_mark() {
    let text = "fn f() {\nx.g(1)?.h()?\n}";
    let s = stmts("x.g(1)?.h()?");
    let StmtKind::Expr(e) = &s[0].kind else {
        panic!("expected an expression")
    };
    let ExprKind::Try(inner) = &e.kind else {
        panic!("expected `?`")
    };
    assert_eq!(&text[e.span.end - 1..e.span.end], "?");
    let ExprKind::Method { receiver, .. } = &inner.kind else {
        panic!("expected a method call")
    };
    assert_eq!(&text[receiver.span.start..receiver.span.end], "x.g(1)?");
}

#[test]
fn for_over_bytes() {
    assert_eq!(
        shown("for b in bytes { out.push(b) }"),
        ["(for b bytes {(method out push b)})"]
    );
}

// ---- slice 10 (`docs/slice.md` section 14) ------------------------------

#[test]
fn quantity_literals() {
    assert_eq!(expr("100 ms"), "(quantity 100 ms)");
    assert_eq!(expr("1.5 seconds"), "(quantity 1.5 seconds)");
    assert_eq!(expr("99.9%"), "(quantity 99.9 %)");
    assert_eq!(expr("1 MiB"), "(quantity 1 MiB)");
    assert_eq!(expr("100 ms * 2"), "(* (quantity 100 ms) 2)");
    assert_eq!(expr("a < 30 days"), "(< a (quantity 30 days))");
}

#[test]
fn named_arguments() {
    assert_eq!(
        expr("area(width = 2, height = 3)"),
        "(call area (= width 2) (= height 3))"
    );
    assert_eq!(expr("area(1, height = 3)"), "(call area 1 (= height 3))");
    assert_eq!(expr("pen.thicker(by = 2)"), "(method pen thicker (= by 2))");
}

#[test]
fn brace_literal_keys_are_names_or_references() {
    assert_eq!(
        expr("f({ admin = 1, Terminal.ingress = g() })"),
        "(call f (brace (admin 1) (Terminal.ingress (call g))))"
    );
    assert_eq!(
        shown("let m: Map<Text, Int> = {\n  admin = 1\n  guest = 2\n}"),
        ["(let m: Map<Text, Int> (brace (admin 1) (guest 2)))"]
    );
}

#[test]
fn for_with_a_tuple_pattern() {
    let stmts = stmts("for (k, v) in m {\n  log(k)\n}");
    assert_eq!(stmts.len(), 1);
    assert!(matches!(&stmts[0].kind, StmtKind::ForPattern { .. }));
    assert_eq!(shown("for x in xs {\n}")[0], "(for x xs {})");
}

#[test]
fn keywords_as_field_names() {
    assert_eq!(expr("b.system"), "(. b system)");
    assert_eq!(
        expr("Binding { system = 1, contract = 2 }"),
        "(record Binding (system 1) (contract 2))"
    );
    assert_eq!(errors("fn f() {\n  let system = 1\n}"), ["E001@2:7"]);
    assert_eq!(errors("fn policy() -> Int = 1"), ["E001@1:4"]);
    assert_eq!(errors("type assembly {\n  x: Int\n}"), ["E001@1:6"]);
}

#[test]
fn contracts_policies_and_assemblies() {
    let program = program(
        "system S {\n  resource r: TcpListener {\n    address = \"127.0.0.1\"\n    port = 1\n  }\n  component c: K {\n    x = r\n  }\n  contract q: Q {\n    signal = c.attempts\n    target = 99.9%\n  }\n  policy p: P {\n    backoff = 100 ms\n  }\n}\n\nassembly local: Deployment {\n  system = S\n  bindings = {\n    S.c = f()\n  }\n}\n",
    );
    assert_eq!(program.decls.len(), 2);
    let Decl::System(system) = &program.decls[0] else {
        panic!("a system");
    };
    assert_eq!(system.components.len(), 1);
    assert_eq!(system.contracts.len(), 1);
    assert_eq!(system.contracts[0].name.name, "q");
    assert_eq!(system.contracts[0].kind.name, "Q");
    assert_eq!(system.contracts[0].properties.len(), 2);
    assert_eq!(
        show(&system.contracts[0].properties[0].value),
        "(. c attempts)"
    );
    assert_eq!(system.policies.len(), 1);
    assert_eq!(system.policies[0].kind.name, "P");
    let Decl::Assembly(assembly) = &program.decls[1] else {
        panic!("an assembly");
    };
    assert_eq!(assembly.name.name, "local");
    assert_eq!(assembly.kind.name, "Deployment");
    assert_eq!(assembly.properties.len(), 2);
    assert_eq!(show(&assembly.properties[0].value), "S");
    assert_eq!(
        show(&assembly.properties[1].value),
        "(brace (S.c (call f)))"
    );
}

/// `docs/slice.md` 19.5: every clause lands in the AST field 19.13 names,
/// and the clause words stay names elsewhere.
#[test]
fn clauses_fill_the_ast() {
    let program = program(
        "protocol P conforms [A, B] scope { parties = [Server], sections = [\"2\"] } {\n\
           fn serve(s: TcpStream) -> Int satisfies [A.R1] = 1\n\
           unsupported B.R2 \"no\"\n\
           not_applicable A.R3 \"none\"\n\
         }\n\
         type L satisfies [A.R4] { head: Int = 8 satisfies [A.R5, B.R6], tail: Int }\n\
         test \"t\" verifies [A.R7] { assert true }\n\
         fn scope(satisfies: Int) -> Int = satisfies\n",
    );
    let path = |p: &ignite_syntax::ast::Path| p.dotted();
    let Decl::Protocol(p) = &program.decls[0] else {
        panic!("protocol")
    };
    let names: Vec<&str> = p.conforms.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["A", "B"]);
    let scope = p.scope.as_ref().expect("scope");
    assert_eq!(scope.parties.as_ref().map(Vec::len), Some(1));
    assert_eq!(scope.sections.as_ref().map(|s| s[0].0.as_str()), Some("2"));
    assert_eq!(
        p.fns[0].satisfies.iter().map(path).collect::<Vec<_>>(),
        ["A.R1"]
    );
    let statuses: Vec<(String, &str)> = p
        .statuses
        .iter()
        .map(|s| (path(&s.requirement), s.kind.word()))
        .collect();
    assert_eq!(
        statuses,
        [
            ("B.R2".to_string(), "unsupported"),
            ("A.R3".to_string(), "not_applicable")
        ]
    );
    assert_eq!(p.statuses[0].reason, "no");
    let Decl::Record(r) = &program.decls[1] else {
        panic!("record")
    };
    assert_eq!(r.satisfies.iter().map(path).collect::<Vec<_>>(), ["A.R4"]);
    assert_eq!(
        r.fields[0].satisfies.iter().map(path).collect::<Vec<_>>(),
        ["A.R5", "B.R6"]
    );
    assert!(r.fields[1].satisfies.is_empty());
    let Decl::Test(t) = &program.decls[2] else {
        panic!("test")
    };
    assert_eq!(t.verifies.iter().map(path).collect::<Vec<_>>(), ["A.R7"]);
    let Decl::Fn(f) = &program.decls[3] else {
        panic!("fn")
    };
    assert_eq!(f.name.name, "scope");
    assert!(f.satisfies.is_empty());
}

/// Adversary (slice 15 U1, `docs/slice.md` 19.5): the eight clause words
/// are identifiers outside the clause positions: records, fields, enums,
/// variants, fns, params, protocols, roles, locals.
#[test]
fn adversary_clause_words_are_names_outside_clause_positions() {
    let program = program(
        "type scope {\n\
           conforms: Int = 1\n\
           satisfies: Int\n\
           verifies: Int = 2\n\
           unsupported: Int\n\
         }\n\
         enum parties { unsupported, not_applicable, sections }\n\
         fn conforms(satisfies: Int, verifies: Int) -> Int = satisfies + verifies\n\
         protocol not_applicable {\n\
           fn unsupported(scope: TcpStream) -> Int {\n\
             let sections = 1\n\
             let parties = sections\n\
             return parties\n\
           }\n\
         }\n\
         test \"names\" {\n\
           let verifies = conforms(1, 2)\n\
           assert verifies == 3\n\
         }\n",
    );
    let Decl::Record(r) = &program.decls[0] else {
        panic!("record")
    };
    let fields: Vec<&str> = r.fields.iter().map(|f| f.name.name.as_str()).collect();
    assert_eq!(fields, ["conforms", "satisfies", "verifies", "unsupported"]);
    assert!(r.fields.iter().all(|f| f.satisfies.is_empty()));
    let Decl::Protocol(p) = &program.decls[3] else {
        panic!("protocol")
    };
    assert!(p.conforms.is_empty() && p.statuses.is_empty());
    assert_eq!(p.fns[0].name.name, "unsupported");
}

/// Adversary (slice 15 U1, 19.5): a clause outside its production, an
/// empty list where the grammar asks for one entry, and a `scope` entry
/// of the wrong kind are each E001, never parsed silently.
#[test]
fn adversary_clauses_outside_their_productions_are_e001() {
    for (case, text) in [
        ("const", "const X: Int = 1 satisfies [A.B]\n"),
        ("enum", "enum E satisfies [A.B] { X }\n"),
        (
            "test satisfies",
            "test \"t\" satisfies [A.B] { assert true }\n",
        ),
        ("fn verifies", "fn f() -> Int verifies [A.B] = 1\n"),
        ("fn after body", "fn f() -> Int = 1 satisfies [A.B]\n"),
        (
            "satisfies before requires",
            "fn f(s: TcpStream) -> Int satisfies [A.B] requires [s.read] = 1\n",
        ),
        ("record conforms", "type T conforms [A] { x: Int }\n"),
        (
            "field before default",
            "type T { x: Int satisfies [A.B] = 1 }\n",
        ),
        ("top-level status", "unsupported A.B \"r\"\n"),
        ("empty conforms", "protocol P conforms [] {\n}\n"),
        ("empty satisfies", "fn f() -> Int satisfies [] = 1\n"),
        ("empty verifies", "test \"t\" verifies [] { assert true }\n"),
        (
            "two scopes",
            "protocol P conforms [A] scope { } scope { } {\n}\n",
        ),
        (
            "qualified party",
            "protocol P conforms [A] scope { parties = [Party.Server] } {\n}\n",
        ),
        (
            "numeric section",
            "protocol P conforms [A] scope { sections = [2] } {\n}\n",
        ),
        (
            "empty reason",
            "protocol P conforms [A] {\n  unsupported A.B \"\"\n}\n",
        ),
        ("three segments", "fn f() -> Int satisfies [A.B.C] = 1\n"),
        (
            "parties twice",
            "protocol P conforms [A] scope { parties = [Any], parties = [Server] } {\n}\n",
        ),
    ] {
        let found = errors(text);
        assert!(
            found
                .first()
                .is_some_and(|e| e.starts_with("E001@1:") || e.starts_with("E001@2:")),
            "{case}: {found:?}"
        );
    }
}
