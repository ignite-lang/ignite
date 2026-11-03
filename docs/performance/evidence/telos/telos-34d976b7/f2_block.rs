    fn block(&mut self, block: &Block) {
        self.scopes.push(BTreeMap::new());
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
        if let Some(Stmt {
            kind: StmtKind::Expr(tail),
            ..
        }) = block.stmts.last()
        {
            let roots = self.roots(tail);
            self.tail_roots
                .insert(block as *const Block as usize, roots);
        }
        self.scopes.pop();
    }
