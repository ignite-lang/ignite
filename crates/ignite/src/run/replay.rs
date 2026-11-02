//! Replay (`docs/slice.md` 16.15): the one fn `ignite replay`, a resumed
//! `ignite run` and the end-to-end tests share. It walks the journal's
//! records after the header in `seq` order: a `segment` is read for its
//! generation and declared members; every `event` record that is not an
//! `Emitted` event is re-stepped through the [`Core`], and the records
//! the core produces (the event itself, its effects, signals, standings,
//! consequences, errors and the `Emitted` events of subscribers with
//! their steps) must equal the journaled lines byte for byte. The first
//! difference is E127 with its `seq`. The values a real process
//! contributes (`pid`, `latency`, `code`) enter only through `event`
//! records, so replay is exact (A13).
//!
//! A journal that ends in the middle of a step (a crash between two
//! records) replays up to its last line: the records the core produced
//! past the end are not an error, and a resumed run writes nothing for
//! them either, because its first record is a new `segment`.

use ignite_eval::{CONTROL_MODULE, Json, Value};
use ignite_types::{QualName, Ty, TypedProgram};

use super::engine::{Core, enum_parts, field, int};

/// Why a journal did not replay (E126 for the header, E127 for the rest).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// A line that is no record of 16.5, with its `seq` (its position
    /// when it has no `seq`) and the reason.
    Unparsable { seq: u64, reason: String },
    /// The re-derived record at `seq` differs from the journaled one.
    Diverged {
        seq: u64,
        journaled: String,
        derived: String,
    },
}

impl ReplayError {
    /// The `seq` the diagnostic names.
    pub fn seq(&self) -> u64 {
        match self {
            ReplayError::Unparsable { seq, .. } | ReplayError::Diverged { seq, .. } => *seq,
        }
    }
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Unparsable { seq, reason } => {
                write!(f, "journal record {seq} does not parse: {reason}")
            }
            ReplayError::Diverged {
                seq,
                journaled,
                derived,
            } => write!(
                f,
                "journal record {seq} does not replay: journaled {journaled}, derived {derived}"
            ),
        }
    }
}

/// What a replay read from the journal besides the machines' state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replayed {
    /// The generation of the last `segment`, 1 without one.
    pub generation: i64,
    /// The declared members of the last `segment`.
    pub declared: Vec<String>,
    /// The `at` of the last record, 0 without one.
    pub last_at: i64,
    /// Records replayed, the header not counted.
    pub records: usize,
}

/// One parsed record line of 16.5.
struct Line {
    seq: u64,
    at: i64,
    kind: String,
    machine: Option<String>,
    value: Json,
}

fn parse_line(position: u64, line: &str) -> Result<Line, ReplayError> {
    let json = Json::parse(line).map_err(|reason| ReplayError::Unparsable {
        seq: position,
        reason,
    })?;
    let seq = json
        .get("seq")
        .and_then(Json::as_i64)
        .filter(|seq| *seq >= 0)
        .ok_or_else(|| ReplayError::Unparsable {
            seq: position,
            reason: "no `seq`".to_string(),
        })? as u64;
    let unparsable = |reason: &str| ReplayError::Unparsable {
        seq,
        reason: reason.to_string(),
    };
    if seq != position {
        return Err(unparsable(&format!("expected seq {position}")));
    }
    let at = json
        .get("at")
        .and_then(|at| at.get("ns"))
        .and_then(Json::as_i64)
        .ok_or_else(|| unparsable("no `at`"))?;
    let kind = json
        .get("kind")
        .and_then(Json::as_str)
        .ok_or_else(|| unparsable("no `kind`"))?
        .to_string();
    let machine = match json.get("machine") {
        Some(Json::Str(machine)) => Some(machine.clone()),
        Some(Json::Null) | None => None,
        Some(_) => return Err(unparsable("`machine` is neither a string nor null")),
    };
    let value = json
        .get("value")
        .cloned()
        .ok_or_else(|| unparsable("no `value`"))?;
    Ok(Line {
        seq,
        at,
        kind,
        machine,
        value,
    })
}

/// Re-step `lines`, the journal's records after the header in order,
/// through `core` (module doc). `types` is the typed program of
/// `std.control`, which decodes the journaled events.
pub fn replay(
    core: &mut Core<'_, '_>,
    types: &TypedProgram,
    lines: &[String],
) -> Result<Replayed, ReplayError> {
    let event_ty = Ty::Record(QualName::new(CONTROL_MODULE, "Event"));
    let mut replayed = Replayed {
        generation: 1,
        ..Replayed::default()
    };
    let mut i = 0;
    while i < lines.len() {
        let position = i as u64 + 1;
        let line = parse_line(position, &lines[i])?;
        replayed.last_at = line.at;
        replayed.records = i + 1;
        match line.kind.as_str() {
            "segment" => {
                if let Some(generation) = line
                    .value
                    .get("fields")
                    .and_then(|f| f.get("generation"))
                    .and_then(Json::as_i64)
                {
                    replayed.generation = generation;
                }
                if let Some(declared) = line
                    .value
                    .get("fields")
                    .and_then(|f| f.get("declared"))
                    .and_then(Json::as_array)
                {
                    replayed.declared = declared
                        .iter()
                        .filter_map(Json::as_str)
                        .map(str::to_string)
                        .collect();
                }
                core.set_next_seq(line.seq + 1);
                i += 1;
            }
            "event" => {
                let event = Value::decode_json(&line.value, &event_ty, types).map_err(|error| {
                    ReplayError::Unparsable {
                        seq: line.seq,
                        reason: error.message,
                    }
                })?;
                let kind =
                    field(&event, "kind")
                        .cloned()
                        .ok_or_else(|| ReplayError::Unparsable {
                            seq: line.seq,
                            reason: "an event without `kind`".to_string(),
                        })?;
                let at = field(&event, "at").and_then(int).unwrap_or(line.at);
                if enum_parts(&kind).map(|(variant, _)| variant) == Some("Emitted") {
                    return Err(ReplayError::Diverged {
                        seq: line.seq,
                        journaled: lines[i].clone(),
                        derived: "nothing: an `Emitted` event follows a `signal` record"
                            .to_string(),
                    });
                }
                let machine = line
                    .machine
                    .as_deref()
                    .and_then(|id| core.machine(id))
                    .ok_or_else(|| ReplayError::Unparsable {
                        seq: line.seq,
                        reason: format!(
                            "an event for `{}`, which is no machine of this model",
                            line.machine.as_deref().unwrap_or("null")
                        ),
                    })?;
                core.set_next_seq(line.seq);
                let produced = core.feed(machine, at, kind);
                for record in &produced {
                    let Some(journaled) = lines.get(i) else {
                        break;
                    };
                    let derived = record.line();
                    if *journaled != derived {
                        return Err(ReplayError::Diverged {
                            seq: record.seq,
                            journaled: journaled.clone(),
                            derived,
                        });
                    }
                    replayed.last_at = record.at;
                    replayed.records = i + 1;
                    i += 1;
                }
            }
            other => {
                return Err(ReplayError::Diverged {
                    seq: line.seq,
                    journaled: lines[i].clone(),
                    derived: format!("nothing: a `{other}` record follows its event"),
                });
            }
        }
    }
    Ok(replayed)
}
