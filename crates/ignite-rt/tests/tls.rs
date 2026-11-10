//! The TLS entries of the C ABI (`docs/slice.md`, section "TLS client
//! sessions"), called as generated code calls them: the session is a
//! handle of the one table, and `std.tls.TlsOptions` crosses as
//! `IgniteTlsOptions`. The handshake itself is `ignite-io`'s and tested
//! there; these cases need no TLS server.

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

fn message(error: &IgniteIoError) -> String {
    // SAFETY: produced by the runtime in this task's arena.
    String::from_utf8(unsafe { error.message.as_bytes() }.to_vec()).unwrap()
}

fn options(alpn: &[&str], authorities: &str, timeout_ms: i64) -> IgniteTlsOptions {
    let alpn: Vec<String> = alpn.iter().map(|p| p.to_string()).collect();
    IgniteTlsOptions {
        alpn: IgniteList::from_texts(&alpn),
        authorities: IgniteList::from_bytes(authorities.as_bytes()),
        verify: 1,
        handshake_timeout: timeout_ms * 1_000_000,
    }
}

/// A TCP stream to a listener that accepts nothing and says nothing.
fn silent_stream() -> (i64, i64) {
    let (mut listener, mut stream, mut err) = (0i64, 0i64, no_error());
    let address = IgniteList::from_bytes(b"127.0.0.1");
    assert_eq!(ignite_rt_tcp_bind(&address, 0, &mut listener, &mut err), 0);
    let port = ignite_rt_tcp_local_port(listener);
    assert_eq!(
        ignite_rt_tcp_connect(&address, port, &mut stream, &mut err),
        0,
        "{}",
        message(&err)
    );
    (listener, stream)
}

/// The error of `tls_client` on `stream` for `name` with `options`.
fn refusal(stream: i64, name: &str, options: &IgniteTlsOptions) -> String {
    let (mut session, mut err) = (0i64, no_error());
    let name = IgniteList::from_bytes(name.as_bytes());
    let status = ignite_rt_tls_client(stream, &name, options, &mut session, &mut err);
    assert_eq!(status, 1, "the handshake succeeded");
    message(&err)
}

#[test]
fn every_field_of_the_options_record_reaches_the_session() {
    let (listener, stream) = silent_stream();
    assert_eq!(
        refusal(stream, "localhost", &options(&[], "", 100)),
        "tls localhost: handshake timed out after 100 ms"
    );
    assert_eq!(
        refusal(stream, "localhost", &options(&[], "no pem", 100)),
        "tls localhost: no certificate in the authorities"
    );
    assert_eq!(
        refusal(stream, "localhost", &options(&["h2", ""], "", 100)),
        "tls localhost: ALPN protocol `` is not 1 to 255 bytes"
    );
    assert_eq!(
        refusal(stream, "not a name", &options(&[], "", 100)),
        "tls: invalid server name `not a name`"
    );
    ignite_rt_tcp_release(stream);
    ignite_rt_tcp_release(listener);
}

#[test]
fn a_handle_of_another_kind_is_refused_by_name() {
    let (mut first, mut second) = (0i64, 0i64);
    ignite_rt_pipe_pair(&mut first, &mut second);
    assert_eq!(
        refusal(first, "localhost", &options(&[], "", 100)),
        format!("{first} is not a TCP stream")
    );
    let mut err = no_error();
    assert_eq!(ignite_rt_tls_close(first, &mut err), 1);
    assert_eq!(message(&err), format!("{first} is not a TLS stream"));
    let mut alpn = empty();
    ignite_rt_tls_alpn(&mut alpn, first);
    assert_eq!(alpn.len, 0);
    ignite_rt_tcp_release(first);
    ignite_rt_tcp_release(second);
}
