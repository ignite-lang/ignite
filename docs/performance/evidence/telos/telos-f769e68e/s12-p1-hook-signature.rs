    /// E109 at a `fn K.machine` or `fn K.validate` declaration whose
    /// signature is not the hook's (`docs/slice.md` 16.6, 16.17): `fn
    /// K.machine(self: K, id: Text) -> std.control.Machine` and `fn
    /// K.validate(self: K) -> List<Text>`, both pure, like `K.main` (14.5).
    /// The driver finds both by name on every record, the kinds of
    /// members and of their property values alike, so the shape is checked
    /// where the fn is declared, on records and enums of this module. A
    /// kind without a hook passes here; whether its host needs one is
    /// E129, decided per assembly.
    pub(super) fn hook_signature(&mut self, f: &FnDecl) {
        let Some(owner) = &f.owner else {
            return;
        };
        let (extra, arity, ret) = match f.name.name.as_str() {
            "machine" => (
                "id: Text",
                2,
                Ty::Enum(QualName::new("std.control", "Machine")),
            ),
            "validate" => ("", 1, Ty::List(Box::new(Ty::Text))),
            _ => return,
        };
        let hook_name = f.qualified_name();
        let Some(sig) = self.own.fns.get(&hook_name).cloned() else {
            return;
        };
        let self_ok = matches!(
            sig.params.first(),
            Some(Ty::Record(q) | Ty::GenericRecord(q, _) | Ty::Enum(q) | Ty::GenericEnum(q, _))
                if q.module == self.module && q.name == owner.name
        );
        let shape_ok = self_ok
            && sig.params.len() == arity
            && (arity == 1 || sig.params[1] == Ty::Text)
            && sig.ret == ret;
        if shape_ok && !sig.effectful {
            return;
        }
        let found = fn_ty(sig.params.clone(), sig.ret.clone());
        let comma = if extra.is_empty() { "" } else { ", " };
        let purity = if sig.effectful {
            " and it must be pure"
        } else {
            ""
        };
        self.error(
            "E109",
            f.name.span,
            format!(
                "`{hook_name}` is a hook of `ignite run` and must have the signature `fn {hook_name}(self: {}{comma}{extra}) -> {ret}`, found `{found}`{purity}",
                owner.name
            ),
        );
    }

