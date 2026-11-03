            ListMap => self.list_map(receiver.expect("receiver"), &args[0]),
            IntParse => {
                let text = self.read_arg(&args[0]);
                self.option_from("int_parse", &Ty::Int, |payload| {
                    format!("ptr {text}, ptr {payload}")
                })
            }
            ByteToInt => {
                let r = recv(self);
                let b = self.scalar(&r);
                Val::Scalar(self.f.val(format!("zext i8 {b} to i64")))
            }
            IntToByte => {
                let r = recv(self);
                let i = self.scalar(&r);
                let ty = Ty::Option(Box::new(Ty::Byte));
                let layout = self.env.enum_layout(&ty);
                let result = self.temp(&ty);
                let fits = self.f.val(format!("icmp ult i64 {i}, 256"));
                let tag = self.f.val(format!("select i1 {fits}, i32 0, i32 1"));
                self.f.ins(format!("store i32 {tag}, ptr {result}, align 4"));
                let byte = self.f.val(format!("trunc i64 {i} to i8"));
                let payload = self.gep(&result, layout.variants[0].1[0].1);
                self.f.ins(format!("store i8 {byte}, ptr {payload}, align 1"));
                Val::Ptr(result, true)
            }
            TcpStreamSetIdleTimeout => {
                let r = recv(self);
                let s = self.scalar(&r);
                let ms = self.scalar_arg(&args[0]);
                self.rt_result(
                    "tcp_set_idle_timeout",
                    &[format!("i64 {s}"), format!("i64 {ms}")],
                    Ty::Unit,
                )
            }
            StoreNextId => {
                let r = recv(self);
                let s = self.scalar(&r);
                Val::Scalar(
                    self.f
                        .val(format!("call i64 @telos_rt_store_next_id(ptr {s})")),
                )
            }
            StoreGet | StoreRemove => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let name = if b == StoreGet {
                    "store_get"
                } else {
                    "store_remove"
                };
                self.option_from(name, &element, |payload| {
                    format!("ptr {s}, i64 {id}, ptr {payload}")
                })
            }
            StorePut => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let v = self.expr(&args[1], true);
                if v == Val::Never {
                    return Val::Never;
                }
                let have = self.ty(&args[1]);
                let v = self.coerce(&element, &have, v);
                let value = match v {
                    Val::Ptr(p, _) => p,
                    other => {
                        let t = self.temp(&element);
                        self.store_value(&element, &t, other);
                        t
                    }
                };
                self.f.ins(format!(
                    "call void @telos_rt_store_put(ptr {s}, i64 {id}, ptr {value})"
                ));
                Val::Unit
            }
            StoreValues => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let out = self.temp(&Ty::List(Box::new(element)));
                self.f.ins(format!(
                    "call void @telos_rt_store_values(ptr {s}, ptr {out})"
                ));
                Val::Ptr(out, true)
            }
            StoreUpdate => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let f = self.scalar_arg(&args[1]);
                let thunk = self.m.glue_of(Glue::Update, &element);
                self.option_from("store_update", &element, |payload| {
                    format!("ptr {s}, i64 {id}, ptr {f}, ptr {thunk}, ptr {payload}")
                })
            }
        }
    }

    /// The element type of the `Store` receiver of a builtin.
    fn store_element(&self, receiver: Option<&Expr>) -> Ty {
        match self.ty(receiver.expect("a store method has a receiver")) {
            Ty::Store(t) => *t,
            other => panic!("store method on {other:?}"),
        }
    }

    /// `Option<ty>` from `i8 telos_rt_<name>(args)`, where `args` gets the
    /// pointer to the `Some` payload, which the call fills when it returns
    /// 1.
    fn option_from(&mut self, name: &str, ty: &Ty, args: impl FnOnce(&str) -> String) -> Val {
        let option = Ty::Option(Box::new(ty.clone()));
        let layout = self.env.enum_layout(&option);
        let result = self.temp(&option);
        let payload = self.gep(&result, layout.variants[0].1[0].1);
        let found = self
            .f
            .val(format!("call i8 @telos_rt_{name}({})", args(&payload)));
        let some = self.f.val(format!("icmp ne i8 {found}, 0"));
        let tag = self.f.val(format!("select i1 {some}, i32 0, i32 1"));
        self.f.ins(format!("store i32 {tag}, ptr {result}, align 4"));
        Val::Ptr(result, true)
    }

    /// `list.map(f)`: a new list of `f` applied to a copy of each element,
    /// in order.
    fn list_map(&mut self, receiver: &Expr, f: &Expr) -> Val {
        let element = match self.ty(receiver) {
            Ty::List(t) => *t,
            other => panic!("map on {other:?}"),
        };
        let out = match self.ty(f) {
            Ty::Fn(_, ret) | Ty::EffectFn(_, ret, _) => *ret,
            other => panic!("map with a {other:?}"),
        };
        let r = self.expr(receiver, false);
        if r == Val::Never {
            return Val::Never;
        }
        let list = self.ptr(&r);
        let fv = self.expr(f, true);
        let fp = self.scalar(&fv);
        let len = self.list_len(&list);
        let in_size = self.layout(&element).size;
        let out_layout = self.layout(&out);
        let bytes = self
            .f
            .val(format!("mul i64 {len}, {}", out_layout.size.max(1)));
        let buf = self.f.val(format!(
            "call ptr @telos.alloc(i64 {bytes}, i64 {})",
            out_layout.align.max(1)
        ));
        let data = self.f.val(format!("load ptr, ptr {list}, align 8"));
        let counter = self.f.alloca(Layout::scalar(8));
        self.f.ins(format!("store i64 0, ptr {counter}, align 8"));
        let (head, body, exit) = (self.f.label(), self.f.label(), self.f.label());
        self.f.place(&head);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let more = self.f.val(format!("icmp slt i64 {i}, {len}"));
        self.f.cond_br(&more, &body, &exit);
        self.f.place(&body);
        let at = self.f.val(format!("mul i64 {i}, {in_size}"));
        let src = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        let item = self.at(&element, src, false);
        let operand = self.arg(&element, item);
        let result = self.call(&fp, &out, vec![operand]);
        let to = self.f.val(format!("mul i64 {i}, {}", out_layout.size));
        let dst = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {buf}, i64 {to}"));
        self.store_value(&out, &dst, result);
        let next = self.f.val(format!("add i64 {i}, 1"));
        self.f
            .ins(format!("store i64 {next}, ptr {counter}, align 8"));
        self.f.br(&head);
        self.f.place(&exit);
        let t = self.temp(&Ty::List(Box::new(out)));
        self.f.ins(format!("store ptr {buf}, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 {len}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 {len}, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }
