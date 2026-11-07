//! The typer's tables are keyed by node ids, not by heap addresses
//! (story `stable-node-ids`): a clone of a `Program` types to the same
//! results, and the result of typing the clone answers for the nodes of
//! the original.

use std::path::Path;

use crate::common;

use ignite_syntax::ast::{Decl, Expr, ExprKind, Pattern, PatternKind, Program, Type};
use ignite_types::{TypedProgram, type_module};

/// Everything the typed program says about one expression.
fn facts(typed: &TypedProgram, e: &Expr) -> String {
    format!(
        "{:?} {:?}",
        (
            typed.try_type_of(e),
            typed.callee(e),
            typed.instance(e),
            typed.closure(e),
            typed.quantity(e),
            typed.named_args(e),
        ),
        (
            typed.brace_kind(e),
            typed.reference(e),
            typed.module_record(e),
            typed.failure_site(e),
            typed.effects(e),
            typed.moves(e),
            typed.assert_compare(e),
            typed.test_try(e),
        )
    )
}

fn patterns<'p>(pattern: &'p Pattern, out: &mut Vec<&'p Pattern>) {
    out.push(pattern);
    match &pattern.kind {
        PatternKind::Or(items) | PatternKind::Tuple(items) | PatternKind::List(items) => {
            for item in items {
                patterns(item, out);
            }
        }
        PatternKind::Variant { args, .. } => {
            for arg in args {
                patterns(arg, out);
            }
        }
        PatternKind::Range { start, end } => {
            patterns(start, out);
            patterns(end, out);
        }
        _ => {}
    }
}

fn types<'p>(ty: &'p Type, out: &mut Vec<&'p Type>) {
    out.push(ty);
    for arg in &ty.args {
        types(arg, out);
    }
}

fn declared_types(program: &Program) -> Vec<&Type> {
    let mut out = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Fn(f) => {
                for p in &f.params {
                    types(&p.ty, &mut out);
                }
                if let Some(ret) = &f.ret {
                    types(ret, &mut out);
                }
            }
            Decl::Record(r) => {
                for field in &r.fields {
                    types(&field.ty, &mut out);
                }
            }
            Decl::Const(c) => types(&c.ty, &mut out),
            Decl::Alias(a) => types(&a.ty, &mut out),
            _ => {}
        }
    }
    out
}

/// Every fact of `typed` about the nodes of `program`, one line per node.
fn all_facts(typed: &TypedProgram, program: &Program) -> Vec<String> {
    let mut out = Vec::new();
    for e in common::expressions(program) {
        out.push(facts(typed, e));
        match &e.kind {
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    let mut ps = Vec::new();
                    patterns(&arm.pattern, &mut ps);
                    for p in ps {
                        out.push(format!(
                            "{:?}",
                            (typed.pattern_const(p), typed.range_bounds(p))
                        ));
                    }
                }
            }
            ExprKind::Brace { entries } => {
                for entry in entries {
                    out.push(format!("{:?}", typed.brace_key(&entry.key)));
                }
            }
            _ => {}
        }
    }
    for ty in declared_types(program) {
        out.push(format!("{:?}", typed.resolved_type(ty)));
    }
    out
}

#[test]
fn a_cloned_program_types_to_the_same_results() {
    let std = common::load_std(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std"))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut compared = 0;
    for file in &std.files {
        let original = common::retype(&file.program, &file.symbols, &std.typed);
        let clone = file.program.clone();
        let typed_clone = type_module(&clone, &file.symbols, &std.typed)
            .unwrap_or_else(|d| panic!("{}", file.source.render_all(&d)));
        let expected = all_facts(&original, &file.program);
        assert_eq!(
            all_facts(&typed_clone, &clone),
            expected,
            "{}: the clone's own nodes",
            file.module
        );
        assert_eq!(
            all_facts(&typed_clone, &file.program),
            expected,
            "{}: the original's nodes looked up in the clone's typing",
            file.module
        );
        drop(clone);
        assert_eq!(
            all_facts(&typed_clone, &file.program),
            expected,
            "{}: after the clone is dropped",
            file.module
        );
        compared += expected.len();
    }
    assert!(compared > 10_000, "only {compared} facts compared");
}
