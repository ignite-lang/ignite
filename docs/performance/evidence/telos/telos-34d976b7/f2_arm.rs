                    self.pattern(&arm.pattern, &roots, Binding::Pattern);
                    self.expr(&arm.body);
                    let tail = self.roots(&arm.body);
                    self.tail_roots.insert(expr_key(&arm.body), tail);
                    self.scopes.pop();
