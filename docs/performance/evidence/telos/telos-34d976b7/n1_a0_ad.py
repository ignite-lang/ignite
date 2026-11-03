EDITS = [
("""        if matches!(name.name.as_str(), "push" | "append")
            && let ExprKind::Name(list) = &receiver.kind
        {
            let roots = self.arg_roots(args.iter());
            if let Some(local) = self
                .scopes
                .iter_mut()
                .rev()
                .find_map(|scope| scope.get_mut(&list.name))
            {
                local.roots.extend(roots);
            }
        }""",
"""        let mutating = matches!(name.name.as_str(), "push" | "append" | "clear")
            && self.builtin_method(receiver, name);
        if mutating {
            self.mutable_receiver(expr, receiver, name);
        }
        if matches!(name.name.as_str(), "push" | "append")
            && let Some(list) = place_root(receiver)
        {
            let roots = self.arg_roots(args.iter());
            if let Some(local) = self
                .scopes
                .iter_mut()
                .rev()
                .find_map(|scope| scope.get_mut(&list.name))
            {
                local.roots.extend(roots);
            }
        }"""),
("""    fn method(&mut self, expr: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) {
        if let Some(ty) = self.path_head(receiver) {""",
"""    /// E006 unless the receiver of an in-place `push`, `append` or `clear`
    /// is a place rooted in a `var` (`docs/slice.md` section 13.11 A0-a):
    /// a `let`, a parameter, a `for` element or a pattern binding is a
    /// value, and `x.to_bytes()` shares `x`'s buffer.
    fn mutable_receiver(&mut self, expr: &Expr, receiver: &Expr, name: &Ident) {
        let mut current = receiver;
        loop {
            match &current.kind {
                ExprKind::Field { receiver, .. }
                | ExprKind::Index { receiver, .. }
                | ExprKind::TupleField { receiver, .. } => current = receiver,
                ExprKind::Method {
                    name: inner,
                    args,
                    ..
                } if inner.name == "to_bytes" && args.is_empty() => {
                    self.error(
                        "E006",
                        expr.span,
                        format!(
                            "cannot `{}` on the result of `to_bytes()`, which shares the text's buffer; bind it first: `var b = t.to_bytes()`",
                            name.name
                        ),
                    );
                    return;
                }
                _ => break,
            }
        }
        let ExprKind::Name(base) = &current.kind else {
            return;
        };
        if self.captured(&base.name) {
            return;
        }
        let what = match self.lookup(&base.name) {
            Some(Binding::Let) => "the immutable binding",
            Some(Binding::Param) => "the parameter",
            Some(Binding::Loop) => "the `for` element",
            Some(Binding::Pattern) => "the pattern binding",
            Some(Binding::Var) | None => return,
        };
        self.error(
            "E006",
            expr.span,
            format!(
                "cannot `{}` through {what} `{}`: only a place rooted in a `var` changes in place; copy it into a `var` first",
                name.name, base.name
            ),
        );
    }

    fn method(&mut self, expr: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) {
        if let Some(ty) = self.path_head(receiver) {"""),
]
