EDITS = [
("""fn builtin_applies(ty: &Type, method: &str) -> bool {""",
"""/// The name a place `x`, `x.f`, `x[i]` or `x.0` (nested) is rooted in.
fn place_root(expr: &Expr) -> Option<&Ident> {
    let mut current = expr;
    loop {
        match &current.kind {
            ExprKind::Name(name) => return Some(name),
            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::TupleField { receiver, .. } => current = receiver,
            _ => return None,
        }
    }
}

fn builtin_applies(ty: &Type, method: &str) -> bool {"""),
]
