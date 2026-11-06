//! Urgent data on both streams (`docs/slice.md` 18.8, "Urgent data"):
//! `write_urgent` sends at once with its last byte as the urgent byte, the
//! byte stays in the stream at its place, a read stops before it, and
//! `urgent_pending` holds from its arrival until a read has returned it.
//! The same cases run over TCP (the kernel's urgent mode with
//! `SO_OOBINLINE`) and over a pipe (its counterpart).

use std::time::{Duration, Instant};

use ignite_io::{ByteStream, Input, Pipe, TcpListener, TcpStream};

fn data(bytes: &[u8]) -> Input {
    Input::Data(bytes.to_vec())
}

fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
    let client = TcpStream::connect("127.0.0.1".to_string(), listener.local_port()).unwrap();
    let server = listener.accept().unwrap();
    server.set_idle_timeout(5000).unwrap();
    client.set_idle_timeout(5000).unwrap();
    (client, server)
}

/// Waits until `stream` reports urgent data, up to 5 s: TCP delivers the
/// urgent segment asynchronously.
fn await_urgent<S: ByteStream>(stream: &S) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !stream.urgent_pending().unwrap() {
        assert!(Instant::now() < deadline, "no urgent data within 5 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Bytes written before, the urgent pair, bytes after: the reader sees the
/// urgent mark before it reads, its first read stops before the urgent
/// byte, the next starts with it and clears the mark.
fn reads_stop_before_the_urgent_byte<S: ByteStream>(client: S, server: S) {
    assert!(!server.urgent_pending().unwrap());
    client.write_all(b"junk".to_vec()).unwrap();
    client.write_urgent(vec![255, 242]).unwrap();
    await_urgent(&server);
    assert_eq!(
        server.read(4096).unwrap(),
        data(&[b'j', b'u', b'n', b'k', 255])
    );
    assert!(
        server.urgent_pending().unwrap(),
        "the urgent byte is still ahead"
    );
    client.write_all(b"after".to_vec()).unwrap();
    client.close().unwrap();
    let mut rest = Vec::new();
    loop {
        match server.read(4096).unwrap() {
            Input::Eof => break,
            Input::Data(bytes) => {
                if rest.is_empty() {
                    assert_eq!(bytes[0], 242, "the next read starts with the urgent byte");
                }
                rest.extend(bytes);
                assert!(!server.urgent_pending().unwrap(), "read past the mark");
            }
        }
    }
    assert_eq!(rest, [&[242][..], b"after"].concat());
}

#[test]
fn tcp_reads_stop_before_the_urgent_byte() {
    let (client, server) = tcp_pair();
    reads_stop_before_the_urgent_byte(client, server);
}

#[test]
fn pipe_reads_stop_before_the_urgent_byte() {
    let (client, server) = Pipe::pair();
    server.set_idle_timeout(5000).unwrap();
    reads_stop_before_the_urgent_byte(client, server);
}

/// A lone urgent byte is pending before any read and read inline.
fn a_lone_urgent_byte_is_read_inline<S: ByteStream>(client: S, server: S) {
    client.write_urgent(vec![7]).unwrap();
    await_urgent(&server);
    assert_eq!(server.read(4096).unwrap(), data(&[7]));
    assert!(!server.urgent_pending().unwrap());
}

#[test]
fn tcp_a_lone_urgent_byte_is_read_inline() {
    let (client, server) = tcp_pair();
    a_lone_urgent_byte_is_read_inline(client, server);
}

#[test]
fn pipe_a_lone_urgent_byte_is_read_inline() {
    let (client, server) = Pipe::pair();
    a_lone_urgent_byte_is_read_inline(client, server);
}

/// Empty urgent data sends what was written and marks nothing; after
/// `close` the write is refused.
fn empty_urgent_data_marks_nothing<S: ByteStream>(client: S, server: S) {
    client.write_all(b"x".to_vec()).unwrap();
    client.write_urgent(Vec::new()).unwrap();
    assert_eq!(server.read(4096).unwrap(), data(b"x"));
    assert!(!server.urgent_pending().unwrap());
    client.close().unwrap();
    assert_eq!(
        client.write_urgent(vec![1]).unwrap_err().message,
        "write to a closed stream"
    );
}

#[test]
fn tcp_empty_urgent_data_marks_nothing() {
    let (client, server) = tcp_pair();
    empty_urgent_data_marks_nothing(client, server);
}

#[test]
fn pipe_empty_urgent_data_marks_nothing() {
    let (client, server) = Pipe::pair();
    server.set_idle_timeout(5000).unwrap();
    empty_urgent_data_marks_nothing(client, server);
}
