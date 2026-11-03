//! The pipe entries of the C ABI (`docs/slice.md` 18.9), called as
//! generated code calls them: one handle table for pipes and TCP.

use ignite_rt::*;

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

fn message(error: &IgniteIoError) -> String {
    // SAFETY: produced by the runtime in this task's arena.
    String::from_utf8(unsafe { error.message.as_bytes() }.to_vec()).unwrap()
}

fn pair() -> (i64, i64) {
    let (mut first, mut second) = (0i64, 0i64);
    ignite_rt_pipe_pair(&mut first, &mut second);
    assert!(first > 0 && second > 0 && first != second);
    (first, second)
}

fn write(pipe: i64, data: &[u8]) {
    let mut err = no_error();
    let status = ignite_rt_pipe_write_all(pipe, &IgniteList::from_bytes(data), &mut err);
    assert_eq!(status, 0, "{}", message(&err));
}

/// The bytes of one read, or `None` at end of stream.
fn read(pipe: i64, maximum: i64) -> Option<Vec<u8>> {
    let (mut input, mut err) = (no_input(), no_error());
    let status = ignite_rt_pipe_read(pipe, maximum, &mut input, &mut err);
    assert_eq!(status, 0, "{}", message(&err));
    match input.tag {
        // SAFETY: produced by the runtime in this task's arena.
        0 => Some(unsafe { input.data.as_bytes() }.to_vec()),
        1 => None,
        tag => panic!("input tag {tag}"),
    }
}

#[test]
fn pair_write_read_and_batches_through_the_abi() {
    let (first, second) = pair();
    let mut err = no_error();
    assert_eq!(ignite_rt_pipe_set_idle_timeout(second, 5000, &mut err), 0);
    write(first, b"ab");
    write(first, b"cd");
    assert_eq!(ignite_rt_pipe_received_batches(second), 0);
    assert_eq!(read(second, 64).as_deref(), Some(&b"abcd"[..]));
    assert_eq!(ignite_rt_pipe_received_batches(second), 1);
    write(second, b"back");
    assert_eq!(
        read(first, 2).as_deref(),
        Some(&b"ba"[..]),
        "maximum bounds a read"
    );
    assert_eq!(read(first, 2).as_deref(), Some(&b"ck"[..]));
    assert_eq!(ignite_rt_pipe_received_batches(first), 1);
    assert_eq!(ignite_rt_pipe_close(first, &mut err), 0);
    assert_eq!(read(second, 64), None, "close ends the peer's stream");
    assert_eq!(
        ignite_rt_pipe_write_all(first, &IgniteList::from_bytes(b"x"), &mut err),
        1
    );
    assert_eq!(message(&err), "write to a closed stream");
    ignite_rt_tcp_release(first);
    ignite_rt_tcp_release(second);
    let mut err = no_error();
    assert_eq!(ignite_rt_pipe_close(first, &mut err), 1);
    assert!(message(&err).contains("was released"), "{}", message(&err));
}

/// `tcp_release` takes a pipe handle: releasing the end's only handle
/// delivers what it wrote and closes it, without a flush.
#[test]
fn release_of_the_last_handle_closes_the_peer() {
    let (first, second) = pair();
    write(first, b"last words");
    assert_eq!(ignite_rt_pipe_received_batches(second), 0);
    ignite_rt_tcp_own(first);
    ignite_rt_tcp_release(first);
    assert_eq!(ignite_rt_pipe_received_batches(second), 1);
    assert_eq!(read(second, 64).as_deref(), Some(&b"last words"[..]));
    assert_eq!(read(second, 64), None);
    let mut err = no_error();
    assert_eq!(
        ignite_rt_pipe_write_all(second, &IgniteList::from_bytes(b"?"), &mut err),
        0
    );
    ignite_rt_tcp_release(second);
}

#[test]
fn a_pipe_entry_refuses_a_tcp_handle() {
    let (mut listener, mut err) = (0i64, no_error());
    let address = IgniteList::from_bytes(b"127.0.0.1");
    assert_eq!(ignite_rt_tcp_bind(&address, 0, &mut listener, &mut err), 0);
    let port = ignite_rt_tcp_local_port(listener);
    let mut stream = 0i64;
    assert_eq!(
        ignite_rt_tcp_connect(&address, port, &mut stream, &mut err),
        0
    );
    let not_a_pipe = |handle: i64| format!("{handle} is not a pipe");

    let mut input = no_input();
    assert_eq!(ignite_rt_pipe_read(stream, 64, &mut input, &mut err), 1);
    assert_eq!(message(&err), not_a_pipe(stream));
    assert_eq!(input.tag, -1, "no input on an error");
    assert_eq!(
        ignite_rt_pipe_write_all(stream, &IgniteList::from_bytes(b"x"), &mut err),
        1
    );
    assert_eq!(message(&err), not_a_pipe(stream));
    assert_eq!(ignite_rt_pipe_close(listener, &mut err), 1);
    assert_eq!(message(&err), not_a_pipe(listener));
    assert_eq!(ignite_rt_pipe_set_idle_timeout(stream, 10, &mut err), 1);
    assert_eq!(message(&err), not_a_pipe(stream));
    assert_eq!(ignite_rt_pipe_received_batches(stream), 0);

    let (first, second) = pair();
    let not_tcp = |handle: i64| format!("{handle} is not a TCP stream");
    assert_eq!(ignite_rt_tcp_read(first, 64, &mut input, &mut err), 1);
    assert_eq!(message(&err), not_tcp(first));
    assert_eq!(
        ignite_rt_tcp_write_all(first, &IgniteList::from_bytes(b"x"), &mut err),
        1
    );
    assert_eq!(message(&err), not_tcp(first));
    assert_eq!(ignite_rt_tcp_close(second, &mut err), 1);
    assert_eq!(message(&err), not_tcp(second));
    assert_eq!(ignite_rt_tcp_set_idle_timeout(second, 10, &mut err), 1);
    assert_eq!(message(&err), not_tcp(second));
    let mut accepted = 0i64;
    assert_eq!(ignite_rt_tcp_accept(first, &mut accepted, &mut err), 1);
    assert_eq!(message(&err), format!("{first} is not a TCP listener"));
    assert_eq!(ignite_rt_tcp_local_port(first), 0);

    for handle in [first, second, stream, listener] {
        ignite_rt_tcp_release(handle);
    }
}

/// A released pipe handle stays released when its slot is reused by a new
/// pair: the old value names neither new end, and a second release of it
/// does nothing to the end that now holds the slot.
#[test]
fn adv_a_released_pipe_handle_is_not_its_slots_next_end() {
    let (first, second) = pair();
    ignite_rt_tcp_release(first);
    ignite_rt_tcp_release(first);
    let (third, fourth) = pair();
    assert!(third != first && fourth != first);
    let mut err = no_error();
    assert_eq!(
        ignite_rt_pipe_write_all(first, &IgniteList::from_bytes(b"x"), &mut err),
        1
    );
    assert_eq!(message(&err), format!("handle {first} was released"));
    assert_eq!(err.exit_code, 1);
    ignite_rt_tcp_release(first);
    write(third, b"ok");
    assert_eq!(ignite_rt_pipe_received_batches(first), 0);
    assert_eq!(read(fourth, 64).as_deref(), Some(&b"ok"[..]));
    assert_eq!(ignite_rt_pipe_received_batches(fourth), 1);
    assert_eq!(read(second, 64), None, "the released end's peer reads Eof");
    for handle in [second, third, fourth] {
        ignite_rt_tcp_release(handle);
    }
}

/// `maximum` is clamped to `1..=1 MiB` at the ABI as in `ignite-io`.
#[test]
fn adv_pipe_read_clamps_maximum() {
    let (first, second) = pair();
    write(first, b"ab");
    assert_eq!(read(second, 0).as_deref(), Some(&b"a"[..]));
    assert_eq!(read(second, -5).as_deref(), Some(&b"b"[..]));
    write(first, &vec![9; (1 << 20) + 10]);
    assert_eq!(
        read(second, i64::MAX).map(|bytes| bytes.len()),
        Some(1 << 20)
    );
    assert_eq!(read(second, i64::MAX).map(|bytes| bytes.len()), Some(10));
    ignite_rt_tcp_release(first);
    assert_eq!(read(second, 64), None);
    ignite_rt_tcp_release(second);
}

/// A vanished peer through the ABI: the write is buffered, the next
/// read's flush fails it, and that read returns the kept error with
/// status 1 and no input; the read after it is `Eof`.
#[test]
fn adv_a_vanished_peer_through_the_abi() {
    let (first, second) = pair();
    ignite_rt_tcp_release(second);
    write(first, b"x");
    let (mut input, mut err) = (no_input(), no_error());
    assert_eq!(ignite_rt_pipe_read(first, 64, &mut input, &mut err), 1);
    assert_eq!(message(&err), "pipe peer is gone");
    assert_eq!(input.tag, -1);
    assert_eq!(read(first, 64), None);
    ignite_rt_tcp_release(first);
}

/// Releasing an end while another thread's read on it waits: the waiting
/// read keeps the end alive and still reads what the peer sends; the end
/// is gone once that read ended.
#[test]
fn adv_release_during_a_waiting_read() {
    let (first, second) = pair();
    let reader = std::thread::spawn(move || read(second, 64));
    std::thread::sleep(std::time::Duration::from_millis(50));
    ignite_rt_tcp_release(second);
    write(first, b"late");
    ignite_io::flush();
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(reader.join());
    });
    let got = finished
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the waiting read ended")
        .unwrap();
    assert_eq!(got.as_deref(), Some(&b"late"[..]));
    write(first, b"gone");
    let (mut input, mut err) = (no_input(), no_error());
    assert_eq!(ignite_rt_pipe_read(first, 64, &mut input, &mut err), 1);
    assert_eq!(message(&err), "pipe peer is gone");
    ignite_rt_tcp_release(first);
}
