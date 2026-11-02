EDITS = [
("""    /// `E055` for an own record that holds a value of itself inline through""",
"""    /// `E057` for a type parameter of a generic record, enum or alias that
    /// no field, payload or aliased type uses: no value fixes it, and the
    /// Rust target rejects it (`docs/slice.md` section 13.11).
    fn unused_type_params(&mut self, program: &Program) {
        fn uses(ty: &Type, param: &str) -> bool {
            ty.name.name == param || ty.args.iter().any(|arg| uses(arg, param))
        }
        for decl in &program.decls {
            let (name, params, types): (&Ident, &[Ident], Vec<&Type>) = match decl {
                Decl::Record(r) => (
                    &r.name,
                    &r.type_params,
                    r.fields.iter().map(|f| &f.ty).collect(),
                ),
                Decl::Enum(e) => (
                    &e.name,
                    &e.type_params,
                    e.variants.iter().flat_map(|v| &v.payload).collect(),
                ),
                Decl::Alias(a) => (&a.name, &a.params, vec![&a.ty]),
                _ => continue,
            };
            for param in params {
                if !types.iter().any(|ty| uses(ty, &param.name)) {
                    self.error(
                        "E057",
                        param.span,
                        format!(
                            "type parameter `{}` of `{}` is never used; remove it or use it in a field",
                            param.name, name.name
                        ),
                    );
                }
            }
        }
    }

    /// `E055` for an own record that holds a value of itself inline through"""),
("""    checker.recursive_types(program);
    let collected = checker.diagnostics.len();""",
"""    checker.recursive_types(program);
    checker.unused_type_params(program);
    let collected = checker.diagnostics.len();"""),
]
