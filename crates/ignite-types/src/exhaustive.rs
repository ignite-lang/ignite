//! Match exhaustiveness (`docs/slice.md` sections 3 and 10, E046).
//!
//! The arms' patterns form a matrix with one column per value still to be
//! matched; [`uncovered`] returns the values no row matches, written as
//! patterns. An or-pattern stands for one row per alternative. A type with
//! variants (an enum, `Option`, `Result`, `Input`) is split into its
//! variants and `Bool` into `true` and `false`, each column of a payload
//! checked in turn, so nested payload patterns count. A tuple is split into
//! one column per element. `()` covers `Unit`. An `Int` or `Byte` column
//! with a range pattern is split into the intervals between the bounds of
//! its literal, constant and range patterns (`docs/slice.md` section 13.7).
//! A list (and `Bytes`) is split by length, each length a tuple of its
//! elements (`docs/slice.md` 20.5). Every other type (`Text`, records, and
//! `Int` or `Byte` without a range) is covered only by a `_` or a binding:
//! a literal or constant pattern covers some values, never all.

use ignite_syntax::ast::{FileId, NodeId, Pattern, PatternKind, Span};

use crate::{QualName, Ty, Variants};

static WILDCARD: Pattern = Pattern {
    kind: PatternKind::Wildcard,
    span: Span {
        start: 0,
        end: 0,
        file: FileId::NONE,
    },
    id: NodeId::DETACHED,
};

/// The most uncovered values one diagnostic lists.
const SHOWN: usize = 8;

/// What the matrix needs to know about the program.
pub(crate) struct Context<'a> {
    /// The variants of a type that has them.
    pub(crate) variants: &'a dyn Fn(&Ty) -> Option<(QualName, Variants)>,
    /// Whether a binding pattern names a constant: it then matches one
    /// value, not every value.
    pub(crate) is_const: &'a dyn Fn(&Pattern) -> bool,
    /// The inclusive interval an integer literal, constant or range
    /// pattern matches.
    pub(crate) interval: &'a dyn Fn(&Pattern) -> Option<(i64, i64)>,
}

impl Context<'_> {
    fn is_catch_all(&self, pattern: &Pattern) -> bool {
        match &pattern.kind {
            PatternKind::Wildcard => true,
            PatternKind::Binding(_) => !(self.is_const)(pattern),
            PatternKind::Tuple(items) => items.is_empty(),
            _ => false,
        }
    }

    /// Whether `pattern` matches every value of its type: a catch-all, a
    /// tuple of such patterns, or an or-pattern with such an alternative.
    fn irrefutable(&self, pattern: &Pattern) -> bool {
        match &pattern.kind {
            PatternKind::Tuple(items) => items.iter().all(|item| self.irrefutable(item)),
            PatternKind::Or(alternatives) => alternatives.iter().any(|alt| self.irrefutable(alt)),
            _ => self.is_catch_all(pattern),
        }
    }
}

/// How a column of a type with finitely many constructors splits.
enum Split {
    /// An enum, `Option`, `Result` or `Input`.
    Variants(QualName, Variants),
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

impl Split {
    fn cases(&self) -> Vec<(String, Vec<Ty>)> {
        match self {
            Split::Variants(_, variants) => variants.clone(),
            Split::Bool => vec![("true".to_string(), vec![]), ("false".to_string(), vec![])],
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
    }

    /// The payload patterns of `pattern` when it names case `case`.
    fn args<'p>(&self, pattern: &'p Pattern, case: &str, cx: &Context) -> Option<&'p [Pattern]> {
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
        match (self, &pattern.kind) {
            (Split::Variants(enum_, _), PatternKind::Variant { path, args }) => {
                let named = match path.as_slice() {
                    [v] => v.name == case,
                    [owner, v] => owner.name == enum_.name && v.name == case,
                    _ => false,
                };
                named.then_some(args.as_slice())
            }
            (Split::Bool, PatternKind::Bool(value)) => (value.to_string() == case).then_some(&[]),
            _ => None,
        }
    }

    /// `Command.Echo(_)` for a user enum, `Some(_)` for a builtin one,
    /// `true` for a `Bool`.
    fn show(&self, case: &str, payload: &[String]) -> String {
        let head = match self {
            Split::Variants(enum_, _) if !enum_.is_builtin() => format!("{}.{case}", enum_.name),
            _ => case.to_string(),
        };
        if payload.is_empty() {
            head
        } else {
            format!("{head}({})", payload.join(", "))
        }
    }
}

/// `rows` with every or-pattern in the first column replaced by one row
/// per alternative.
fn expand_or<'p>(rows: &[Vec<&'p Pattern>]) -> Vec<Vec<&'p Pattern>> {
    let mut out = Vec::new();
    for row in rows {
        match &row[0].kind {
            PatternKind::Or(alternatives) => {
                let expanded: Vec<Vec<&Pattern>> = alternatives
                    .iter()
                    .map(|alt| {
                        std::iter::once(alt)
                            .chain(row[1..].iter().copied())
                            .collect()
                    })
                    .collect();
                out.extend(expand_or(&expanded));
            }
            _ => out.push(row.clone()),
        }
    }
    out
}

/// The values of types `tys` that no row of `rows` matches, one pattern
/// per column. A first row that matches every value covers everything,
/// which keeps a matrix of literal and `_` columns polynomial.
pub(crate) fn uncovered(rows: &[Vec<&Pattern>], tys: &[Ty], cx: &Context) -> Vec<Vec<String>> {
    let Some((ty, rest)) = tys.split_first() else {
        return if rows.is_empty() {
            vec![Vec::new()]
        } else {
            Vec::new()
        };
    };
    if *ty == Ty::Never {
        return Vec::new();
    }
    if rows
        .first()
        .is_some_and(|row| row.iter().all(|p| cx.irrefutable(p)))
    {
        return Vec::new();
    }
    let rows = expand_or(rows);
    let wild = |n: usize| vec!["_".to_string(); n];
    if let Ty::Tuple(elements) = ty {
        let n = elements.len();
        let specialised: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter_map(|row| {
                let head: Vec<&Pattern> = match &row[0].kind {
                    PatternKind::Tuple(items) if items.len() == n => items.iter().collect(),
                    _ if cx.is_catch_all(row[0]) => vec![&WILDCARD; n],
                    _ => return None,
                };
                Some(head.into_iter().chain(row[1..].iter().copied()).collect())
            })
            .collect();
        let columns: Vec<Ty> = elements.iter().chain(rest).cloned().collect();
        return uncovered(&specialised, &columns, cx)
            .into_iter()
            .map(|witness| {
                let (inner, tail) = witness.split_at(n);
                std::iter::once(format!("({})", inner.join(", ")))
                    .chain(tail.iter().cloned())
                    .collect()
            })
            .collect();
    }
    // A column of catch-alls only is not split: no row tells its cases
    // apart, and a split would only multiply the rows.
    let only_catch_all = !rows.is_empty() && rows.iter().all(|row| cx.is_catch_all(row[0]));
    if let Ty::List(element) = ty
        && !only_catch_all
    {
        return uncovered_list(&rows, element, rest, cx);
    }
    let ranged = rows
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
            let bounds: Vec<(i64, i64)> = rows
                .iter()
                .filter_map(|row| (cx.interval)(row[0]))
                .collect();
            Some(Split::Intervals(intervals(min, max, &bounds)))
        }
        _ => (cx.variants)(ty).map(|(q, v)| Split::Variants(q, v)),
    };
    let Some(split) = split else {
        let defaults: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter(|row| cx.is_catch_all(row[0]))
            .map(|row| row[1..].to_vec())
            .collect();
        if defaults.is_empty() {
            let mut witness = wild(1);
            witness.extend(wild(rest.len()));
            return vec![witness];
        }
        return uncovered(&defaults, rest, cx)
            .into_iter()
            .map(|w| std::iter::once("_".to_string()).chain(w).collect())
            .collect();
    };
    let mut out = Vec::new();
    for (name, payload) in split.cases() {
        let specialised: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter_map(|row| {
                let head: Vec<&Pattern> = if cx.is_catch_all(row[0]) {
                    vec![&WILDCARD; payload.len()]
                } else {
                    let args = split.args(row[0], &name, cx)?;
                    if args.len() == payload.len() {
                        args.iter().collect()
                    } else {
                        vec![&WILDCARD; payload.len()]
                    }
                };
                Some(head.into_iter().chain(row[1..].iter().copied()).collect())
            })
            .collect();
        if specialised.is_empty() {
            let mut witness = vec![split.show(&name, &wild(payload.len()))];
            witness.extend(wild(rest.len()));
            out.push(witness);
            continue;
        }
        let columns: Vec<Ty> = payload.iter().chain(rest).cloned().collect();
        for witness in uncovered(&specialised, &columns, cx) {
            let (inner, tail) = witness.split_at(payload.len());
            let mut row = vec![split.show(&name, inner)];
            row.extend(tail.iter().cloned());
            out.push(row);
        }
    }
    out
}

/// A list pattern's elements before its `..`, after it, and whether it
/// has one.
fn list_shape(items: &[Pattern]) -> (&[Pattern], &[Pattern], bool) {
    match items
        .iter()
        .position(|item| matches!(item.kind, PatternKind::Rest(_)))
    {
        Some(at) => (&items[..at], &items[at + 1..], true),
        None => (items, &[], false),
    }
}

/// [`uncovered`] on a first column of type `List<element>` (`Bytes` is
/// `List<Byte>`), split by length (`docs/slice.md` 20.5): with `m` the
/// longest exact pattern and `p`, `s` the longest prefix and suffix of a
/// rest pattern, every length up to `max(m, p + s - 1)` is checked on its
/// own as a tuple of that many elements, and `max(m + 1, p + s)` stands
/// for every longer length. A witness at that length prints its first
/// `representative - s` elements, `..`, then its last `s`, so it names
/// every length it stands for.
fn uncovered_list(
    rows: &[Vec<&Pattern>],
    element: &Ty,
    rest: &[Ty],
    cx: &Context,
) -> Vec<Vec<String>> {
    let (mut exact, mut prefix, mut suffix) = (0, 0, 0);
    for row in rows {
        if let PatternKind::List(items) = &row[0].kind {
            let (before, after, has_rest) = list_shape(items);
            if has_rest {
                prefix = prefix.max(before.len());
                suffix = suffix.max(after.len());
            } else {
                exact = exact.max(items.len());
            }
        }
    }
    let longest = exact.max((prefix + suffix).saturating_sub(1));
    let representative = (exact + 1).max(prefix + suffix);
    let mut out = Vec::new();
    for length in (0..=longest).chain(std::iter::once(representative)) {
        let specialised: Vec<Vec<&Pattern>> = rows
            .iter()
            .filter_map(|row| {
                let head: Vec<&Pattern> = match &row[0].kind {
                    PatternKind::List(items) => {
                        let (before, after, has_rest) = list_shape(items);
                        if !has_rest && items.len() == length {
                            items.iter().collect()
                        } else if has_rest && before.len() + after.len() <= length {
                            let middle = length - before.len() - after.len();
                            before
                                .iter()
                                .chain(std::iter::repeat_n(&WILDCARD, middle))
                                .chain(after.iter())
                                .collect()
                        } else {
                            return None;
                        }
                    }
                    _ if cx.is_catch_all(row[0]) => vec![&WILDCARD; length],
                    _ => return None,
                };
                Some(head.into_iter().chain(row[1..].iter().copied()).collect())
            })
            .collect();
        let witnesses = if specialised.is_empty() {
            vec![vec!["_".to_string(); length + rest.len()]]
        } else {
            let columns: Vec<Ty> = std::iter::repeat_n(element.clone(), length)
                .chain(rest.iter().cloned())
                .collect();
            uncovered(&specialised, &columns, cx)
        };
        for witness in witnesses {
            let (inner, tail) = witness.split_at(length);
            let shown = if length == representative {
                let (front, back) = inner.split_at(representative - suffix);
                let items: Vec<&str> = front
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(".."))
                    .chain(back.iter().map(String::as_str))
                    .collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("[{}]", inner.join(", "))
            };
            out.push(std::iter::once(shown).chain(tail.iter().cloned()).collect());
        }
    }
    out
}

/// The E046 message for a `match` on `ty` whose arms leave `missing`
/// uncovered, or `None` when they cover every value.
pub(crate) fn message(ty: &Ty, missing: &[Vec<String>]) -> Option<String> {
    if missing.is_empty() {
        return None;
    }
    if missing.len() == 1 && missing[0].first().is_some_and(|w| w == "_") {
        return Some(format!(
            "match on `{ty}` does not cover every value; add a `_` arm or a binding arm"
        ));
    }
    let mut shown: Vec<String> = missing
        .iter()
        .take(SHOWN)
        .map(|w| format!("`{}`", w.join(", ")))
        .collect();
    if missing.len() > SHOWN {
        shown.push(format!("and {} more", missing.len() - SHOWN));
    }
    Some(format!("match does not cover {}", shown.join(", ")))
}
