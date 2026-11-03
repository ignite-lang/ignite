EDITS = [
("""                Decl::Fn(f) if f.owner.is_some() => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {
                        self.function(f, &sig);
                    }
                }""", ""),
("""                Decl::Fn(f) if f.owner.is_none() => {
                    if let Some(sig) = self.own.fns.get(&f.name.name).cloned() {""",
"""                Decl::Fn(f) => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {"""),
]
