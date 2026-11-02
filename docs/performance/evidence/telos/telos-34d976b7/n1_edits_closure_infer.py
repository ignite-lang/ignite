EDITS = [
    (
        "                let Some(found) = self.synth(arg) else {\n",
        "                let given = matches!(arg.kind, ExprKind::Closure(_)).then_some(&param);\n"
        "                let Some(found) = self.expr(arg, Mode::Value(given)) else {\n",
    ),
    (
        "        match self.synth(f)? {\n",
        "        let open = fn_ty(vec![element.clone()], Ty::Param(\"'U\".into()));\n"
        "        let given = matches!(f.kind, ExprKind::Closure(_)).then_some(&open);\n"
        "        match self.expr(f, Mode::Value(given))? {\n",
    ),
]
