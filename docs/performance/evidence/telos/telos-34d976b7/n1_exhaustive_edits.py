EDITS = [
("""//! one column per element. `()` covers `Unit`. Every other type (`Int`,
//! `Byte`, `Text`, records, lists) is covered only by a `_` or a binding:
//! a literal, constant or list pattern covers some values, never all.""",
"""//! one column per element. `()` covers `Unit`. An `Int` or `Byte` column
//! with a range pattern is split into the intervals between the bounds of
//! its literal, constant and range patterns (`docs/slice.md` section 13.7).
//! Every other type (`Text`, records, lists, and `Int` or `Byte` without a
//! range) is covered only by a `_` or a binding: a literal, constant or
//! list pattern covers some values, never all."""),
("""    pub(crate) is_const: &'a dyn Fn(&Pattern) -> bool,
}
""",
"""    pub(crate) is_const: &'a dyn Fn(&Pattern) -> bool,
    /// The inclusive interval an integer literal, constant or range
    /// pattern matches.
    pub(crate) interval: &'a dyn Fn(&Pattern) -> Option<(i64, i64)>,
}
"""),
("""    Variants(QualName, Variants),
    Bool,
}
""",
"""    Variants(QualName, Variants),
    Bool,
    /// Disjoint inclusive intervals covering an `Int` or `Byte` column.
    Intervals(Vec<(i64, i64)>),
}

/// The intervals `bounds` cut the values `min..=max` into.
fn intervals(min: i64, max: i64, bounds: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut cuts: Vec<i128> = vec![i128::from(min), i128::from(max) + 1];
    for (lo, hi) in bounds {
        cuts.push(i128::from(*lo).clamp(i128::from(min), i128::from(max) + 1));
        cuts.push((i128::from(*hi) + 1).clamp(i128::from(min), i128::from(max) + 1));
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|w| (w[0] as i64, (w[1] - 1) as i64))
        .collect()
}
"""),
("""            Split::Bool => vec![("true".to_string(), vec![]), ("false".to_string(), vec![])],
        }
    }""",
"""            Split::Bool => vec![("true".to_string(), vec![]), ("false".to_string(), vec![])],
            Split::Intervals(cases) => cases
                .iter()
                .map(|(lo, hi)| {
                    let name = if lo == hi {
                        lo.to_string()
                    } else {
                        format!("{lo}..={hi}")
                    };
                    (name, vec![])
                })
                .collect(),
        }
    }"""),
("""    fn args<'p>(&self, pattern: &'p Pattern, case: &str) -> Option<&'p [Pattern]> {
        match (self, &pattern.kind) {""",
"""    fn args<'p>(&self, pattern: &'p Pattern, case: &str, cx: &Context) -> Option<&'p [Pattern]> {
        if let Split::Intervals(cases) = self {
            let (lo, hi) = cases.iter().copied().find(|(lo, hi)| {
                if lo == hi {
                    lo.to_string() == case
                } else {
                    format!("{lo}..={hi}") == case
                }
            })?;
            let (start, end) = (cx.interval)(pattern)?;
            return (start <= lo && hi <= end).then_some(&[]);
        }
        match (self, &pattern.kind) {"""),
("""    let split = match ty {
        _ if only_catch_all => None,
        Ty::Bool => Some(Split::Bool),
        _ => (cx.variants)(ty).map(|(q, v)| Split::Variants(q, v)),
    };""",
"""    let ranged = rows
        .iter()
        .any(|row| matches!(row[0].kind, PatternKind::Range { .. }));
    let split = match ty {
        _ if only_catch_all => None,
        Ty::Bool => Some(Split::Bool),
        Ty::Int | Ty::Byte if ranged => {
            let (min, max) = if *ty == Ty::Byte {
                (0, 255)
            } else {
                (i64::MIN, i64::MAX)
            };
            let bounds: Vec<(i64, i64)> = rows.iter().filter_map(|row| (cx.interval)(row[0])).collect();
            Some(Split::Intervals(intervals(min, max, &bounds)))
        }
        _ => (cx.variants)(ty).map(|(q, v)| Split::Variants(q, v)),
    };"""),
("""                    let args = split.args(row[0], &name)?;""",
"""                    let args = split.args(row[0], &name, cx)?;"""),
]
