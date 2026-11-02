EDITS = [
("""                _ => self.arg_roots(args.iter()),
            },
            ExprKind::Closure(closure) => self.roots(&closure.body),""",
"""                ExprKind::Name(f) if self.lookup(&f.name).is_some() => self
                    .roots(callee)
                    .into_iter()
                    .chain(self.arg_roots(args.iter()))
                    .collect(),
                _ => self.arg_roots(args.iter()),
            },
            ExprKind::Closure(closure) => self.roots(&closure.body),"""),
]
