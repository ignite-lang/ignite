//! The substrate through real loopback sockets.

use std::sync::mpsc;
use std::time::Duration;

use ignite_io::{Input, IoError, TcpListener, TcpStream};

const LOCALHOST: &str = "127.0.0.1";
const LIMIT: Duration = Duration::from_secs(5);

fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port();
    let client = TcpStream::connect(LOCALHOST.to_string(), port).unwrap();
    let server = listener.accept().unwrap();
    (client, server)
}

fn read_exactly(stream: &TcpStream, count: usize) -> Vec<u8> {
    let mut received = Vec::new();
    while received.len() < count {
        match stream.read(4096).unwrap() {
            Input::Data(bytes) => received.extend(bytes),
            Input::Eof => panic!("end of file after {} bytes", received.len()),
        }
    }
    received
}

/// `operation` on a new thread; its result, or `None` when it has not
/// finished within `limit`.
fn within<T: Send + 'static>(
    limit: Duration,
    operation: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(operation());
    });
    receiver.recv_timeout(limit).ok()
}

/// A read of `stream` on another thread, and the receiver of its result.
fn pending_read(stream: &TcpStream) -> mpsc::Receiver<Result<Input, IoError>> {
    let reader = stream.clone();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(reader.read(4096));
    });
    receiver
}

#[test]
fn bind_port_zero_picks_a_port() {
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port();
    assert!((1..=65535).contains(&port), "port {port}");
}

#[test]
fn bind_twice_on_one_port_fails() {
    let first = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let error = TcpListener::bind(LOCALHOST.to_string(), first.local_port()).unwrap_err();
    assert!(!error.message.is_empty());
}

#[test]
fn bind_port_out_of_range_fails() {
    let error = TcpListener::bind(LOCALHOST.to_string(), 70000).unwrap_err();
    assert!(error.message.contains("70000"), "{error}");
    let error = TcpStream::connect(LOCALHOST.to_string(), -1).unwrap_err();
    assert!(!error.message.is_empty());
}

#[test]
fn round_trip_and_half_close() {
    let (client, server) = pair();

    client.write_all(b"hello".to_vec()).unwrap();
    assert_eq!(server.read(4096).unwrap(), Input::Data(b"hello".to_vec()));

    client.close().unwrap();
    assert_eq!(server.read(4096).unwrap(), Input::Eof);

    server.write_all(b"bye".to_vec()).unwrap();
    assert_eq!(read_exactly(&client, 3), b"bye");
}

#[test]
fn read_respects_maximum() {
    let (client, server) = pair();
    client.write_all(b"0123456789".to_vec()).unwrap();
    match server.read(4).unwrap() {
        Input::Data(bytes) => assert!(!bytes.is_empty() && bytes.len() <= 4, "{bytes:?}"),
        Input::Eof => panic!("unexpected end of file"),
    }
}

#[test]
fn clone_shares_the_socket() {
    let (client, server) = pair();
    let copy = client.clone();
    copy.write_all(b"ab".to_vec()).unwrap();
    client.write_all(b"cd".to_vec()).unwrap();
    assert_eq!(read_exactly(&server, 4), b"abcd");
}

#[test]
fn write_proceeds_while_a_read_is_pending() {
    let (client, server) = pair();
    let pending = pending_read(&server);
    std::thread::sleep(Duration::from_millis(50));
    assert!(pending.try_recv().is_err(), "read returned without input");

    let writer = server.clone();
    within(LIMIT, move || writer.write_all(b"ping".to_vec()))
        .expect("write blocked behind the pending read")
        .unwrap();
    assert_eq!(read_exactly(&client, 4), b"ping");

    client.write_all(b"pong".to_vec()).unwrap();
    ignite_io::flush();
    let received = pending
        .recv_timeout(LIMIT)
        .expect("pending read did not complete")
        .unwrap();
    assert_eq!(received, Input::Data(b"pong".to_vec()));
}

#[test]
fn close_proceeds_while_a_read_is_pending() {
    let (client, server) = pair();
    let pending = pending_read(&server);
    std::thread::sleep(Duration::from_millis(50));
    let closer = server.clone();
    within(LIMIT, move || closer.close())
        .expect("close blocked behind the pending read")
        .unwrap();
    assert_eq!(client.read(4096).unwrap(), Input::Eof);
    client.close().unwrap();
    assert_eq!(pending.recv_timeout(LIMIT).unwrap().unwrap(), Input::Eof);
}

#[test]
fn run_returns_zero_on_ok() {
    assert_eq!(ignite_io::run(|| Ok(())), 0);
}

#[test]
fn run_returns_one_on_err() {
    assert_eq!(ignite_io::run(|| Err(IoError::new("failed"))), 1);
}

#[test]
fn spawn_inside_run_executes() {
    let (sender, receiver) = mpsc::channel();
    let status = ignite_io::run(move || {
        ignite_io::spawn(move || {
            sender.send(42).unwrap();
        });
        match receiver.recv_timeout(LIMIT) {
            Ok(value) => {
                assert_eq!(value, 42);
                Ok(())
            }
            Err(_) => Err(IoError::new("spawned task did not run")),
        }
    });
    assert_eq!(status, 0);
}

#[test]
fn spawn_runs_tasks_in_parallel() {
    let (client, server) = pair();
    let (sender, receiver) = mpsc::channel();
    ignite_io::spawn(move || {
        let _ = sender.send(server.read(4096));
    });
    std::thread::sleep(Duration::from_millis(50));
    client.write_all(b"go".to_vec()).unwrap();
    ignite_io::flush();
    let received = receiver
        .recv_timeout(LIMIT)
        .expect("the spawned read did not complete")
        .unwrap();
    assert_eq!(received, Input::Data(b"go".to_vec()));
}

#[test]
fn writes_are_sent_together_at_the_next_wait() {
    let (client, server) = pair();
    let task = server.clone();
    let (sender, receiver) = mpsc::channel();
    ignite_io::spawn(move || {
        task.write_all(b"one ".to_vec()).unwrap();
        task.write_from(b"two").unwrap();
        let _ = sender.send(());
        let _ = task.read(1);
    });
    receiver.recv_timeout(LIMIT).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(read_exactly(&client, 7), b"one two");
    client.write_all(b"x".to_vec()).unwrap();
}

#[test]
fn a_thread_sends_what_it_wrote_when_it_ends() {
    let (client, server) = pair();
    std::thread::spawn(move || server.write_all(b"last".to_vec()).unwrap())
        .join()
        .unwrap();
    assert_eq!(read_exactly(&client, 4), b"last");
}

/// A substrate stream and a plain `std::net` peer, so a read on the peer
/// does not flush this thread's pending writes.
fn raw_pair() -> (TcpStream, std::net::TcpStream) {
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = u16::try_from(listener.local_port()).unwrap();
    let peer = std::net::TcpStream::connect((LOCALHOST, port)).unwrap();
    peer.set_read_timeout(Some(LIMIT)).unwrap();
    (listener.accept().unwrap(), peer)
}

fn raw_read(peer: &mut std::net::TcpStream) -> Vec<u8> {
    use std::io::Read;
    let mut buffer = vec![0u8; 64];
    let count = peer.read(&mut buffer).unwrap();
    buffer.truncate(count);
    buffer
}

#[test]
fn dropping_the_last_handle_sends_and_closes_at_once() {
    let (server, mut peer) = raw_pair();
    server.write_all(b"dropped".to_vec()).unwrap();
    drop(server);
    assert_eq!(raw_read(&mut peer), b"dropped");
    assert_eq!(raw_read(&mut peer), b"");
}

#[test]
fn a_write_after_close_is_the_writers_error() {
    let (server, mut peer) = raw_pair();
    let reader = server.clone();
    reader.write_all(b"from-A".to_vec()).unwrap();
    let closer = server.clone();
    within(LIMIT, move || closer.close()).unwrap().unwrap();
    let error = server.write_all(b"after-close".to_vec()).unwrap_err();
    assert_eq!(error.message, "write to a closed stream");
    assert_eq!(server.write_from(b"again").unwrap_err(), error);
    assert_eq!(raw_read(&mut peer), b"from-A");
    assert_eq!(raw_read(&mut peer), b"");
    use std::io::Write;
    peer.write_all(b"x").unwrap();
    assert_eq!(reader.read(16).unwrap(), Input::Data(b"x".to_vec()));
}
