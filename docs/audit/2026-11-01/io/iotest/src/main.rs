use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use telos_io::{Input, TcpListener, TcpStream, flush, spawn};

fn pair() -> (TcpStream, std::net::TcpStream) {
    let listener = TcpListener::bind("127.0.0.1".into(), 0).unwrap();
    let raw = std::net::TcpStream::connect(("127.0.0.1", listener.local_port() as u16)).unwrap();
    let server = listener.accept().unwrap();
    (server, raw)
}

fn read_with_timeout(raw: &mut std::net::TcpStream, ms: u64) -> std::io::Result<Vec<u8>> {
    raw.set_read_timeout(Some(Duration::from_millis(ms))).unwrap();
    let mut buf = vec![0u8; 1024];
    match raw.read(&mut buf) {
        Ok(n) => Ok(buf[..n].to_vec()),
        Err(e) => Err(e),
    }
}

/// (f) A task that panics after writing: are its pending bytes sent?
fn panic_flushes() {
    let (server, mut raw) = pair();
    let (tx, rx) = mpsc::channel();
    spawn(move || {
        server.write_all(b"before-panic".to_vec()).unwrap();
        tx.send(()).unwrap();
        panic!("task panics with pending writes");
    });
    rx.recv().unwrap();
    let got = read_with_timeout(&mut raw, 2000);
    println!("panic_flushes: peer read = {:?}", got.map(|b| String::from_utf8_lossy(&b).into_owned()));
}

/// (a) A dropped last user handle with unsent bytes: when does the peer see the data and EOF?
fn drop_without_flush() {
    let (server, mut raw) = pair();
    server.write_all(b"dropped".to_vec()).unwrap();
    drop(server);
    let started = Instant::now();
    let first = read_with_timeout(&mut raw, 500);
    println!(
        "drop_without_flush: after drop, before flush: {:?} ({:?})",
        first.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).map_err(|e| e.kind()),
        started.elapsed()
    );
    flush();
    let second = read_with_timeout(&mut raw, 500);
    println!(
        "drop_without_flush: after flush: {:?}",
        second.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).map_err(|e| e.kind())
    );
    let eof = read_with_timeout(&mut raw, 500);
    println!("drop_without_flush: then: {:?}", eof.as_ref().map(|b| b.len()).map_err(|e| e.kind()));
}

/// (b)/(d) Task A buffers a write and never waits; task B closes; then B writes after
/// close (EPIPE kept); A's next read reports it although A did not fail.
fn cross_task_failure() {
    let (server, mut raw) = pair();
    let a = server.clone();
    let b = server.clone();
    let (tx, rx) = mpsc::channel::<String>();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let tx_a = tx.clone();
    spawn(move || {
        a.write_all(b"from-A".to_vec()).unwrap();
        tx_a.send("A wrote (buffered)".into()).unwrap();
        go_rx.recv().unwrap();
        let r = a.read(16);
        tx_a.send(format!("A read -> {r:?}")).unwrap();
    });
    println!("cross_task_failure: {}", rx.recv().unwrap());
    let tx_b = tx.clone();
    spawn(move || {
        let c = b.close();
        tx_b.send(format!("B close -> {c:?}")).unwrap();
        let w = b.write_all(b"after-close".to_vec());
        tx_b.send(format!("B write after close -> {w:?}")).unwrap();
    });
    println!("cross_task_failure: {}", rx.recv().unwrap());
    println!("cross_task_failure: {}", rx.recv().unwrap());
    let got = read_with_timeout(&mut raw, 1000);
    println!("cross_task_failure: peer read -> {:?}", got.map(|b| String::from_utf8_lossy(&b).into_owned()));
    std::thread::sleep(Duration::from_millis(1300));
    let eof = read_with_timeout(&mut raw, 500);
    println!("cross_task_failure: peer read -> {:?}", eof.map(|b| b.len()));
    go_tx.send(()).unwrap();
    println!("cross_task_failure: {}", rx.recv_timeout(Duration::from_secs(3)).unwrap_or("A read timed out".into()));
}

/// (h) accept on a listener whose fd is shut down returns the OS error (non-transient).
fn accept_error_kind() {
    let listener = TcpListener::bind("127.0.0.1".into(), 0).unwrap();
    let l2 = listener.clone();
    let (tx, rx) = mpsc::channel();
    spawn(move || {
        let r = l2.accept();
        tx.send(format!("{:?}", r.map(|_| ())));
    });
    std::thread::sleep(Duration::from_millis(100));
    let port = listener.local_port();
    let _ = std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
    println!("accept_error_kind: {}", rx.recv_timeout(Duration::from_secs(2)).unwrap_or("accept blocked".into()));
}

/// Thread reuse: a second task after the first ended runs on the same thread and has an
/// empty pending set (its first flush sends nothing of its own).
fn reuse_has_clean_pending() {
    let (server, mut raw) = pair();
    let s1 = server.clone();
    let (tx, rx) = mpsc::channel();
    let t1 = tx.clone();
    spawn(move || {
        s1.write_all(b"task1".to_vec()).unwrap();
        t1.send(std::thread::current().id()).unwrap();
    });
    let id1 = rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let got = read_with_timeout(&mut raw, 1000).map(|b| String::from_utf8_lossy(&b).into_owned());
    let t2 = tx.clone();
    spawn(move || {
        let _ = telos_io::Stdin::is_terminal();
        t2.send(std::thread::current().id()).unwrap();
    });
    let id2 = rx.recv().unwrap();
    println!("reuse: task1 bytes at task end -> {got:?}; same thread = {}", id1 == id2);
    let _ = server.read(1);
    let r = read_with_timeout(&mut raw, 300).map(|b| b.len());
    println!("reuse: extra bytes after reuse -> {r:?}");
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    match which.as_str() {
        "panic" => panic_flushes(),
        "drop" => drop_without_flush(),
        "cross" => cross_task_failure(),
        "accept" => accept_error_kind(),
        "reuse" => reuse_has_clean_pending(),
        _ => {
            panic_flushes();
            drop_without_flush();
            cross_task_failure();
            reuse_has_clean_pending();
            accept_error_kind();
        }
    }
    let _ = Input::Eof;
}
