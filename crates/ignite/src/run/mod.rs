//! The driver of `ignite run` and `ignite replay` (`docs/slice.md` section
//! 16): one thread steps the `std.control` reducers in the evaluator
//! (16.2, O27), the adapters turn their effects into processes, probe
//! attempts and timers and feed the results back as events (16.7 to
//! 16.9), the journal records every event, effect, signal, standing and
//! consequence (16.5), and replay re-steps the journal (16.15).
//!
//! Slice 12 (16.18, 16.20): `process`, `probe`, `clock`, `journal` and
//! `control` are P2's; `engine` (the deterministic core), `live` (the
//! adapter-facing loop), `replay` and this file P3's.

pub mod clock;
pub mod control;
pub mod engine;
pub mod journal;
pub mod live;
pub mod probe;
pub mod process;
pub mod replay;

/// The standing of one contract as `ignite replay` prints it and `status`
/// reports it (16.3, 16.10): the `std.control.Standing` value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
    pub contract: String,
    pub value: ignite_eval::Value,
}

/// The default epoch of the manual clock (16.4).
pub const DEFAULT_EPOCH: &str = "2026-01-01T00:00:00Z";

/// E142 (`docs/slice.md` 21.6, 21.9): the message for each `Setting`
/// without `default` that one of `started` (member ids) uses, directly or
/// through the members it uses; once per setting, in id order, naming the
/// first started member that reaches it. `ignite run` and `ignite apply`
/// pass no setting values (D12), so such a member would exit 2 at every
/// start.
pub fn settings_without_default<'a>(
    model: &ignite_eval::Model,
    started: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    let mut found: std::collections::BTreeMap<&str, (&str, &str)> = Default::default();
    for member in started {
        let mut seen = std::collections::BTreeSet::new();
        let mut pending = vec![member.as_str()];
        while let Some(from) = pending.pop() {
            for edge in &model.edges {
                if edge.from == from
                    && edge.kind == ignite_eval::EdgeKind::Uses
                    && seen.insert(edge.to.as_str())
                {
                    pending.push(&edge.to);
                }
            }
        }
        for id in seen {
            let Some(resource) = model.member(id) else {
                continue;
            };
            if resource.declaration != ignite_types::MemberDeclaration::Resource
                || resource.kind != "Setting"
                || resource
                    .properties
                    .iter()
                    .any(|(name, _)| name == "default")
            {
                continue;
            }
            let flag = resource
                .properties
                .iter()
                .find_map(|(name, value)| match (name.as_str(), value) {
                    ("flag", ignite_eval::Value::Text(flag)) => Some(flag.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            found.entry(&resource.id).or_insert((member, flag));
        }
    }
    found
        .into_iter()
        .map(|(setting, (member, flag))| {
            format!(
                "setting `{setting}` (`{flag}`) has no `default`, and `{member}` uses it: `ignite run` and `ignite apply` pass no setting values, so the member would exit 2 at every start; give it a `default`"
            )
        })
        .collect()
}

/// E142 for the members `ignite run` or `ignite plan --for-apply` would
/// start (`crate::run_members`), one diagnostic per setting
/// ([`settings_without_default`]); `Ok` when there is none.
pub(crate) fn refuse_settings_without_default(
    loaded: &crate::Loaded,
    model: &ignite_eval::Model,
) -> Result<(), crate::Error> {
    let (started, _) = crate::run_members(model);
    let found = settings_without_default(model, started.keys());
    if found.is_empty() {
        return Ok(());
    }
    Err(crate::diagnostics(
        loaded.source(),
        found
            .into_iter()
            .map(|message| {
                ignite_syntax::diagnostic::Diagnostic::new(
                    "E142",
                    ignite_syntax::ast::Span::new(0, 0),
                    message,
                )
            })
            .collect(),
    ))
}

/// An RFC 3339 instant (`2026-10-31T22:50:00Z`, `…T23:50:00+01:00`, an
/// optional fraction) as nanoseconds since 1970-01-01T00:00:00Z; `None`
/// when malformed.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let text = text.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[10] != b'T' && bytes[10] != b't' {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> {
        let slice = text.get(from..to)?;
        if slice.is_empty() || !slice.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        slice.parse().ok()
    };
    let year = number(0, 4)?;
    let month = number(5, 7)?;
    let day = number(8, 10)?;
    let hour = number(11, 13)?;
    let minute = number(14, 16)?;
    let second = number(17, 19)?;
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let mut index = 19;
    let mut fraction_ns = 0i64;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let digits = &text[start..index];
        if digits.is_empty() {
            return None;
        }
        let mut scale = 100_000_000;
        for digit in digits.bytes().take(9) {
            fraction_ns += i64::from(digit - b'0') * scale;
            scale /= 10;
        }
    }
    let offset_s = match bytes.get(index) {
        Some(b'Z') | Some(b'z') if index + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-')) if index + 6 == bytes.len() && bytes[index + 3] == b':' => {
            let hours = number(index + 1, index + 3)?;
            let minutes = number(index + 4, index + 6)?;
            let offset = hours * 3600 + minutes * 60;
            if *sign == b'+' { offset } else { -offset }
        }
        _ => return None,
    };
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_s;
    seconds.checked_mul(1_000_000_000)?.checked_add(fraction_ns)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::parse_rfc3339;

    #[test]
    fn rfc3339_instants() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339("2026-01-01T00:00:00Z"),
            Some(1_767_225_600_000_000_000)
        );
        assert_eq!(
            parse_rfc3339("2026-10-31T22:50:00Z"),
            Some(1_793_487_000_000_000_000)
        );
        assert_eq!(
            parse_rfc3339("2026-10-31T23:50:00+01:00"),
            parse_rfc3339("2026-10-31T22:50:00Z")
        );
        assert_eq!(
            parse_rfc3339("2026-10-31T22:50:00.5Z"),
            Some(1_793_487_000_500_000_000)
        );
        assert_eq!(parse_rfc3339("2026-10-31 22:50:00Z"), None);
        assert_eq!(parse_rfc3339("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("2026-10-31T22:50:00"), None);
    }
}
