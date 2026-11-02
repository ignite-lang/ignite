EDITS = [
("""        for param in &decl.params {
            self.check_type(&param.ty);
        }
        if let Some(ret) = &decl.ret {
            self.check_type(ret);
        }
        let role = decl.owner.is_none() && display.contains('.');""",
"""        for param in &decl.params {
            self.ambient_name(&param.name);
            self.check_type(&param.ty);
        }
        if let Some(ret) = &decl.ret {
            self.check_type(ret);
        }
        let role = decl.owner.is_none() && display.contains('.');"""),
("""    fn check_type(&mut self, ty: &Type) {
        let name = ty.name.name.as_str();""",
"""    /// E056 for a parameter (of a fn, a role, a fn type or a closure) named
    /// like an ambient capability: its `requires` head would be the
    /// parameter, so the ambient effect would be charged to the argument's
    /// roots instead (`docs/slice.md` section 13.11).
    fn ambient_name(&mut self, name: &Ident) {
        if AMBIENT_HEADS.contains(&name.name.as_str()) {
            self.error(
                "E056",
                name.span,
                format!(
                    "parameter `{}` is named like the ambient capability `{}`; rename it",
                    name.name, name.name
                ),
            );
        }
    }

    fn check_type(&mut self, ty: &Type) {
        for param in &ty.fn_names {
            self.ambient_name(param);
        }
        let name = ty.name.name.as_str();"""),
("""            seen.push(&param.name.name);
            if let Some(ty) = &param.ty {""",
"""            seen.push(&param.name.name);
            self.ambient_name(&param.name);
            if let Some(ty) = &param.ty {"""),
("""/// The ambient effect an effectful builtin static needs.""",
"""/// The heads of ambient effects (`stdin.read`, `net.connect`): no
/// parameter may take one of these names (E056).
pub const AMBIENT_HEADS: &[&str] = &["stdin", "stdout", "process", "net"];

/// The ambient effect an effectful builtin static needs."""),
]
