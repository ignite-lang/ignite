//! The todo service of `examples/todo/todo.ignite` in plain Rust: hyper 1 (HTTP/1.1,
//! keep-alive) on tokio's multi-threaded runtime, a reference program for
//! `docs/quality.md` row 3.3.
//!
//! Same routes, status codes, bodies and `Content-Type`s as the Ignite program, and the same
//! durability: every change writes `<shard>/<id>/<version>.json` under `todo-data` in the
//! working directory (a temporary file renamed over it, no `fsync`), then removes the older
//! versions of that todo, before it answers; a deletion is a tombstone. Differences: the
//! data is loaded once at start instead of on the first request (a failed load exits instead
//! of answering 503), the todos are a `BTreeMap` behind one `Mutex` instead of a `Store`, and
//! headers are hyper's (lower-case names, a `date` header, no `Connection: keep-alive`).
//! `--port P` (default 8080) on 127.0.0.1; `listening on 127.0.0.1:P` on stderr once bound.

mod json;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{ALLOW, CONTENT_TYPE, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;

use json::Value;

const SHARD_SIZE: i64 = 65536;
const BODY_LIMIT: usize = 1 << 20;

#[derive(Clone, Debug)]
struct Todo {
    id: i64,
    title: String,
    done: bool,
    version: i64,
    deleted: bool,
}

struct Todos {
    todos: BTreeMap<i64, Todo>,
    next_id: i64,
}

struct State {
    todos: Mutex<Todos>,
    data: PathBuf,
    writes: AtomicU64,
}

fn to_json(todo: &Todo) -> String {
    let mut out = format!("{{\"id\":{},\"title\":", todo.id);
    json::quote(&todo.title, &mut out);
    out.push_str(&format!(",\"done\":{}}}", todo.done));
    out
}

fn to_record(todo: &Todo) -> String {
    let mut out = format!("{{\"id\":{},\"title\":", todo.id);
    json::quote(&todo.title, &mut out);
    out.push_str(&format!(
        ",\"done\":{},\"version\":{},\"deleted\":{}}}",
        todo.done, todo.version, todo.deleted
    ));
    out
}

fn from_record(value: &Value) -> Option<Todo> {
    Some(Todo {
        id: value.int("id")?,
        title: value.text("title")?.to_string(),
        done: value.bool("done")?,
        version: value.int("version")?,
        deleted: value.bool("deleted")?,
    })
}

fn shard_of(id: i64) -> String {
    if id <= SHARD_SIZE {
        "todos".to_string()
    } else {
        format!("todos-{}", (id - 1) / SHARD_SIZE)
    }
}

fn is_shard(name: &str) -> bool {
    name == "todos" || name.strip_prefix("todos-").and_then(parse_int).is_some()
}

/// `Int.parse` of Ignite: decimal digits with an optional leading `-`.
fn parse_int(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn version_of(name: &str) -> Option<i64> {
    name.strip_suffix(".json").and_then(parse_int)
}

impl State {
    fn save(&self, todo: &Todo) -> std::io::Result<()> {
        let directory = self.data.join(shard_of(todo.id)).join(todo.id.to_string());
        std::fs::create_dir_all(&directory)?;
        let n = self.writes.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(".tmp-{}-{n}", std::process::id()));
        std::fs::write(&temporary, to_record(todo))?;
        if let Err(error) =
            std::fs::rename(&temporary, directory.join(format!("{}.json", todo.version)))
        {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        for entry in std::fs::read_dir(&directory)?.flatten() {
            if let Some(version) = entry.file_name().to_str().and_then(version_of)
                && version < todo.version
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}

fn newest(directory: &Path) -> std::io::Result<Option<Todo>> {
    let mut found: Option<Todo> = None;
    for entry in std::fs::read_dir(directory)?.flatten() {
        if entry.file_name().to_str().and_then(version_of).is_none() {
            continue;
        }
        let Some(todo) = std::fs::read(entry.path())
            .ok()
            .and_then(|bytes| json::parse(&bytes))
            .and_then(|value| from_record(&value))
        else {
            continue;
        };
        if found.as_ref().is_none_or(|old| old.version < todo.version) {
            found = Some(todo);
        }
    }
    Ok(found)
}

fn load(data: &Path) -> std::io::Result<Todos> {
    std::fs::create_dir_all(data.join("todos"))?;
    let mut todos = BTreeMap::new();
    for shard in std::fs::read_dir(data)?.flatten() {
        let name = shard.file_name();
        if !shard.file_type()?.is_dir() || !name.to_str().is_some_and(is_shard) {
            continue;
        }
        for entry in std::fs::read_dir(shard.path())?.flatten() {
            if entry.file_type()?.is_dir()
                && let Some(todo) = newest(&entry.path())?
            {
                todos.insert(todo.id, todo);
            }
        }
    }
    Ok(Todos { todos, next_id: 0 })
}

type Answer = Response<Full<Bytes>>;

fn respond(status: u16, content_type: Option<&'static str>, body: String) -> Answer {
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = StatusCode::from_u16(status).expect("status");
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    }
    response
}

fn json_response(status: u16, body: String) -> Answer {
    respond(status, Some("application/json"), body)
}

fn plain(status: u16, body: &str) -> Answer {
    respond(status, Some("text/plain; charset=utf-8"), body.to_string())
}

fn status_response(status: u16) -> Answer {
    match status {
        204 => respond(204, None, String::new()),
        400 => plain(400, "Bad Request\n"),
        404 => plain(404, "Not Found\n"),
        405 => plain(405, "Method Not Allowed\n"),
        _ => plain(status, "Unknown\n"),
    }
}

fn method_not_allowed(allow: &'static str) -> Answer {
    let mut response = status_response(405);
    response
        .headers_mut()
        .insert(ALLOW, HeaderValue::from_static(allow));
    response
}

fn stored(state: &State, todo: &Todo, ok: Answer) -> Answer {
    match state.save(todo) {
        Ok(()) => ok,
        Err(_) => plain(500, "not stored\n"),
    }
}

fn list(state: &State) -> Answer {
    let todos = state.todos.lock().unwrap();
    let shown: Vec<String> = todos
        .todos
        .values()
        .filter(|t| !t.deleted)
        .map(to_json)
        .collect();
    json_response(200, format!("[{}]", shown.join(",")))
}

fn create(state: &State, body: &[u8]) -> Answer {
    let Some(value) = json::parse(body) else {
        return status_response(400);
    };
    let Some(title) = value.text("title") else {
        return status_response(400);
    };
    let todo = {
        let mut todos = state.todos.lock().unwrap();
        let mut id = todos.next_id + 1;
        while todos.todos.contains_key(&id) {
            id += 1;
        }
        todos.next_id = id;
        let todo = Todo {
            id,
            title: title.to_string(),
            done: value.bool("done").unwrap_or(false),
            version: 1,
            deleted: false,
        };
        todos.todos.insert(id, todo.clone());
        todo
    };
    stored(state, &todo, json_response(201, to_json(&todo)))
}

fn live(state: &State, id: i64) -> Option<Todo> {
    let todos = state.todos.lock().unwrap();
    todos.todos.get(&id).filter(|t| !t.deleted).cloned()
}

fn show(state: &State, id: i64) -> Answer {
    match live(state, id) {
        Some(todo) => json_response(200, to_json(&todo)),
        None => status_response(404),
    }
}

fn patch(state: &State, id: i64, body: &[u8]) -> Answer {
    let Some(value) = json::parse(body) else {
        return status_response(if live(state, id).is_none() { 404 } else { 400 });
    };
    let changed = {
        let mut todos = state.todos.lock().unwrap();
        match todos.todos.get_mut(&id) {
            Some(todo) if !todo.deleted => {
                if let Some(title) = value.text("title") {
                    todo.title = title.to_string();
                }
                if let Some(done) = value.bool("done") {
                    todo.done = done;
                }
                todo.version += 1;
                Some(todo.clone())
            }
            _ => None,
        }
    };
    match changed {
        Some(todo) => stored(state, &todo, json_response(200, to_json(&todo))),
        None => status_response(404),
    }
}

fn delete(state: &State, id: i64) -> Answer {
    let deleted = {
        let mut todos = state.todos.lock().unwrap();
        match todos.todos.get_mut(&id) {
            Some(todo) if !todo.deleted => {
                todo.deleted = true;
                todo.version += 1;
                Some(todo.clone())
            }
            _ => None,
        }
    };
    match deleted {
        Some(todo) => stored(state, &todo, status_response(204)),
        None => status_response(404),
    }
}

fn route(state: &State, method: &Method, path: &str, body: &[u8]) -> Answer {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match (method.as_str(), segments.as_slice()) {
        ("GET", ["health"]) => plain(200, "ok\n"),
        ("GET", ["todos"]) => list(state),
        ("POST", ["todos"]) => create(state, body),
        (_, ["todos"]) => method_not_allowed("GET, POST"),
        (method, ["todos", id]) => match parse_int(id) {
            None => status_response(404),
            Some(id) => match method {
                "GET" => show(state, id),
                "PATCH" => patch(state, id, body),
                "DELETE" => delete(state, id),
                _ => method_not_allowed("GET, PATCH, DELETE"),
            },
        },
        _ => status_response(404),
    }
}

async fn handle(state: Arc<State>, request: Request<Incoming>) -> Result<Answer, Infallible> {
    let (parts, body) = request.into_parts();
    let body = match http_body_util::Limited::new(body, BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return Ok(plain(413, "Content Too Large\n")),
    };
    Ok(route(&state, &parts.method, parts.uri.path(), &body))
}

fn port() -> u16 {
    let mut args = std::env::args().skip(1);
    let mut port = 8080;
    while let Some(arg) = args.next() {
        if arg == "--port" {
            port = args
                .next()
                .and_then(|p| p.parse().ok())
                .unwrap_or_else(|| exit("--port needs a port number"));
        } else {
            exit(&format!("unknown argument {arg}"));
        }
    }
    port
}

fn exit(message: &str) -> ! {
    eprintln!("todo: {message}");
    std::process::exit(2)
}

#[tokio::main]
async fn main() {
    let port = port();
    let data = PathBuf::from("todo-data");
    let todos = load(&data).unwrap_or_else(|e| exit(&format!("loading {}: {e}", data.display())));
    let state = Arc::new(State {
        todos: Mutex::new(todos),
        data,
        writes: AtomicU64::new(0),
    });
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap_or_else(|e| exit(&format!("bind 127.0.0.1:{port}: {e}")));
    let bound = listener.local_addr().map_or(port, |a| a.port());
    eprintln!("listening on 127.0.0.1:{bound}");
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let service = service_fn(move |request| handle(Arc::clone(&state), request));
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}
