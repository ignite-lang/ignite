    /// The parameters of the enclosing function the value of `expr` may
    /// come from (`docs/slice.md` section 5.1): a parameter, a local bound
    /// or assigned from one, a field, element, `copy()`, `at`, `slice` or
    /// `e?` of one, a tuple, list or record literal holding one, the value
    /// of an `if`, `match` or block that may be one, and the result of any
    /// call of a user fn (pure or effectful, by name or UFCS), a fn value,
    /// a variant constructor, a role or a `Store` method taking one as an
    /// argument whose type can hold a substrate handle. Empty for literals,
    /// operators and the results of substrate operations: the function owns
    /// them.
    fn roots(&self, expr: &Expr) -> BTreeSet<String> {
        match &expr.kind {
            ExprKind::Name(name) => self
                .local(&name.name)
                .map(|local| local.roots.clone())
                .unwrap_or_default(),
            ExprKind::Field { receiver, .. } if self.path_head(receiver).is_some() => {
                BTreeSet::new()
            }
            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.roots(receiver),
            ExprKind::Tuple(items) | ExprKind::List(items) => {
                items.iter().flat_map(|item| self.roots(item)).collect()
            }
            ExprKind::Record { fields, .. } => fields
                .iter()
                .flat_map(|field| self.roots(&field.value))
                .collect(),
            ExprKind::Block(block) => self.block_roots(block),
            ExprKind::If {
                then, otherwise, ..
            } => {
                let mut roots = self.block_roots(then);
                if let Some(otherwise) = otherwise {
                    roots.extend(self.roots(otherwise));
                }
                roots
            }
            ExprKind::Match { arms, .. } => arms
                .iter()
                .flat_map(|arm| match self.tail_roots.get(&expr_key(&arm.body)) {
                    Some(roots) => roots.clone(),
                    None => self.roots(&arm.body),
                })
                .collect(),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => match self.path_head(receiver) {
                Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                Some(_) => self.arg_roots(args.iter()),
                None => self.method_roots(receiver, name, args),
            },
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                    Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                    _ => self.arg_roots(args.iter()),
                },
                _ => self.arg_roots(args.iter()),
            },
            _ => BTreeSet::new(),
        }
    }

    /// The roots of `receiver.name(args)` when it is no path call.
    fn method_roots(&self, receiver: &Expr, name: &Ident, args: &[Expr]) -> BTreeSet<String> {
        match name.name.as_str() {
            "copy" | "at" | "slice" => return self.roots(receiver),
            "concat" => return self.arg_roots(std::iter::once(receiver).chain(args)),
            _ => {}
        }
        if BUILTIN_METHODS.contains(&name.name.as_str()) {
            return BTreeSet::new();
        }
        let receiver_ty = self.static_type(receiver);
        let fn_field = receiver_ty
            .as_ref()
            .and_then(|ty| self.field_type(ty, &name.name))
            .is_some_and(|ty| ty.is_fn());
        let store = self
            .store_op(receiver_ty.as_ref(), &name.name)
            .is_some();
        if store || (!fn_field && self.ufcs(receiver, name).is_some()) {
            self.arg_roots(std::iter::once(receiver).chain(args))
        } else {
            self.arg_roots(args.iter())
        }
    }

    /// The roots of the arguments of a call whose types can hold a
    /// substrate handle; an argument of a declared type that cannot
    /// (`Int`, `Text`, ..) adds none.
    fn arg_roots<'e>(&self, args: impl Iterator<Item = &'e Expr>) -> BTreeSet<String> {
        args.filter(|arg| {
            self.static_type(arg)
                .is_none_or(|ty| self.can_hold_handle(&ty, 0))
        })
        .flat_map(|arg| self.roots(arg))
        .collect()
    }

    /// The roots of the value of `block`: those of its last statement when
    /// that is an expression, recorded while the block's scope was open.
    fn block_roots(&self, block: &Block) -> BTreeSet<String> {
        self.tail_roots
            .get(&(block as *const Block as usize))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether a value of declared type `ty` may hold a `TcpStream`,
    /// `TcpListener`, `Store`, `Stdin` or `Stdout`, directly or in a
    /// field, payload, element or type argument. Unknown types may.
    fn can_hold_handle(&self, ty: &Type, depth: usize) -> bool {
        if depth > 16 {
            return true;
        }
        if ty.is_fn() {
            return false;
        }
        let ty = self.expand(ty);
        let name = ty.name.name.as_str();
        match name {
            "Int" | "Text" | "Bool" | "Byte" | "Bytes" | "Unit" | "IoError" | "Input" => {
                return false;
            }
            "TcpStream" | "TcpListener" | "Store" | "Stdin" | "Stdout" => return true,
            "List" | "Option" | "Result" | "Tuple" => {
                return ty.args.iter().any(|a| self.can_hold_handle(a, depth + 1));
            }
            _ => {}
        }
        if self.type_params.contains(name) {
            return true;
        }
        let params = self
            .symbols
            .type_params
            .get(name)
            .cloned()
            .unwrap_or_default();
        let members: Vec<Type> = if let Some(fields) = self.symbols.record_field_types.get(name) {
            fields.iter().map(|(_, t)| t.clone()).collect()
        } else if let Some(payloads) = self.symbols.enum_payload_types.get(name) {
            payloads.clone()
        } else {
            return true;
        };
        members
            .iter()
            .any(|m| self.can_hold_handle(&substitute_type(m, &params, &ty.args), depth + 1))
    }
