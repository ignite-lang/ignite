//! The exported functions called directly, as generated code calls them:
//! tasks, TCP over loopback and the text helpers.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use ignite_rt::*;

fn text(value: &str) -> IgniteList {
    IgniteList::from_bytes(value.as_bytes())
}

fn read_text(value: &IgniteList) -> String {
    // SAFETY: produced by the runtime in this task's arena.
    String::from_utf8(unsafe { value.as_bytes() }.to_vec()).unwrap()
}

fn empty() -> IgniteList {
    IgniteList {
        ptr: std::ptr::null_mut(),
        len: 0,
        cap: 0,
    }
}

fn no_error() -> IgniteIoError {
    IgniteIoError {
        message: empty(),
        exit_code: -1,
    }
}

fn no_input() -> IgniteInput {
    IgniteInput {
        tag: -1,
        data: empty(),
    }
}

struct Environment {
    value: IgniteList,
    copied: AtomicBool,
    received: AtomicI64,
    task_arena: AtomicI64,
    done: mpsc::Sender<String>,
}

extern "C" fn slow_child(env: *mut c_void) {
    // SAFETY: the parent keeps `Environment` alive until `spawn_ready`.
    let env = unsafe { &*env.cast::<Environment>() };
    std::thread::sleep(Duration::from_millis(100));
    let copy = IgniteList::from_bytes(unsafe { env.value.as_bytes() });
    env.task_arena
        .store(arena_reserved() as i64, Ordering::SeqCst);
    let done = env.done.clone();
    env.copied.store(true, Ordering::SeqCst);
    ignite_rt_spawn_ready();
    std::thread::sleep(Duration::from_millis(50));
    done.send(read_text(&copy)).unwrap();
    ignite_rt_task_end();
}

#[test]
fn spawn_blocks_until_the_child_is_ready() {
    let (done, finished) = mpsc::channel();
    let mut env = Environment {
        value: text("hello task"),
        copied: AtomicBool::new(false),
        received: AtomicI64::new(0),
        task_arena: AtomicI64::new(-1),
        done,
    };
    ignite_rt_spawn(Some(slow_child), (&raw mut env).cast());
    assert!(env.copied.load(Ordering::SeqCst));
    assert!(env.task_arena.load(Ordering::SeqCst) > 0);
    env.value = text("overwritten after spawn_ready");
    let copied = finished.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(copied, "hello task");
    ignite_rt_task_end();
}

extern "C" fn child_without_ready(env: *mut c_void) {
    // SAFETY: the parent waits until this function returns.
    let env = unsafe { &*env.cast::<Environment>() };
    env.received.store(42, Ordering::SeqCst);
}

#[test]
fn spawn_returns_when_the_child_ends_without_ready() {
    let (done, _finished) = mpsc::channel();
    let mut env = Environment {
        value: empty(),
        copied: AtomicBool::new(false),
        received: AtomicI64::new(0),
        task_arena: AtomicI64::new(-1),
        done,
    };
    ignite_rt_spawn(Some(child_without_ready), (&raw mut env).cast());
    assert_eq!(env.received.load(Ordering::SeqCst), 42);
}

fn bind() -> i64 {
    let (mut listener, mut err) = (0i64, no_error());
    assert_eq!(
        ignite_rt_tcp_bind(&text("127.0.0.1"), 0, &mut listener, &mut err),
        0
    );
    listener
}

/// Every IPv4 socket of the host, from `/proc/net/tcp`: local address as
/// the kernel prints it, state, socket inode.
fn tcp_table() -> Vec<(String, String, u64)> {
    let table = std::fs::read_to_string("/proc/net/tcp").expect("/proc/net/tcp");
    table
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            (
                fields[1].to_string(),
                fields[3].to_string(),
                fields[9].parse().expect("socket inode"),
            )
        })
        .collect()
}

/// The socket inode of the listener bound to `127.0.0.1:port`, so that the
/// socket can be checked after its handle is released whoever owns the port
/// by then (state `0A` is LISTEN).
fn listening_socket(port: i64) -> Option<u64> {
    let localhost = u32::from_ne_bytes(std::net::Ipv4Addr::LOCALHOST.octets());
    let local = format!("{localhost:08X}:{port:04X}");
    tcp_table()
        .into_iter()
        .find(|(address, state, _)| *address == local && state == "0A")
        .map(|(_, _, inode)| inode)
}

/// Whether a socket with `inode` is still open anywhere on the host.
fn socket_exists(inode: u64) -> bool {
    tcp_table().iter().any(|(_, _, open)| *open == inode)
}

fn read(stream: i64, maximum: i64) -> IgniteInput {
    let (mut input, mut err) = (no_input(), no_error());
    let status = ignite_rt_tcp_read(stream, maximum, &mut input, &mut err);
    assert_eq!(status, 0, "{}", read_text(&err.message));
    input
}

fn write(stream: i64, data: &[u8]) {
    let mut err = no_error();
    let status = ignite_rt_tcp_write_all(stream, &IgniteList::from_bytes(data), &mut err);
    assert_eq!(status, 0, "{}", read_text(&err.message));
}

#[test]
fn tcp_round_trip_with_concurrent_read_and_write() {
    let listener = bind();
    let port = ignite_rt_tcp_local_port(listener);
    assert!(port > 0);

    let server = std::thread::spawn(move || {
        let (mut stream, mut err) = (0i64, no_error());
        assert_eq!(ignite_rt_tcp_accept(listener, &mut stream, &mut err), 0);
        let mut received = Vec::new();
        while received.len() < 4 {
            let input = read(stream, 2);
            assert_eq!(input.tag, 0);
            assert!(input.data.len <= 2);
            received.extend_from_slice(unsafe { input.data.as_bytes() });
        }
        assert_eq!(received, b"ping");
        write(stream, b"pong");
        let input = read(stream, 64);
        assert_eq!(input.tag, 1, "client closed its write half");
        assert_eq!(ignite_rt_tcp_close(stream, &mut err), 0);
        ignite_rt_task_end();
    });

    let (mut client, mut err) = (0i64, no_error());
    assert_eq!(
        ignite_rt_tcp_connect(&text("127.0.0.1"), port, &mut client, &mut err),
        0
    );

    let (read_done, read_result) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let input = read(client, 64);
        read_done.send(()).unwrap();
        let data = unsafe { input.data.as_bytes() }.to_vec();
        let second = read(client, 64);
        ignite_rt_task_end();
        (input.tag, data, second.tag)
    });
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        read_result.try_recv().is_err(),
        "the read waits for the server"
    );

    let (written, write_result) = mpsc::channel();
    std::thread::spawn(move || {
        write(client, b"ping");
        written.send(()).unwrap();
    });
    write_result
        .recv_timeout(Duration::from_secs(5))
        .expect("a write is not blocked by a pending read on the same stream");

    read_result.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(ignite_rt_tcp_close(client, &mut err), 0);
    let (tag, data, second) = reader.join().unwrap();
    assert_eq!((tag, data.as_slice()), (0, &b"pong"[..]));
    assert_eq!(second, 1, "server closed its write half");
    server.join().unwrap();
    ignite_rt_task_end();
}

#[test]
fn release_closes_the_socket_and_invalidates_the_handle() {
    let listener = bind();
    let port = ignite_rt_tcp_local_port(listener);
    let (mut client, mut server, mut err) = (0i64, 0i64, no_error());
    assert_eq!(
        ignite_rt_tcp_connect(&text("127.0.0.1"), port, &mut client, &mut err),
        0
    );
    assert_eq!(ignite_rt_tcp_accept(listener, &mut server, &mut err), 0);

    ignite_rt_tcp_release(server);
    assert_eq!(read(client, 16).tag, 1, "the peer sees the socket closed");

    let status = ignite_rt_tcp_write_all(server, &text("late"), &mut err);
    assert_eq!((status, err.exit_code), (1, 1));
    assert_eq!(
        read_text(&err.message),
        format!("handle {server} was released")
    );
    let mut input = no_input();
    assert_eq!(ignite_rt_tcp_read(server, 1, &mut input, &mut err), 1);
    assert_eq!(ignite_rt_tcp_close(server, &mut err), 1);
    ignite_rt_tcp_release(server);
    ignite_rt_tcp_release(0);
    ignite_rt_tcp_release(-5);
    ignite_rt_tcp_release(i64::MAX);

    let inode =
        listening_socket(port).expect("the listener is a listening socket in /proc/net/tcp");
    ignite_rt_tcp_release(listener);
    assert_eq!(ignite_rt_tcp_local_port(listener), 0);
    assert!(
        !socket_exists(inode),
        "the released listener's socket (inode {inode}) is closed"
    );
    let mut accepted = 0i64;
    assert_eq!(ignite_rt_tcp_accept(listener, &mut accepted, &mut err), 1);
    assert_eq!(err.exit_code, 1);
    let again = bind();
    assert_ne!(again, listener, "released handles are not reused");
    ignite_rt_tcp_release(again);
    ignite_rt_tcp_release(client);
    ignite_rt_task_end();
}

#[test]
fn os_errors_have_exit_code_one_and_the_os_message() {
    let listener = bind();
    let port = ignite_rt_tcp_local_port(listener);
    let (mut other, mut err) = (0i64, no_error());
    assert_eq!(
        ignite_rt_tcp_bind(&text("127.0.0.1"), port, &mut other, &mut err),
        1
    );
    assert_eq!(err.exit_code, 1);
    let message = read_text(&err.message);
    assert!(
        message.starts_with(&format!("bind 127.0.0.1:{port}: ")),
        "{message}"
    );
    assert!(message.contains("os error"), "{message}");

    assert_eq!(
        ignite_rt_tcp_bind(&text("127.0.0.1"), 70000, &mut other, &mut err),
        1
    );
    assert!(read_text(&err.message).contains("outside 0..=65535"));

    assert_eq!(
        ignite_rt_tcp_connect(&text("127.0.0.1"), 1, &mut other, &mut err),
        1
    );
    assert_eq!(err.exit_code, 1);
    assert!(read_text(&err.message).starts_with("connect 127.0.0.1:1: "));

    let mut input = no_input();
    assert_eq!(ignite_rt_tcp_read(listener, 1, &mut input, &mut err), 1);
    assert!(read_text(&err.message).contains("not a TCP stream"));
    assert_eq!(ignite_rt_tcp_local_port(9_999_999), 0);
    ignite_rt_task_end();
}

#[test]
fn text_helpers() {
    let mut out = empty();

    ignite_rt_text_from_bytes_lossy(&mut out, &IgniteList::from_bytes(b"ok \xff\xfe end"));
    assert_eq!(read_text(&out), "ok \u{fffd}\u{fffd} end");
    ignite_rt_text_from_bytes_lossy(&mut out, &empty());
    assert_eq!(read_text(&out), "");

    ignite_rt_text_trim(&mut out, &text("\u{3000}\t hello world \r\n\u{2003}"));
    assert_eq!(read_text(&out), "hello world");
    ignite_rt_text_trim(&mut out, &text(" \t\n"));
    assert_eq!(read_text(&out), "");
    assert_eq!(out.len, 0);

    ignite_rt_text_concat(&mut out, &text("ign"), &text("ite"));
    assert_eq!(read_text(&out), "ignite");
    assert_eq!(out.cap, 6);

    assert_eq!(ignite_rt_text_eq(&text("a"), &text("a")), 1);
    assert_eq!(ignite_rt_text_eq(&text("a"), &text("b")), 0);
    assert_eq!(ignite_rt_text_eq(&text(""), &empty()), 1);
    assert_eq!(
        ignite_rt_text_starts_with(&text("help me"), &text("help")),
        1
    );
    assert_eq!(ignite_rt_text_starts_with(&text("he"), &text("help")), 0);
    assert_eq!(ignite_rt_text_starts_with(&text("x"), &text("")), 1);

    let parts = [text("a"), text("bc"), text("")];
    let buffer = ignite_rt_alloc(
        (parts.len() * size_of::<IgniteList>()) as i64,
        align_of::<IgniteList>() as i64,
    );
    unsafe {
        std::ptr::copy_nonoverlapping(parts.as_ptr(), buffer.cast(), parts.len());
    }
    let list = IgniteList {
        ptr: buffer,
        len: 3,
        cap: 3,
    };
    ignite_rt_text_join(&mut out, &list, &text(", "));
    assert_eq!(read_text(&out), "a, bc, ");
    ignite_rt_text_join(&mut out, &empty(), &text(", "));
    assert_eq!(read_text(&out), "");

    for value in [0, 7, -42, i64::MAX, i64::MIN] {
        ignite_rt_int_to_text(&mut out, value);
        assert_eq!(read_text(&out), value.to_string());
    }
    ignite_rt_task_end();
}

#[test]
fn layouts_match_section_three() {
    assert_eq!((size_of::<IgniteList>(), align_of::<IgniteList>()), (24, 8));
    assert_eq!(
        (size_of::<IgniteIoError>(), align_of::<IgniteIoError>()),
        (32, 8)
    );
    assert_eq!(std::mem::offset_of!(IgniteIoError, exit_code), 24);
    assert_eq!(std::mem::offset_of!(IgniteInput, data), 8);
    assert_eq!(size_of::<IgniteInput>(), 32);
}
