//! The in-memory stream pair (`docs/slice.md` 18.4, 18.8): coalesced
//! writes, deliveries counted per batch, close, drop, a vanished peer, the
//! bounded queue and the idle timeout.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::{ByteStream, Input, PIPE_CAPACITY, Pipe, PipeStream, SEND_BUFFER};

const LIMIT: Duration = Duration::from_secs(5);

fn data(bytes: &[u8]) -> Input {
    Input::Data(bytes.to_vec())
}

fn error(result: Result<impl std::fmt::Debug, ignite_io::IoError>) -> String {
    result.expect_err("an error").message
}

#[test]
fn three_writes_before_a_read_arrive_as_one_batch() {
    bounded(
        "three_writes_before_a_read_arrive_as_one_batch",
        three_writes_before_a_read_arrive_as_one_batch_body,
    );
}

fn three_writes_before_a_read_arrive_as_one_batch_body() {
    let (client, server) = Pipe::pair();
    client.write_all(b"one ".to_vec()).unwrap();
    client.write_all(b"two ".to_vec()).unwrap();
    client.write_all(b"three".to_vec()).unwrap();
    assert_eq!(
        server.received_batches(),
        0,
        "nothing delivered before a flush"
    );
    assert_eq!(server.read(4096).unwrap(), data(b"one two three"));
    assert_eq!(server.received_batches(), 1);
}

/// The writer's own read, on its own end, flushes what it wrote to the
/// peer before it waits; the peer answers, which ends that read.
#[test]
fn a_read_flushes_the_reading_task_first() {
    bounded(
        "a_read_flushes_the_reading_task_first",
        a_read_flushes_the_reading_task_first_body,
    );
}

fn a_read_flushes_the_reading_task_first_body() {
    let (client, server) = Pipe::pair();
    client.write_all(b"ping".to_vec()).unwrap();
    assert_eq!(server.received_batches(), 0);
    let (answered, answer) = mpsc::channel();
    std::thread::spawn(move || {
        server.set_idle_timeout(5000).unwrap();
        let got = server.read(64).unwrap();
        server.write_all(b"pong".to_vec()).unwrap();
        ignite_io::flush();
        let _ = answered.send(got);
        server
    });
    client.set_idle_timeout(5000).unwrap();
    assert_eq!(client.read(64).unwrap(), data(b"pong"));
    assert_eq!(answer.recv_timeout(LIMIT).unwrap(), data(b"ping"));
    assert_eq!(client.received_batches(), 1);
}

#[test]
fn send_buffer_threshold_delivers_at_once() {
    bounded(
        "send_buffer_threshold_delivers_at_once",
        send_buffer_threshold_delivers_at_once_body,
    );
}

fn send_buffer_threshold_delivers_at_once_body() {
    let (client, server) = Pipe::pair();
    client.write_all(vec![7; SEND_BUFFER - 1]).unwrap();
    assert_eq!(server.received_batches(), 0);
    client.write_from(&[7]).unwrap();
    assert_eq!(server.received_batches(), 1, "delivered without a flush");
    client.write_all(vec![8; 3 * SEND_BUFFER]).unwrap();
    assert_eq!(
        server.received_batches(),
        2,
        "one delivery per threshold send"
    );
    let mut total = 0;
    while total < 4 * SEND_BUFFER {
        match server.read(1 << 20).unwrap() {
            Input::Data(bytes) => total += bytes.len(),
            Input::Eof => panic!("end of stream after {total} bytes"),
        }
    }
    assert_eq!(total, 4 * SEND_BUFFER);
    assert_eq!(server.received_batches(), 2, "reads count nothing");
}

#[test]
fn close_delivers_then_eof() {
    bounded("close_delivers_then_eof", close_delivers_then_eof_body);
}

fn close_delivers_then_eof_body() {
    let (client, server) = Pipe::pair();
    client.write_all(b"bye".to_vec()).unwrap();
    client.close().unwrap();
    assert_eq!(server.received_batches(), 1, "close delivered at once");
    assert_eq!(server.read(64).unwrap(), data(b"bye"));
    assert_eq!(server.read(64).unwrap(), Input::Eof);
    assert_eq!(server.read(64).unwrap(), Input::Eof, "end of stream stays");
    client.close().expect("a second close is not an error");
    server.write_all(b"late".to_vec()).unwrap();
    server.close().unwrap();
    assert_eq!(
        client.read(64).unwrap(),
        data(b"late"),
        "the closed end reads on"
    );
    assert_eq!(client.read(64).unwrap(), Input::Eof);
    assert_eq!(
        server.received_batches(),
        1,
        "a second close delivers nothing"
    );
}

#[test]
fn write_after_close_is_the_writers_error() {
    bounded(
        "write_after_close_is_the_writers_error",
        write_after_close_is_the_writers_error_body,
    );
}

fn write_after_close_is_the_writers_error_body() {
    let (client, server) = Pipe::pair();
    let other = client.clone();
    client.close().unwrap();
    assert_eq!(
        error(other.write_all(b"x".to_vec())),
        "write to a closed stream"
    );
    assert_eq!(error(client.write_from(b"y")), "write to a closed stream");
    server.close().unwrap();
    assert_eq!(
        client.read(64).unwrap(),
        Input::Eof,
        "the writer's error is not kept for a read"
    );
    client.close().expect("nor for a close");
    assert_eq!(server.read(64).unwrap(), Input::Eof);
    assert_eq!(server.received_batches(), 0);
}

#[test]
fn dropping_the_last_handle_delivers_and_closes() {
    bounded(
        "dropping_the_last_handle_delivers_and_closes",
        dropping_the_last_handle_delivers_and_closes_body,
    );
}

fn dropping_the_last_handle_delivers_and_closes_body() {
    let (client, server) = Pipe::pair();
    client.write_all(b"last".to_vec()).unwrap();
    let other = client.clone();
    drop(client);
    assert_eq!(server.received_batches(), 0, "a clone still holds the end");
    drop(other);
    assert_eq!(server.received_batches(), 1, "the last drop delivered");
    assert_eq!(server.read(64).unwrap(), data(b"last"));
    assert_eq!(server.read(64).unwrap(), Input::Eof);
}

/// Every handle of one end gone: a delivery by a flush fails, and the
/// writing end's next `write_all`, `read` or `close` returns the kept
/// error, once.
#[test]
fn a_vanished_peer_fails_the_next_operation() {
    bounded(
        "a_vanished_peer_fails_the_next_operation",
        a_vanished_peer_fails_the_next_operation_body,
    );
}

fn a_vanished_peer_fails_the_next_operation_body() {
    let gone = "pipe peer is gone";
    let (client, server) = Pipe::pair();
    drop(server);
    client.write_all(b"x".to_vec()).unwrap();
    ignite_io::flush();
    assert_eq!(error(client.write_all(b"y".to_vec())), gone);
    client
        .write_all(b"z".to_vec())
        .expect("the error was reported once");

    let (client, server) = Pipe::pair();
    drop(server);
    client.write_all(b"x".to_vec()).unwrap();
    ignite_io::flush();
    assert_eq!(error(client.read(64)), gone);
    assert_eq!(
        client.read(64).unwrap(),
        Input::Eof,
        "the peer's half is closed"
    );

    let (client, server) = Pipe::pair();
    drop(server);
    client.write_all(b"x".to_vec()).unwrap();
    ignite_io::flush();
    assert_eq!(error(client.close()), gone);

    let (client, server) = Pipe::pair();
    drop(server);
    assert_eq!(
        error(client.write_all(vec![0; SEND_BUFFER])),
        gone,
        "a threshold delivery is the writer's error"
    );
}

#[test]
fn a_full_queue_fails_the_writer_and_queues_nothing() {
    bounded(
        "a_full_queue_fails_the_writer_and_queues_nothing",
        a_full_queue_fails_the_writer_and_queues_nothing_body,
    );
}

fn a_full_queue_fails_the_writer_and_queues_nothing_body() {
    let full = "pipe queue full";
    let (client, server) = Pipe::pair();
    let deliveries = PIPE_CAPACITY / SEND_BUFFER;
    for _ in 0..deliveries {
        client.write_all(vec![1; SEND_BUFFER]).unwrap();
    }
    assert_eq!(server.received_batches(), deliveries as i64);
    client.write_all(vec![2]).unwrap();
    ignite_io::flush();
    assert_eq!(
        server.received_batches(),
        deliveries as i64,
        "the failed delivery is not counted"
    );
    assert_eq!(
        error(client.write_all(vec![3])),
        full,
        "kept for the writer"
    );
    assert_eq!(
        error(client.write_all(vec![4; SEND_BUFFER])),
        full,
        "a threshold delivery fails at once"
    );
    client.close().unwrap();
    let mut received = 0;
    while let Input::Data(bytes) = server.read(1 << 20).unwrap() {
        assert!(
            bytes.iter().all(|byte| *byte == 1),
            "only the first batches"
        );
        received += bytes.len();
    }
    assert_eq!(
        received, PIPE_CAPACITY,
        "none of the failed bytes were queued"
    );
    assert_eq!(server.received_batches(), deliveries as i64);
}

#[test]
fn idle_timeout_reads_eof() {
    bounded("idle_timeout_reads_eof", idle_timeout_reads_eof_body);
}

fn idle_timeout_reads_eof_body() {
    let (client, server) = Pipe::pair();
    server.set_idle_timeout(100).unwrap();
    let started = Instant::now();
    assert_eq!(server.read(16).unwrap(), Input::Eof);
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(90), "{waited:?}");
    assert!(waited < LIMIT, "{waited:?}");
    client.write_all(b"hi".to_vec()).unwrap();
    ignite_io::flush();
    assert_eq!(
        server.read(16).unwrap(),
        data(b"hi"),
        "data before the timeout"
    );
    server.set_idle_timeout(0).unwrap();
    let reader = std::thread::spawn(move || server.read(16));
    std::thread::sleep(Duration::from_millis(200));
    assert!(!reader.is_finished(), "0 removed the timeout");
    client.close().unwrap();
    assert_eq!(reader.join().unwrap().unwrap(), Input::Eof);
}

/// A task's end flushes it: the task writes and ends without a read or a
/// flush, and a clone kept outside keeps the end alive, so only the
/// task-end flush can deliver.
#[test]
fn task_end_delivers() {
    bounded("task_end_delivers", task_end_delivers_body);
}

fn task_end_delivers_body() {
    let (client, server) = Pipe::pair();
    let writer = client.clone();
    let (ended, end) = mpsc::channel();
    ignite_io::spawn(move || {
        writer.write_all(b"from a task".to_vec()).unwrap();
        let _ = ended.send(());
    });
    end.recv_timeout(LIMIT).unwrap();
    server.set_idle_timeout(5000).unwrap();
    assert_eq!(server.read(64).unwrap(), data(b"from a task"));
    assert_eq!(server.received_batches(), 1);
    drop(client);
}

#[test]
fn clones_share_the_end() {
    bounded("clones_share_the_end", clones_share_the_end_body);
}

fn clones_share_the_end_body() {
    let (client, server) = Pipe::pair();
    let other = client.clone();
    client.write_all(b"x".to_vec()).unwrap();
    other.write_all(b"y".to_vec()).unwrap();
    let reader = server.clone();
    assert_eq!(reader.read(64).unwrap(), data(b"xy"));
    assert_eq!(server.received_batches(), 1, "one buffer for both clones");
    assert_eq!(reader.received_batches(), 1);
    reader.set_idle_timeout(100).unwrap();
    let started = Instant::now();
    assert_eq!(
        server.read(16).unwrap(),
        Input::Eof,
        "the timeout is shared"
    );
    assert!(started.elapsed() < LIMIT);
    other.close().unwrap();
    assert_eq!(
        error(client.write_all(b"z".to_vec())),
        "write to a closed stream",
        "the closed flag is shared"
    );
    assert_eq!(server.read(16).unwrap(), Input::Eof);
}

/// The interface the Rust target calls on a bounded parameter dispatches
/// to the inherent methods of each stream type.
#[test]
fn byte_stream_dispatches_to_the_pipe() {
    bounded(
        "byte_stream_dispatches_to_the_pipe",
        byte_stream_dispatches_to_the_pipe_body,
    );
}

fn byte_stream_dispatches_to_the_pipe_body() {
    fn echo<S: ByteStream>(stream: &S) {
        let line = <S as ByteStream>::read(stream, 64).unwrap();
        if let Input::Data(bytes) = line {
            <S as ByteStream>::write_all(stream, bytes).unwrap();
        }
        <S as ByteStream>::close(stream).unwrap();
    }
    let (client, server): (PipeStream, PipeStream) = Pipe::pair();
    client.write_all(b"echo".to_vec()).unwrap();
    client.close().unwrap();
    <PipeStream as ByteStream>::set_idle_timeout(&server, 5000).unwrap();
    echo(&server);
    assert_eq!(client.read(64).unwrap(), data(b"echo"));
    assert_eq!(client.read(64).unwrap(), Input::Eof);
}

/// Run `body` on its own thread and fail when it has not finished within
/// `LIMIT`: a regression that leaves a read waiting (a lost flush, a lost
/// wakeup) fails the case instead of hanging the suite. The body runs
/// whole on that thread, so its flush points are the ones it would have.
fn bounded<T: Send + 'static>(name: &str, body: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, finished) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = done.send(body());
    });
    match finished.recv_timeout(LIMIT) {
        Ok(value) => {
            let _ = worker.join();
            value
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => match worker.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(()) => panic!("{name}: the body ended without a value"),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("{name}: still waiting after {LIMIT:?}"),
    }
}

/// Every byte delivered and unread, up to `maximum`: two deliveries, the
/// first read in part, then one read large enough for the rest. A TCP
/// read returns all the bytes the socket holds; a read that returns part
/// of the queue while the rest sits in it makes the bytes of each read
/// depend on the queue's ring layout.
#[test]
fn adv_a_read_returns_every_queued_byte_up_to_maximum() {
    bounded("queued bytes", || {
        for first in 2..40usize {
            for taken in 1..first {
                for second in 1..40usize {
                    let (client, server) = Pipe::pair();
                    client.write_all(vec![1; first]).unwrap();
                    ignite_io::flush();
                    assert_eq!(server.read(taken as i64).unwrap(), data(&vec![1; taken]));
                    client.write_all(vec![2; second]).unwrap();
                    ignite_io::flush();
                    let mut expected = vec![1; first - taken];
                    expected.extend(vec![2; second]);
                    assert_eq!(
                        server.read(1 << 20).unwrap(),
                        data(&expected),
                        "first {first}, read {taken}, second {second}"
                    );
                }
            }
        }
    });
}

/// D6 at its boundary: one delivery of exactly `PIPE_CAPACITY` fits, one
/// byte more does not, and a single write of `PIPE_CAPACITY + 1` queues
/// nothing and counts nothing.
#[test]
fn adv_the_capacity_is_exact() {
    bounded("capacity", || {
        let (client, server) = Pipe::pair();
        client.write_all(vec![1; PIPE_CAPACITY]).unwrap();
        assert_eq!(server.received_batches(), 1);
        client.write_all(vec![2]).unwrap();
        ignite_io::flush();
        assert_eq!(server.received_batches(), 1);
        assert_eq!(error(client.read(1)), "pipe queue full");
        let mut part = vec![0; 1 << 20];
        assert_eq!(server.read_into(&mut part[..1]).unwrap(), 1);
        client.write_all(vec![3]).unwrap();
        ignite_io::flush();
        assert_eq!(
            server.received_batches(),
            2,
            "one byte freed, one byte fits"
        );

        let (client, server) = Pipe::pair();
        assert_eq!(
            error(client.write_all(vec![1; PIPE_CAPACITY + 1])),
            "pipe queue full"
        );
        client.close().unwrap();
        assert_eq!(server.read(64).unwrap(), Input::Eof, "nothing queued");
        assert_eq!(server.received_batches(), 0);
    });
}

/// Many small writes before a flush are one batch; writes that cross
/// `SEND_BUFFER` deliver once there and once more at the flush.
#[test]
fn adv_many_small_writes_coalesce() {
    bounded("small writes", || {
        let (client, server) = Pipe::pair();
        for _ in 0..1000 {
            client.write_from(&[5; 10]).unwrap();
        }
        assert_eq!(server.received_batches(), 0);
        ignite_io::flush();
        ignite_io::flush();
        assert_eq!(server.received_batches(), 1);
        for _ in 0..7000 {
            client.write_from(&[6; 10]).unwrap();
        }
        assert_eq!(server.received_batches(), 2, "the threshold delivered once");
        ignite_io::flush();
        assert_eq!(server.received_batches(), 3, "the rest at the flush");
        client.write_all(Vec::new()).unwrap();
        ignite_io::flush();
        assert_eq!(
            server.received_batches(),
            3,
            "an empty buffer is not counted"
        );
        client.close().unwrap();
        let mut total = 0;
        while let Input::Data(bytes) = server.read(1 << 20).unwrap() {
            total += bytes.len();
        }
        assert_eq!(total, 80_000);
        assert_eq!(
            client.received_batches(),
            0,
            "writes count on the peer only"
        );
    });
}

/// A peer that closed only its write half still reads: a write to it is
/// delivered, not refused.
#[test]
fn adv_a_write_after_the_peer_closed_reaches_it() {
    bounded("peer closed", || {
        let (client, server) = Pipe::pair();
        server.close().unwrap();
        assert_eq!(client.read(16).unwrap(), Input::Eof);
        client.write_all(b"still".to_vec()).unwrap();
        client.close().unwrap();
        assert_eq!(server.read(16).unwrap(), data(b"still"));
        assert_eq!(server.read(16).unwrap(), Input::Eof);
    });
}

/// Dropping an end with unread bytes: they are discarded, the peer reads
/// `Eof`, and the peer's next delivery fails as to a vanished peer.
#[test]
fn adv_dropping_an_end_with_unread_bytes() {
    bounded("unread drop", || {
        let (client, server) = Pipe::pair();
        client.write_all(b"unread".to_vec()).unwrap();
        ignite_io::flush();
        drop(server);
        client.write_all(b"more".to_vec()).unwrap();
        ignite_io::flush();
        assert_eq!(error(client.read(16)), "pipe peer is gone");
        assert_eq!(client.read(16).unwrap(), Input::Eof);
        client.close().expect("the kept error was reported once");
    });
}

/// A read that waits on an end does not hold up a write or a close on the
/// same end, from another thread; the peer's close then ends the read.
#[test]
fn adv_a_waiting_read_does_not_hold_up_a_write_on_the_same_end() {
    bounded("read and write", || {
        let (client, server) = Pipe::pair();
        let reading = server.clone();
        let reader = std::thread::spawn(move || reading.read(64));
        std::thread::sleep(Duration::from_millis(50));
        server.write_all(b"w".to_vec()).unwrap();
        server.close().unwrap();
        assert_eq!(client.read(64).unwrap(), data(b"w"));
        assert_eq!(client.read(64).unwrap(), Input::Eof);
        client.write_all(b"r".to_vec()).unwrap();
        client.close().unwrap();
        assert_eq!(reader.join().unwrap().unwrap(), data(b"r"));
    });
}

/// Two readers of one end are serialised: every byte is read once.
#[test]
fn adv_two_readers_of_one_end_read_every_byte_once() {
    bounded("two readers", || {
        let (client, server) = Pipe::pair();
        let readers: Vec<_> = (0..2)
            .map(|_| {
                let end = server.clone();
                std::thread::spawn(move || {
                    let mut got = Vec::new();
                    while let Input::Data(bytes) = end.read(7).unwrap() {
                        got.extend(bytes);
                    }
                    got
                })
            })
            .collect();
        let mut sent = Vec::new();
        for round in 0..200u32 {
            let chunk: Vec<u8> = (0..37).map(|i| ((round * 37 + i) % 251) as u8).collect();
            sent.extend(&chunk);
            client.write_all(chunk).unwrap();
            ignite_io::flush();
        }
        client.close().unwrap();
        let mut got: Vec<u8> = readers
            .into_iter()
            .flat_map(|reader| reader.join().unwrap())
            .collect();
        got.sort_unstable();
        sent.sort_unstable();
        assert_eq!(got, sent);
    });
}

/// A plain thread that wrote and ended delivers at its end (its pending
/// list's drop), as for a `TcpStream`.
#[test]
fn adv_a_plain_thread_end_delivers() {
    bounded("thread end", || {
        let (client, server) = Pipe::pair();
        let writer = client.clone();
        std::thread::spawn(move || writer.write_all(b"t".to_vec()).unwrap())
            .join()
            .unwrap();
        assert_eq!(server.received_batches(), 1);
        assert_eq!(server.read(8).unwrap(), data(b"t"));
        drop(client);
        assert_eq!(server.read(8).unwrap(), Input::Eof);
    });
}

/// The thread's kept read buffer: a short read after a long one on the
/// same thread returns only the new bytes, never the tail of the earlier
/// read, and an idle timeout after a long read is `Eof`, not stale bytes.
#[test]
fn adv_a_short_read_after_a_long_read_returns_only_the_new_bytes() {
    bounded("short after long", || {
        let (client, server) = Pipe::pair();
        client.write_all(vec![7; 65536]).unwrap();
        ignite_io::flush();
        assert_eq!(server.read(65536).unwrap(), data(&[7; 65536]));
        client.write_all(b"new".to_vec()).unwrap();
        ignite_io::flush();
        assert_eq!(server.read(65536).unwrap(), data(b"new"));
        server.set_idle_timeout(20).unwrap();
        assert_eq!(server.read(65536).unwrap(), Input::Eof);
        client.write_all(b"z".to_vec()).unwrap();
        ignite_io::flush();
        assert_eq!(server.read(4).unwrap(), data(b"z"));
    });
}

/// The same across tasks: tasks run on reused worker threads, and a task
/// whose short read follows another task's long read gets only its own
/// bytes.
#[test]
fn adv_a_later_task_reads_none_of_an_earlier_tasks_bytes() {
    bounded("later task", || {
        for _ in 0..8 {
            let (done, results) = mpsc::channel();
            let (client, server) = Pipe::pair();
            let long = done.clone();
            let reader = server.clone();
            client.write_all(vec![9; 60000]).unwrap();
            ignite_io::flush();
            ignite_io::spawn(move || {
                let _ = long.send(reader.read(65536).unwrap());
            });
            assert_eq!(results.recv().unwrap(), data(&[9; 60000]));
            client.write_all(b"ab".to_vec()).unwrap();
            ignite_io::flush();
            ignite_io::spawn(move || {
                let _ = done.send(server.read(65536).unwrap());
            });
            assert_eq!(results.recv().unwrap(), data(b"ab"));
        }
    });
}

/// Reads around the kept buffer's size of 64 KiB: each takes exactly
/// `maximum` bytes of the queue (0 and negative as 1), in order.
#[test]
fn adv_reads_at_the_kept_buffer_boundary_take_exactly_maximum() {
    bounded("boundary", || {
        let (client, server) = Pipe::pair();
        let sent: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        client.write_all(sent.clone()).unwrap();
        ignite_io::flush();
        let mut received = Vec::new();
        for (maximum, expected) in [
            (0, 1),
            (-5, 1),
            (1, 1),
            (65536, 65536),
            (65537, 65537),
            (65535, 65535),
            (65536, 65536),
            (1 << 20, 300_000 - 3 - 4 * 65536),
        ] {
            let Input::Data(bytes) = server.read(maximum).unwrap() else {
                panic!("maximum {maximum}: Eof");
            };
            assert_eq!(bytes.len(), expected, "maximum {maximum}");
            received.extend(bytes);
        }
        assert_eq!(received, sent);
    });
}
