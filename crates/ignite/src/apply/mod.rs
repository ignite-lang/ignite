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

fn write_frame(stream: &mut UnixStream, value: &Json) -> io::Result<()> {
    let bytes = value.compact().into_bytes();
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host frame exceeds 16 MiB",
        ));
    }
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)
}

fn read_frame(stream: &mut UnixStream) -> io::Result<Json> {
    let mut size = [0; 4];
    stream.read_exact(&mut size)?;
    let size = u32::from_be_bytes(size) as usize;
    if size > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host frame exceeds 16 MiB",
        ));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    let source = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
    Json::parse(source).map_err(io::Error::other)
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
    let mut stream = connect(state)?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let value = object([
        ("version", text(PROTOCOL)),
        ("requestId", text(&id)),
        ("stateIdentity", text(identity)),
        ("operation", text(operation)),
        ("payload", payload),
    ]);
    write_frame(&mut stream, &value)?;
    let response = read_frame(&mut stream)?;
    if required(&response, "version")? != PROTOCOL || required(&response, "requestId")? != id {
        return Err(fail(3, "host response version/request id mismatch"));
    }
    if response.get("status").and_then(Json::as_i64) != Some(0) {
        return Err(fail(
            response.get("status").and_then(Json::as_i64).unwrap_or(3) as i32,
            required(&response, "reason")?,
        ));
    }
    Ok(response.get("payload").cloned().unwrap_or(Json::Null))
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
pub fn apply(file: &Path, state: &Path) -> Result<Json, Failure> {
    let value = Json::parse(&std::fs::read_to_string(file)?).map_err(|reason| fail(2, reason))?;
    let plan = plan::Plan::parse(&value).map_err(|reason| fail(2, reason))?;
    request(state, "apply", &plan.state_identity, value)
}

/// Inspect live status, or label a durable snapshot as offline when no host
/// is reachable. Offline status never asserts current workload health.
pub fn inspect(state: &Path, operation: Option<&str>) -> Result<Json, Failure> {
    match request(state, "inspect", "", operation.map_or(Json::Null, text)) {
        Ok(value) => Ok(value),
        Err(error) => {
            if connect(state).is_ok() {
                return Err(error);
            }
            let mut value = Json::parse(&std::fs::read_to_string(state.join("status.json"))?)
                .map_err(|reason| fail(3, reason))?;
            if let Json::Object(fields) = &mut value {
                fields.insert("online".into(), Json::Bool(false));
                fields.insert("health".into(), text("unknown: host offline"));
                if let Some(operation) = operation {
                    let receipt = fields
                        .get("operations")
                        .and_then(|ops| ops.get(operation))
                        .cloned()
                        .ok_or_else(|| fail(2, "unknown operation"))?;
                    fields.insert("operation".into(), receipt);
                }
            }
            Ok(value)
        }
    }
}
