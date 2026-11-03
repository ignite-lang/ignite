EDITS = [
# fields
("""    closure_frames: Vec<(usize, Vec<Capture>)>,
}

impl<'a> Typer<'a> {""",
"""    closure_frames: Vec<(usize, Vec<Capture>)>,
    /// Set by a value whose type parameter neither the expected type nor
    /// its arguments decide (`[]`, `None`, `List.empty()`, ..): the body is
    /// walked again to let later uses decide (`docs/slice.md` section 13,
    /// row 3).
    hole_wanted: bool,
    /// During that probe walk: the open holes and what uses bound them.
    holes: Option<Holes>,
    /// `let` bindings whose type the probe walk decided, by name span.
    let_hints: HashMap<Span, Ty>,
}

/// Inference holes of the row-3 probe walk: `'?n` type variables standing
/// for an undecided type parameter, and the types uses bound them to.
#[derive(Default)]
struct Holes {
    next: usize,
    solved: BTreeMap<String, Ty>,
    /// The name span and type of each `let` whose value has a hole.
    lets: Vec<(Span, Ty)>,
}

fn is_hole(name: &str) -> bool {
    name.starts_with("'?")
}

impl<'a> Typer<'a> {"""),
("""            binding_spans: HashMap::new(),
            closure_frames: Vec::new(),
        }
    }""",
"""            binding_spans: HashMap::new(),
            closure_frames: Vec::new(),
            hole_wanted: false,
            holes: None,
            let_hints: HashMap::new(),
        }
    }

    /// Walk a body with `walk`; when a value left a type parameter
    /// undecided, walk it again with holes to learn the binding types later
    /// uses decide, then a third time with those types as if annotated.
    fn with_local_inference(&mut self, walk: &dyn Fn(&mut Self)) {
        let diagnostics = self.diagnostics.len();
        let tests = self.tests.len();
        self.hole_wanted = false;
        walk(self);
        if !self.hole_wanted {
            return;
        }
        self.hole_wanted = false;
        self.diagnostics.truncate(diagnostics);
        self.tests.truncate(tests);
        self.holes = Some(Holes::default());
        walk(self);
        let holes = self.holes.take().unwrap_or_default();
        self.diagnostics.truncate(diagnostics);
        self.tests.truncate(tests);
        for (span, ty) in &holes.lets {
            let mut ty = ty.clone();
            for _ in 0..=holes.solved.len() {
                ty = apply(&ty, &holes.solved);
            }
            if !ty.has_param(&is_hole) {
                self.let_hints.insert(*span, ty);
            }
        }
        walk(self);
        self.let_hints.clear();
        self.hole_wanted = false;
    }

    /// A fresh hole during the probe walk; `None` outside it, after noting
    /// that a probe walk is wanted.
    fn hole(&mut self) -> Option<Ty> {
        match &mut self.holes {
            Some(holes) => {
                holes.next += 1;
                Some(Ty::Param(format!("'?{}", holes.next)))
            }
            None => {
                self.hole_wanted = true;
                None
            }
        }
    }"""),
# bodies: wrap function and test
("""                Decl::Test(test) => self.test(index, test),
                Decl::Fn(f) => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {
                        self.function(f, &sig);
                    }
                }""",
"""                Decl::Test(test) => self.with_local_inference(&|t| t.test(index, test)),
                Decl::Fn(f) => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {
                        self.with_local_inference(&|t| t.function(f, &sig));
                    }
                }"""),
# check: unify holes
("""    fn check(&mut self, expr: &Expr, expected: &Ty) -> Option<Ty> {
        let found = self.expr(expr, Mode::Value(Some(expected)));
        if let Some(found) = &found""",
"""    fn check(&mut self, expr: &Expr, expected: &Ty) -> Option<Ty> {
        let found = self.expr(expr, Mode::Value(Some(expected)));
        if let (Some(holes), Some(found)) = (&mut self.holes, &found) {
            let mut binds = BTreeMap::new();
            unify(expected, found, &mut binds);
            unify(found, expected, &mut binds);
            for (name, ty) in binds {
                if is_hole(&name) {
                    holes.solved.entry(name).or_insert(ty);
                }
            }
        }
        if let Some(found) = &found"""),
# let
("""                    None => self.synth(value),
                };
                self.bind(name, ty);
                Some(Ty::Unit)""",
"""                    None => match self.let_hints.get(&name.span).cloned() {
                        Some(hint) => {
                            self.check(value, &hint);
                            Some(hint)
                        }
                        None => {
                            let ty = self.synth(value);
                            if let (Some(holes), Some(ty)) = (&mut self.holes, &ty)
                                && ty.has_param(&is_hole)
                            {
                                holes.lets.push((name.span, ty.clone()));
                            }
                            ty
                        }
                    },
                };
                self.bind(name, ty);
                Some(Ty::Unit)"""),
# sites
("""                None => self.error(
                    "E042",
                    expr.span,
                    "cannot infer the element type of `[]`; add a type annotation".to_string(),
                ),
            }
            return None;""",
"""                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::List(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `[]`; add a type annotation".to_string(),
                    )
                }
            }
            return None;"""),
("""                None => {
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `List.empty()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }""",
"""                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::List(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `List.empty()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }"""),
("""                None => {
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `Store.new()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }""",
"""                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::Store(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `Store.new()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }"""),
("""                    None => {
                        self.error(
                            "E042",
                            expr.span,
                            "cannot infer the type of `None`; add a type annotation".to_string(),
                        );
                        None
                    }""",
"""                    None => {
                        if let Some(hole) = self.hole() {
                            return Some(Ty::Option(Box::new(hole)));
                        }
                        self.error(
                            "E042",
                            expr.span,
                            "cannot infer the type of `None`; add a type annotation".to_string(),
                        );
                        None
                    }"""),
("""                None => {
                    for arg in args {
                        self.synth(arg);
                    }
                    let other = if name == "Ok" { "error" } else { "success" };""",
"""                None => {
                    let mut known = Vec::new();
                    for arg in args {
                        known.push(self.synth(arg));
                    }
                    if self.holes.is_some()
                        && let [Some(known)] = known.as_slice()
                    {
                        let known = known.clone();
                        let hole = self.hole()?;
                        return Some(if name == "Ok" {
                            Ty::Result(Box::new(known), Box::new(hole))
                        } else {
                            Ty::Result(Box::new(hole), Box::new(known))
                        });
                    }
                    self.hole_wanted = true;
                    let other = if name == "Ok" { "error" } else { "success" };"""),
]
