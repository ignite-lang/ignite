//! Source-bound plans and a foreground local host. Workload effects remain
//! owned by the existing `run` driver and `std.control` reducers.

pub mod digest;
mod host;
pub mod plan;
mod revision;

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use ignite_eval::Json;

pub use host::run as host;

const PROTOCOL: &str = "ignite-host-control/0.1";
const MAX_FRAME: usize = 16 * 1024 * 1024;
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// An apply command's documented exit category and explanation.
#[derive(Debug)]
pub struct Failure {
    pub status: i32,
    pub reason: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for Failure {}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        fail(3, error.to_string())
    }
}

impl From<crate::Error> for Failure {
    fn from(error: crate::Error) -> Self {
        fail(2, error.to_string())
    }
}

fn fail(status: i32, reason: impl Into<String>) -> Failure {
    Failure {
        status,
        reason: reason.into(),
    }
}

fn object<const N: usize>(fields: [(&str, Json); N]) -> Json {
    Json::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn text(value: impl Into<String>) -> Json {
    Json::Str(value.into())
}

fn required<'a>(value: &'a Json, key: &str) -> Result<&'a str, Failure> {
    value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| fail(2, format!("missing or invalid {key}")))
}

/// The longest request id a host accepts and echoes.
const MAX_REQUEST_ID: usize = 128;

/// Why a control frame could not be read (17.2). `Closed` is a peer that
/// sent nothing before its end of file, or stalled or failed mid-frame,
/// which has nobody to answer; `Truncated` is a frame cut short by the
/// peer's end of file, whose sender may still read; it and the other two
/// are answered with status 2 before admission.
#[derive(Debug)]
enum FrameError {
    Closed(io::Error),
    Truncated { expected: usize, received: usize },
    Oversized(usize),
    Malformed(String),
}

impl FrameError {
    fn reason(&self) -> String {
        match self {
            FrameError::Closed(error) => format!("control connection ended mid-frame: {error}"),
            FrameError::Truncated { expected, received } => {
                format!("malformed control frame: cut short at {received} of {expected} bytes")
            }
            FrameError::Oversized(size) => {
                format!("control frame of {size} bytes exceeds the 16 MiB limit")
            }
            FrameError::Malformed(why) => format!("malformed control frame: {why}"),
        }
    }
}

/// One frame: a 4-byte big-endian length, then that many bytes of UTF-8
/// JSON. A frame over `MAX_FRAME` is refused without allocating it.
fn encode_frame(value: &Json) -> Result<Vec<u8>, FrameError> {
    let body = value.compact().into_bytes();
    if body.len() > MAX_FRAME {
        return Err(FrameError::Oversized(body.len()));
    }
    let mut bytes = Vec::with_capacity(4 + body.len());
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&body);
    Ok(bytes)
}

fn write_frame(stream: &mut UnixStream, value: &Json) -> io::Result<()> {
    let bytes = encode_frame(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.reason()))?;
    stream.write_all(&bytes)
}

/// Read until `buffer` is full or the peer's end of file; the count read.
fn fill(stream: &mut UnixStream, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match stream.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

fn read_frame(stream: &mut UnixStream) -> Result<Json, FrameError> {
    let mut size = [0; 4];
    match fill(stream, &mut size).map_err(FrameError::Closed)? {
        0 => {
            return Err(FrameError::Closed(io::Error::from(
                io::ErrorKind::UnexpectedEof,
            )));
        }
        4 => {}
        received => {
            return Err(FrameError::Truncated {
                expected: 4,
                received,
            });
        }
    }
    let size = u32::from_be_bytes(size) as usize;
    if size > MAX_FRAME {
        return Err(FrameError::Oversized(size));
    }
    let mut bytes = vec![0; size];
    let received = fill(stream, &mut bytes).map_err(FrameError::Closed)?;
    if received < size {
        return Err(FrameError::Truncated {
            expected: 4 + size,
            received: 4 + received,
        });
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| FrameError::Malformed(format!("not UTF-8: {error}")))?;
    Json::parse(source).map_err(|why| FrameError::Malformed(format!("not JSON: {why}")))
}

/// A request envelope that passed the checks every request passes before
/// it reaches the host queue.
struct Envelope<'a> {
    id: &'a str,
    operation: &'a str,
    identity: &'a str,
}

/// Check the envelope of a request: an object with this protocol's
/// `version`, a non-empty `requestId`, an `operation` and a
/// `stateIdentity`. `Err` carries the request id when one could be read,
/// otherwise the empty id, and the status-2 rejection.
fn envelope(request: &Json) -> Result<Envelope<'_>, (String, Failure)> {
    let id = request
        .get("requestId")
        .and_then(Json::as_str)
        .filter(|id| !id.is_empty() && id.len() <= MAX_REQUEST_ID);
    let reject = |reason: String| (id.unwrap_or("").to_string(), fail(2, reason));
    if request.as_object().is_none() {
        return Err(reject("malformed control frame: not a JSON object".into()));
    }
    let id = id.ok_or_else(|| {
        reject(format!(
            "malformed control frame: requestId must be a string of 1 to {MAX_REQUEST_ID} bytes"
        ))
    })?;
    match request.get("version").and_then(Json::as_str) {
        Some(PROTOCOL) => {}
        Some(other) => {
            return Err(reject(format!(
                "unsupported control protocol version {other}; this host speaks {PROTOCOL}"
            )));
        }
        None => {
            return Err(reject(format!(
                "malformed control frame: missing version; this host speaks {PROTOCOL}"
            )));
        }
    }
    let field = |key: &str| {
        request
            .get(key)
            .and_then(Json::as_str)
            .ok_or_else(|| reject(format!("malformed control frame: missing or invalid {key}")))
    };
    Ok(Envelope {
        id,
        operation: field("operation")?,
        identity: field("stateIdentity")?,
    })
}

/// Linux resolves this short pathname through an open state directory. The
/// visible socket stays at `<state>/control.sock`, including when the absolute
/// state pathname exceeds the Unix socket address limit.
fn socket_address(state: &Path) -> io::Result<(std::fs::File, std::path::PathBuf)> {
    let directory = std::fs::File::open(state)?;
    let address = std::path::PathBuf::from(format!(
        "/proc/self/fd/{}/control.sock",
        directory.as_raw_fd()
    ));
    Ok((directory, address))
}

fn connect(state: &Path) -> io::Result<UnixStream> {
    let (_directory, address) = socket_address(state)?;
    UnixStream::connect(address)
}

fn request(state: &Path, operation: &str, identity: &str, payload: Json) -> Result<Json, Failure> {
    let id = format!(
        "{}-{}",
        std::process::id(),
        NEXT_REQUEST.fetch_add(1, Ordering::Relaxed)
    );
    let value = object([
        ("version", text(PROTOCOL)),
        ("requestId", text(&id)),
        ("stateIdentity", text(identity)),
        ("operation", text(operation)),
        ("payload", payload),
    ]);
    let frame = encode_frame(&value).map_err(|error| fail(2, error.reason()))?;
    let mut stream = connect(state).map_err(|error| {
        fail(
            3,
            format!(
                "no host answers on {}: {error}; start `ignite host --state {}`",
                state.join("control.sock").display(),
                state.display()
            ),
        )
    })?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    stream.write_all(&frame)?;
    let response = read_frame(&mut stream)
        .map_err(|error| fail(3, format!("host reply: {}", error.reason())))?;
    reply_result(&response, &id)
}

/// Map a host reply to the request `id` onto the exit categories of 17.1.
/// A host that rejected a frame before it could read the request id
/// answers with the empty id; that is the host's rejection, not a reply to
/// another request.
fn reply_result(response: &Json, id: &str) -> Result<Json, Failure> {
    let version = response.get("version").and_then(Json::as_str);
    if version != Some(PROTOCOL) {
        return Err(fail(
            3,
            format!(
                "host replied with control protocol {}; this client speaks {PROTOCOL}",
                version.unwrap_or("(none)")
            ),
        ));
    }
    let status = response.get("status").and_then(Json::as_i64);
    let reply_id = response
        .get("requestId")
        .and_then(Json::as_str)
        .unwrap_or("");
    let rejected_unread = reply_id.is_empty() && matches!(status, Some(1..=3));
    if reply_id != id && !rejected_unread {
        return Err(fail(
            3,
            format!("host reply carries request id {reply_id:?}, expected {id:?}"),
        ));
    }
    let reason = response
        .get("reason")
        .and_then(Json::as_str)
        .unwrap_or("host gave no reason");
    match status {
        Some(0) => Ok(response.get("payload").cloned().unwrap_or(Json::Null)),
        Some(status @ 1..=3) => Err(fail(status as i32, reason)),
        other => Err(fail(3, format!("host reply has invalid status {other:?}"))),
    }
}

/// Prepare all immutable artifacts in the live host, then save its admitted
/// manifest. No workload is started while preparing a plan.
pub fn prepare(
    file: &Path,
    state: &Path,
    out: &Path,
    options: &crate::PlanOptions,
    build: &crate::RunOptions,
) -> Result<Json, Failure> {
    plan::refuse_settings_without_default(file, options)?;
    let snapshot = request(state, "snapshot", "", Json::Null)?;
    let identity = required(&snapshot, "stateIdentity")?;
    let fuel = options
        .fuel
        .map(i64::try_from)
        .transpose()
        .map_err(|_| fail(2, "--fuel exceeds the saved-plan integer range"))?;
    let nullable = |path: Option<&Path>| -> Result<Json, Failure> {
        path.map(|path| std::fs::canonicalize(path).map(|p| text(p.to_string_lossy())))
            .transpose()
            .map(|value| value.unwrap_or(Json::Null))
            .map_err(Into::into)
    };
    let payload = object([
        (
            "source",
            text(std::fs::canonicalize(file)?.to_string_lossy()),
        ),
        (
            "assembly",
            options.assembly.as_ref().map_or(Json::Null, text),
        ),
        ("fuel", fuel.map_or(Json::Null, Json::Int)),
        ("std", nullable(options.std.as_deref())?),
        ("runtime", nullable(build.runtime.as_deref())?),
        (
            "target",
            text(if build.target == crate::Target::Rust {
                "rust"
            } else {
                "llvm"
            }),
        ),
        ("release", Json::Bool(build.release)),
    ]);
    let result = request(state, "prepare", identity, payload)?;
    std::fs::write(out, result.compact() + "\n")?;
    Ok(result)
}

/// Submit a saved plan and wait for its existing or newly accepted operation.
/// Exit 0 only for a completed receipt (17.1); a receipt in any other state
/// is exit 1, the host's rejections keep their own category.
pub fn apply(file: &Path, state: &Path) -> Result<Json, Failure> {
    let value = Json::parse(&std::fs::read_to_string(file)?).map_err(|reason| fail(2, reason))?;
    let plan = plan::Plan::parse(&value).map_err(|reason| fail(2, reason))?;
    let receipt = request(state, "apply", &plan.state_identity, value)?;
    match receipt.get("state").and_then(Json::as_str) {
        Some("completed") => Ok(receipt),
        other => Err(fail(
            1,
            format!(
                "operation {} is {}: {}; run `ignite inspect --state {} --operation {}`",
                plan.id,
                other.unwrap_or("in an unknown state"),
                receipt
                    .get("reason")
                    .and_then(Json::as_str)
                    .unwrap_or("no reason recorded"),
                state.display(),
                plan.id
            ),
        )),
    }
}

/// Ask the host of `state` to recover `operation` (`docs/slice.md` 22.9)
/// and wait until the committed revision it restarts is ready: the host's
/// payload (`operationId`, `stopped`, `committedRevision`,
/// `committedPlan`). Exit 1 when the host refuses an unaccounted process or
/// the revision is not ready, 2 when the operation requires no recovery.
pub fn recover(state: &Path, operation: &str) -> Result<Json, Failure> {
    let snapshot = request(state, "snapshot", "", Json::Null)?;
    let identity = required(&snapshot, "stateIdentity")?;
    request(
        state,
        "recover",
        identity,
        object([("operationId", text(operation))]),
    )
}

/// The line `ignite recover` prints for the host's payload.
pub fn recovered_line(payload: &Json) -> String {
    let stopped = payload
        .get("stopped")
        .and_then(Json::as_array)
        .map_or(0, <[Json]>::len);
    let operation = payload
        .get("operationId")
        .and_then(Json::as_str)
        .unwrap_or_default();
    match payload.get("committedPlan").and_then(Json::as_str) {
        Some(_) => format!(
            "recovered {operation}: stopped {stopped} instances; committed revision {} restarted",
            payload
                .get("committedRevision")
                .and_then(Json::as_i64)
                .unwrap_or(0)
        ),
        None => format!(
            "recovered {operation}: stopped {stopped} instances; no committed revision to restart"
        ),
    }
}

/// The next action of offline `inspect` on a damaged journal (22.7).
const DAMAGED: &str = "restore host-journal.jsonl or move the state aside; the host does not start on a damaged journal";

/// Inspect live status, or label a durable snapshot as offline when no host
/// is reachable. Offline status never asserts current workload health; it
/// verifies the journals read only and reports them as `journal` (22.7).
pub fn inspect(state: &Path, operation: Option<&str>) -> Result<Json, Failure> {
    match request(state, "inspect", "", operation.map_or(Json::Null, text)) {
        Ok(value) => Ok(value),
        Err(error) => {
            if connect(state).is_ok() {
                return Err(error);
            }
            let path = state.join("status.json");
            let source = std::fs::read_to_string(&path).map_err(|why| {
                fail(
                    3,
                    format!(
                        "{}; and no durable status at {}: {why}",
                        error.reason,
                        path.display()
                    ),
                )
            })?;
            let snapshot = Json::parse(&source)
                .map_err(|reason| fail(3, format!("{}: {reason}", path.display())))?;
            let mut value = describe(&snapshot, state, None);
            let journal = host::integrity(state)?;
            if let Json::Object(fields) = &mut value {
                if journal.get("intact") == Some(&Json::Bool(false)) {
                    fields.insert("nextAction".into(), text(DAMAGED));
                }
                fields.insert("journal".into(), journal);
            }
            if let (Some(operation), Json::Object(fields)) = (operation, &mut value) {
                let receipt = fields
                    .get("operations")
                    .and_then(|ops| ops.get(operation))
                    .cloned()
                    .ok_or_else(|| fail(2, format!("unknown operation {operation}")))?;
                fields.insert("operation".into(), receipt);
            }
            Ok(value)
        }
    }
}

/// What only a running host can observe: its own process and the
/// readiness of the machines its revision controller drives.
struct Liveness {
    pid: u32,
    start_time: Option<String>,
    expected: Vec<String>,
    ready: Vec<String>,
    /// Whether admission takes a corrective plan for the host's recovery.
    correctable: bool,
}

/// The inspect view of a host snapshot (17.1): the snapshot's identity,
/// revisions, ownership and recovery reason, plus the current operation,
/// the progress of its plan's member actions, readiness and one top-level
/// next action. `live` is `None` for a durable snapshot read while no host
/// answers: the view is then labelled offline, and ownership loses its
/// readiness flags, because a snapshot cannot say what still runs.
fn describe(snapshot: &Json, state: &Path, live: Option<Liveness>) -> Json {
    let Json::Object(mut fields) = snapshot.clone() else {
        return snapshot.clone();
    };
    let get = |key: &str| snapshot.get(key).and_then(Json::as_str);
    let current = get("activeOperation").or_else(|| get("committedPlan"));
    let receipt = current.and_then(|id| snapshot.get("operations").and_then(|ops| ops.get(id)));
    let receipt_text = |key: &str| receipt.and_then(|r| r.get(key)).and_then(Json::as_str);
    let operation_state = receipt_text("state");
    fields.insert(
        "currentOperation".into(),
        match current {
            Some(id) => object([
                ("operationId", text(id)),
                ("state", operation_state.map_or(Json::Null, text)),
                ("reason", receipt_text("reason").map_or(Json::Null, text)),
                (
                    "revision",
                    receipt
                        .and_then(|r| r.get("revision"))
                        .cloned()
                        .unwrap_or(Json::Null),
                ),
            ]),
            None => Json::Null,
        },
    );
    fields.insert(
        "actionProgress".into(),
        current.map_or(Json::Null, |id| {
            action_progress(snapshot, state, id, operation_state)
        }),
    );
    let recovery = get("recoveryReason");
    let next = if live.is_none() {
        format!(
            "start `ignite host --state {}`; this is the last durable status, not live health",
            state.display()
        )
    } else if let Some(next) = snapshot
        .get("recovery")
        .and_then(|recovery| recovery.get("nextAction"))
        .and_then(Json::as_str)
    {
        next.to_string()
    } else if recovery.is_some() {
        let first = receipt_text("nextAction")
            .unwrap_or("inspect retained ownership and the host journal")
            .to_string();
        // A corrective is named only when admission would take one.
        if live.as_ref().is_some_and(|live| live.correctable) {
            first + "; then prepare and apply a corrective plan"
        } else {
            first
        }
    } else if let (Some(id), Some(progress @ ("accepted" | "applying"))) =
        (get("activeOperation"), operation_state)
    {
        format!("wait: operation {id} is {progress}; `ignite apply` of its plan attaches to it")
    } else {
        "none".to_string()
    };
    fields.insert("nextAction".into(), text(next));
    match live {
        Some(live) => {
            // A running host verified its journals at its start.
            fields.insert("journal".into(), object([("intact", Json::Bool(true))]));
            let converged = live.expected.iter().all(|id| live.ready.contains(id))
                && recovery.is_none()
                && get("activeOperation").is_none();
            fields.insert(
                "host".into(),
                object([
                    ("pid", Json::Int(i64::from(live.pid))),
                    ("startTime", live.start_time.map_or(Json::Null, text)),
                ]),
            );
            fields.insert(
                "readiness".into(),
                object([
                    (
                        "expected",
                        Json::Array(live.expected.into_iter().map(text).collect()),
                    ),
                    (
                        "ready",
                        Json::Array(live.ready.into_iter().map(text).collect()),
                    ),
                    ("converged", Json::Bool(converged)),
                ]),
            );
        }
        None => {
            fields.insert("online".into(), Json::Bool(false));
            fields.insert("host".into(), Json::Null);
            fields.insert("health".into(), text("unknown: host offline"));
            fields.insert("readiness".into(), text("unknown: host offline"));
            let mut view = Json::Object(fields);
            rename_ready(&mut view);
            return view;
        }
    }
    Json::Object(fields)
}

/// Rename every `ready` key in a durable snapshot, at any depth (owners,
/// retiring instances, receipts), to `lastRecordedReady`: a snapshot read
/// while no host answers records readiness, it does not observe it.
fn rename_ready(value: &mut Json) {
    match value {
        Json::Object(fields) => {
            if let Some(ready) = fields.remove("ready") {
                fields.insert("lastRecordedReady".into(), ready);
            }
            fields.values_mut().for_each(rename_ready);
        }
        Json::Array(items) => items.iter_mut().for_each(rename_ready),
        _ => {}
    }
}

/// Each member action of operation `id`'s plan with its progress: `done`
/// once the operation completed or the snapshot shows the action's effect
/// (an added or replaced member owned by this operation and ready, a
/// removed member no longer owned), `blocked` while the operation requires
/// recovery, `pending` otherwise.
fn action_progress(snapshot: &Json, state: &Path, id: &str, operation: Option<&str>) -> Json {
    let path = state.join("plans").join(format!("{id}.json"));
    let plan = std::fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|source| Json::parse(&source));
    let members = match &plan {
        Ok(plan) => plan
            .get("actions")
            .and_then(|actions| actions.get("members"))
            .and_then(Json::as_array)
            .unwrap_or(&[]),
        Err(why) => {
            return object([
                ("operationId", text(id)),
                ("error", text(format!("{}: {why}", path.display()))),
            ]);
        }
    };
    let ownership = snapshot.get("ownership");
    let owner = |member: &str| ownership.and_then(|owned| owned.get(member));
    let mut done = 0;
    let rows: Vec<Json> = members
        .iter()
        .map(|action| {
            let member = action.get("id").and_then(Json::as_str).unwrap_or("");
            let kind = action.get("kind").and_then(Json::as_str).unwrap_or("");
            let effect = match kind {
                "unchanged" | "detach" => true,
                "add" | "replace" => owner(member).is_some_and(|owned| {
                    owned.get("operationId").and_then(Json::as_str) == Some(id)
                        && owned.get("ready").and_then(Json::as_bool) == Some(true)
                }),
                "remove" => owner(member).is_none(),
                _ => false,
            };
            let progress = match operation {
                Some("completed") => "done",
                _ if effect => "done",
                Some("accepted" | "applying") => "pending",
                _ => "blocked",
            };
            if progress == "done" {
                done += 1;
            }
            object([
                ("member", text(member)),
                ("action", text(kind)),
                (
                    "reason",
                    action.get("reason").cloned().unwrap_or(Json::Null),
                ),
                ("progress", text(progress)),
            ])
        })
        .collect();
    object([
        ("operationId", text(id)),
        ("state", operation.map_or(Json::Null, text)),
        ("done", Json::Int(done)),
        ("total", Json::Int(rows.len() as i64)),
        ("members", Json::Array(rows)),
    ])
}
