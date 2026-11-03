
/// The symbol of an instance of the generic declaration `base`:
/// `"<module>.<name><Int,Text>"` (`docs/llvm.md` section 3).
pub fn instance_symbol(base: &str, args: &[Ty]) -> String {
    let args: Vec<String> = args.iter().map(crate::layout::mangle).collect();
    format!("{base}<{}>", args.join(","))
}

/// `sig` with `params` replaced by `args`.
fn substitute_sig(sig: &FnSig, params: &[String], args: &[Ty]) -> FnSig {
    FnSig {
        params: sig
            .params
            .iter()
            .map(|t| t.substitute(params, args))
            .collect(),
        ret: sig.ret.substitute(params, args),
        effectful: sig.effectful,
    }
}

/// Define `instance` in `m`, lowering its generic declaration from the
/// declaring module's source with the instance's type arguments.
pub fn define_instance(m: &mut Module<'_>, instance: &Instance) {
    let (Generic::Fn(q) | Generic::Role(q, _) | Generic::Default(q, _)) = &instance.generic;
    let source = m.source(&q.module).unwrap_or_else(|| {
        panic!(
            "instance {} needs the source of module {}",
            instance.symbol, q.module
        )
    });
    let env = m.env;
    let cx = Ctx::new(source.path, source.program, source.symbols, source.types);
    let args = instance.args.clone();
    match &instance.generic {
        Generic::Fn(q) => {
            let decl = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Fn(f) if f.name.name == q.name => Some(f),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no fn {}", symbol(q)));
            let params = env.generics(q).to_vec();
            let sig = env
                .fn_sig(q)
                .unwrap_or_else(|| panic!("no signature for {}", symbol(q)));
            let sig = substitute_sig(sig, &params, &args);
            define_body(m, &cx, instance, decl, &sig, (params, args));
        }
        Generic::Role(q, role) => {
            let decl = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Protocol(p) if p.name.name == q.name => {
                        p.fns.iter().find(|f| f.name.name == *role)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no role {}.{role}", symbol(q)));
            let params = env.role_generics(q, role).to_vec();
            let sig = env
                .role_sig(q, role)
                .unwrap_or_else(|| panic!("no signature for role {}.{role}", symbol(q)));
            let sig = substitute_sig(sig, &params, &args);
            define_body(m, &cx, instance, decl, &sig, (params, args));
        }
        Generic::Default(q, field) => {
            let value = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Record(r) if r.name.name == q.name => r
                        .fields
                        .iter()
                        .find(|f| f.name.name == *field)
                        .and_then(|f| f.default.as_ref()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no default of {}.{field}", symbol(q)));
            let ty = Ty::GenericRecord(q.clone(), args.clone());
            let field_ty = env
                .fields_of(&ty)
                .into_iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t)
                .unwrap_or_else(|| panic!("no field {}.{field}", symbol(q)));
            let params = env.generics(q).to_vec();
            Lower::define_const_in(
                m,
                &cx,
                &instance.symbol,
                &field_ty,
                value,
                (params, args),
                true,
            );
        }
    }
}

fn define_body(
    m: &mut Module<'_>,
    cx: &Ctx<'_>,
    instance: &Instance,
    decl: &telos_syntax::ast::FnDecl,
    sig: &FnSig,
    subst: (Vec<String>, Vec<Ty>),
) {
    let params: Vec<(Ident, Ty)> = decl
        .params
        .iter()
        .zip(&sig.params)
        .map(|(p, ty)| (p.name.clone(), ty.clone()))
        .collect();
    Lower::define_fn_in(
        m,
        cx,
        &instance.symbol,
        &params,
        &sig.ret,
        &decl.body,
        subst,
        true,
    );
}
