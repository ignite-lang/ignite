    fn is_pure_call(&self, callee: &Expr) -> bool {
        match &callee.kind {
            ExprKind::Name(name) => match self.local(&name.name) {
                Some(local) => !local
                    .ty
                    .as_ref()
                    .is_some_and(|ty| self.expand(ty).requires.is_some()),
                None => self
                    .symbols
                    .functions
                    .get(&name.name)
                    .is_some_and(|f| !f.effectful),
            },
            _ => false,
        }
    }
