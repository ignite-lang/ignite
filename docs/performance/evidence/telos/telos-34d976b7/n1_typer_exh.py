EDITS = [
("""        let consts = &self.pattern_consts;
        let cx = exhaustive::Context {
            variants: &|t| self.variants(t),
            is_const: &|p| consts.contains_key(&(p as *const Pattern as usize)),
        };""",
"""        let consts = &self.pattern_consts;
        let interval = |p: &Pattern| match &p.kind {
            PatternKind::Int(v) => Some((*v, *v)),
            PatternKind::Range { .. } => self.ranges.get(&(p as *const Pattern as usize)).copied(),
            _ => {
                let v = self.const_value(consts.get(&(p as *const Pattern as usize))?)?;
                Some((v, v))
            }
        };
        let cx = exhaustive::Context {
            variants: &|t| self.variants(t),
            is_const: &|p| consts.contains_key(&(p as *const Pattern as usize)),
            interval: &interval,
        };"""),
]
