EDITS = [
("""            "concat" => return self.arg_roots(std::iter::once(receiver).chain(args)),
            _ => {}
        }
        if self.builtin_method(receiver, name) {""",
"""            "concat" => return self.arg_roots(std::iter::once(receiver).chain(args)),
            "map" if self.builtin_method(receiver, name) => {
                return self
                    .roots(receiver)
                    .into_iter()
                    .chain(args.iter().flat_map(|f| self.roots(f)))
                    .collect();
            }
            _ => {}
        }
        if self.builtin_method(receiver, name) {"""),
("""            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                    Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                    _ => self.arg_roots(args.iter()),
                },
                _ => self.arg_roots(args.iter()),
            },
            _ => BTreeSet::new(),""",
"""            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                    Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                    _ => self.arg_roots(args.iter()),
                },
                _ => self.arg_roots(args.iter()),
            },
            ExprKind::Closure(closure) => self.roots(&closure.body),
            _ => BTreeSet::new(),"""),
]
