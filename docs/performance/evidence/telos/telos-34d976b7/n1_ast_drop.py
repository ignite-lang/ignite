EDITS = [
("""#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
""",
"""#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// Drops the operands of a binary or unary operator chain from a work
/// list instead of recursively, so that a 100,000-term `+` chain does not
/// overflow the stack (`docs/slice.md` section 13, row 14).
impl Drop for Expr {
    fn drop(&mut self) {
        if !matches!(
            self.kind,
            ExprKind::Binary { .. } | ExprKind::Unary { .. }
        ) {
            return;
        }
        let mut pending = Vec::new();
        take_operands(&mut self.kind, &mut pending);
        while let Some(mut expr) = pending.pop() {
            take_operands(&mut expr.kind, &mut pending);
        }
    }
}

fn take_operands(kind: &mut ExprKind, pending: &mut Vec<Expr>) {
    if !matches!(kind, ExprKind::Binary { .. } | ExprKind::Unary { .. }) {
        return;
    }
    match std::mem::replace(kind, ExprKind::Break) {
        ExprKind::Binary { left, right, .. } => {
            pending.push(*left);
            pending.push(*right);
        }
        ExprKind::Unary { operand, .. } => pending.push(*operand),
        _ => {}
    }
}
"""),
]
