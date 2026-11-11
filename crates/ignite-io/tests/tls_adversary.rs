//! Adversary cases of `story:tls-client` (`docs/slice.md`
//! 38):`TlsStream::client` against local TLS servers that
//! misbehave, with authorities generated in the test; loopback only, no
//! network. Every client call runs on its own thread under a deadline, so
//! a hang is a red case and not a stuck suite. The cases under both IO
//! modes run in a child process of this executable, as in `scheduler.rs`.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::{ByteStream, Input, IoError, TcpStream, TlsOptions, TlsStream};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

const LIMIT: Duration = Duration::from_secs(20);
const CHILD: &str = "IGNITE_TLS_ADVERSARY_CHILD";

struct Authority {
    issuer: CertifiedIssuer<'static, KeyPair>,
}

#[derive(Clone, Copy)]
enum Validity {
    Now,
    NotYet,
}

impl Authority {
    fn new() -> Authority {
        Authority::named("ignite adversary authority")
    }

    fn named(name: &str) -> Authority {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.distinguished_name.push(DnType::CommonName, name);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let key = KeyPair::generate().unwrap();
        Authority {
            issuer: CertifiedIssuer::self_signed(params, key).unwrap(),
        }
    }

    fn pem(&self) -> String {
        self.issuer.pem()
    }

    fn server(&self, names: &[&str], validity: Validity) -> Identity {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let mut params = CertificateParams::new(names).unwrap();
        match validity {
            Validity::Now => {}
            Validity::NotYet => {
                params.not_before = rcgen::date_time_ymd(2090, 1, 1);
                params.not_after = rcgen::date_time_ymd(2091, 1, 1);
            }
        }
        let key = KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.issuer).unwrap();
        Identity {
            chain: vec![cert.der().clone(), self.issuer.der().clone()],
            key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        }
    }
}

/// A leaf for `names` signed by its own key, no authority behind it.
fn self_signed(names: &[&str]) -> (Identity, String) {
    let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    let params = CertificateParams::new(names).unwrap();
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    let pem = cert.pem();
    (
        Identity {
            chain: vec![cert.der().clone()],
            key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        },
        pem,
    )
}

struct Identity {
    chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

#[derive(Clone)]
enum Behaviour {
    /// Echo until the client's close_notify, then close_notify back.
    Echo,
    /// The handshake, then wait `ms`, then echo as `Echo`.
    DelayedEcho(u64),
    /// The handshake, then `data`, then a TCP close without close_notify.
    Truncate(&'static [u8]),
    /// The handshake, then `data`, then a TCP reset.
    Reset(&'static [u8]),
    /// The handshake, then nothing for `LIMIT`.
    QuietAfterHandshake,
    /// Read the ClientHello, then a record header promising 16 KiB and one
    /// byte of it every 50 ms.
    Trickle,
    /// Read the ClientHello, then shut down the write half.
    HangUp,
}

struct Server {
    port: i64,
}

fn serve_with(config: rustls::ServerConfig, behaviour: Behaviour) -> Server {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = i64::from(listener.local_addr().unwrap().port());
    let config = Arc::new(config);
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        match behaviour {
            Behaviour::Trickle => {
                let mut hello = [0u8; 4096];
                let _ = socket.read(&mut hello);
                if socket.write_all(&[0x16, 0x03, 0x03, 0x40, 0x00]).is_err() {
                    return;
                }
                let until = Instant::now() + LIMIT;
                while Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(50));
                    if socket.write_all(&[0]).is_err() {
                        return;
                    }
                }
            }
            Behaviour::HangUp => {
                let mut hello = [0u8; 4096];
                let _ = socket.read(&mut hello);
                let _ = socket.shutdown(Shutdown::Write);
                std::thread::sleep(Duration::from_secs(3));
            }
            other => {
                let connection = rustls::ServerConnection::new(config).unwrap();
                let mut tls = rustls::StreamOwned::new(connection, socket);
                while tls.conn.is_handshaking() {
                    if tls.conn.complete_io(&mut tls.sock).is_err() {
                        return;
                    }
                }
                match other {
                    Behaviour::Truncate(data) => {
                        let _ = tls.write_all(data);
                        let _ = tls.flush();
                        std::thread::sleep(Duration::from_millis(100));
                        let _ = tls.sock.shutdown(Shutdown::Both);
                    }
                    Behaviour::Reset(data) => {
                        let _ = tls.write_all(data);
                        let _ = tls.flush();
                        std::thread::sleep(Duration::from_millis(100));
                        let linger = nix::libc::linger {
                            l_onoff: 1,
                            l_linger: 0,
                        };
                        let _ = nix::sys::socket::setsockopt(
                            &tls.sock,
                            nix::sys::socket::sockopt::Linger,
                            &linger,
                        );
                        drop(tls);
                    }
                    Behaviour::QuietAfterHandshake => std::thread::sleep(LIMIT),
                    Behaviour::DelayedEcho(ms) => {
                        std::thread::sleep(Duration::from_millis(ms));
                        echo(tls);
                    }
                    _ => echo(tls),
                }
            }
        }
    });
    Server { port }
}

fn echo(mut tls: rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>) {
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        match tls.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tls.write_all(&buffer[..n]).is_err() {
                    return;
                }
            }
        }
    }
    tls.conn.send_close_notify();
    let _ = tls.flush();
}

fn config(identity: Identity, alpn: &[&str]) -> rustls::ServerConfig {
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(identity.chain, identity.key)
        .unwrap();
    config.alpn_protocols = alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
    config
}

fn serve(identity: Identity, behaviour: Behaviour) -> Server {
    serve_with(config(identity, &[]), behaviour)
}

fn trusting(authority: &Authority) -> TlsOptions {
    TlsOptions {
        authorities: authority.pem(),
        ..TlsOptions::default()
    }
}

fn connect(port: i64) -> TcpStream {
    TcpStream::connect("127.0.0.1".to_string(), port).unwrap()
}

/// `run` on a thread, its result within `LIMIT`, else a panic naming the
/// hang.
fn within<T: Send + 'static>(what: &str, run: impl FnOnce() -> T + Send + 'static) -> T {
    let (done, got) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(run());
    });
    got.recv_timeout(LIMIT)
        .unwrap_or_else(|_| panic!("{what}: no result within {LIMIT:?}"))
}

fn open(port: i64, name: &str, options: TlsOptions) -> Result<TlsStream, IoError> {
    let name = name.to_string();
    within("the handshake", move || {
        TlsStream::client(connect(port), name, options)
    })
}

fn refusal(result: Result<TlsStream, IoError>) -> String {
    match result {
        Ok(_) => "the handshake succeeded".to_string(),
        Err(error) => error.message,
    }
}

/// Every read of `stream` until end of stream or an error.
fn reads<S: ByteStream>(stream: &S) -> (Vec<u8>, Result<(), String>) {
    let mut all = Vec::new();
    loop {
        match stream.read(64 * 1024) {
            Ok(Input::Data(bytes)) => all.extend(bytes),
            Ok(Input::Eof) => return (all, Ok(())),
            Err(e) => return (all, Err(e.message)),
        }
    }
}

// ---------------------------------------------------------------------
// 1. Verification
// ---------------------------------------------------------------------

#[test]
fn a_certificate_not_yet_valid_is_refused() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::NotYet),
        Behaviour::Echo,
    );
    assert_eq!(
        refusal(open(server.port, "localhost", trusting(&authority))),
        "tls localhost: certificate refused: not yet valid"
    );
}

#[test]
fn a_self_signed_leaf_is_refused_by_the_system_roots() {
    let (identity, _) = self_signed(&["localhost"]);
    let server = serve(identity, Behaviour::Echo);
    assert_eq!(
        refusal(open(server.port, "localhost", TlsOptions::default())),
        "tls localhost: certificate refused: unknown issuer"
    );
}

#[test]
fn a_self_signed_leaf_is_refused_by_another_authority() {
    let authority = Authority::new();
    let (identity, _) = self_signed(&["localhost"]);
    let server = serve(identity, Behaviour::Echo);
    assert_eq!(
        refusal(open(server.port, "localhost", trusting(&authority))),
        "tls localhost: certificate refused: unknown issuer"
    );
}

#[test]
fn a_chain_to_an_unknown_authority_is_refused() {
    let theirs = Authority::named("ignite adversary authority, theirs");
    let ours = Authority::named("ignite adversary authority, ours");
    let server = serve(
        theirs.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    assert_eq!(
        refusal(open(server.port, "localhost", trusting(&ours))),
        "tls localhost: certificate refused: unknown issuer"
    );
}

#[test]
fn an_ip_literal_is_not_matched_by_a_dns_name_in_the_certificate() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    assert_eq!(
        refusal(open(server.port, "127.0.0.1", trusting(&authority))),
        "tls 127.0.0.1: certificate refused: not valid for 127.0.0.1"
    );
}

#[test]
fn a_dns_name_is_not_matched_by_an_ip_address_in_the_certificate() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["127.0.0.1"], Validity::Now),
        Behaviour::Echo,
    );
    assert_eq!(
        refusal(open(server.port, "localhost", trusting(&authority))),
        "tls localhost: certificate refused: not valid for localhost"
    );
}

/// Names that are no DNS name and no IP address never reach a handshake:
/// the listener answers nothing, so a name that did would time out.
#[test]
fn odd_server_names_are_refused_before_the_handshake() {
    let authority = Authority::new();
    for name in [
        "",
        "*.example.test",
        "*",
        "local\0host",
        "localhost/x",
        " localhost",
        "local host",
        "..",
    ] {
        let listener = ignite_io::TcpListener::bind("127.0.0.1".into(), 0).unwrap();
        let options = TlsOptions {
            handshake_timeout: 200_000_000,
            ..trusting(&authority)
        };
        let got = refusal(open(listener.local_port(), name, options));
        assert_eq!(
            got,
            format!("tls: invalid server name `{name}`"),
            "name {name:?}"
        );
    }
}

#[test]
fn a_wildcard_certificate_covers_one_label_only() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["*.example.test"], Validity::Now),
        Behaviour::Echo,
    );
    assert_eq!(
        refusal(open(server.port, "a.b.example.test", trusting(&authority))),
        "tls a.b.example.test: certificate refused: not valid for a.b.example.test"
    );
}

#[test]
fn the_server_name_matches_without_regard_to_case() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let tls = open(server.port, "LocalHost", trusting(&authority)).unwrap();
    tls.close().unwrap();
}

#[test]
fn a_trailing_dot_does_not_let_another_name_through() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let got = refusal(open(server.port, "other.test.", trusting(&authority)));
    assert!(
        got.starts_with("tls other.test.: certificate refused: ")
            || got.starts_with("tls: invalid server name"),
        "{got}"
    );
}

/// Authorities that hold only whitespace or only a key are no fallback to
/// the system roots and no fallback to `verify = false`.
#[test]
fn authorities_without_a_certificate_never_turn_verification_off() {
    let key = KeyPair::generate().unwrap().serialize_pem();
    for authorities in ["\n", " ", key.as_str()] {
        let server = serve(self_signed(&["localhost"]).0, Behaviour::Echo);
        let options = TlsOptions {
            authorities: authorities.to_string(),
            ..TlsOptions::default()
        };
        assert_eq!(
            refusal(open(server.port, "localhost", options)),
            "tls localhost: no certificate in the authorities",
            "authorities {authorities:?}"
        );
    }
}

#[test]
fn alpn_without_a_common_protocol_fails_the_handshake() {
    let authority = Authority::new();
    let server = serve_with(
        config(authority.server(&["localhost"], Validity::Now), &["h2"]),
        Behaviour::Echo,
    );
    let options = TlsOptions {
        alpn: vec!["http/1.1".into()],
        ..trusting(&authority)
    };
    let got = refusal(open(server.port, "localhost", options));
    assert!(got.starts_with("tls localhost: "), "{got}");
    assert!(!got.contains("succeeded"), "{got}");
}

/// RFC 7301: a server may ignore ALPN; the session goes on and `alpn()` is
/// empty, which the caller has to check.
#[test]
fn a_server_without_alpn_leaves_the_choice_empty() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let options = TlsOptions {
        alpn: vec!["h2".into()],
        ..trusting(&authority)
    };
    let tls = open(server.port, "localhost", options).unwrap();
    assert_eq!(tls.alpn(), "");
}

#[test]
fn a_tls12_only_server_is_verified_too() {
    let authority = Authority::new();
    let identity = authority.server(&["localhost"], Validity::Now);
    let config = rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
        .with_no_client_auth()
        .with_single_cert(identity.chain, identity.key)
        .unwrap();
    let server = serve_with(config, Behaviour::Echo);
    assert_eq!(
        refusal(open(server.port, "other.test", trusting(&authority))),
        "tls other.test: certificate refused: not valid for other.test"
    );
}

// ---------------------------------------------------------------------
// 2. Handshake deadline
// ---------------------------------------------------------------------

#[test]
fn a_server_that_trickles_bytes_is_cut_at_the_deadline() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Trickle,
    );
    let options = TlsOptions {
        handshake_timeout: 300_000_000,
        ..trusting(&authority)
    };
    let started = Instant::now();
    let got = refusal(open(server.port, "localhost", options));
    let waited = started.elapsed();
    assert_eq!(got, "tls localhost: handshake timed out after 300 ms");
    assert!(waited < Duration::from_secs(3), "{waited:?}");
}

#[test]
fn a_server_that_hangs_up_mid_handshake_fails_at_once() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::HangUp,
    );
    let started = Instant::now();
    let got = refusal(open(server.port, "localhost", trusting(&authority)));
    let waited = started.elapsed();
    assert_eq!(
        got,
        "tls localhost: the server closed the connection during the handshake"
    );
    assert!(waited < Duration::from_secs(3), "{waited:?}");
}

/// The handshake's deadline is not left behind as the stream's idle
/// timeout: a stream without one reads data that comes after the
/// handshake timeout has passed.
#[test]
fn the_handshake_timeout_does_not_outlive_the_handshake() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::DelayedEcho(700),
    );
    let options = TlsOptions {
        handshake_timeout: 200_000_000,
        ..trusting(&authority)
    };
    let tls = open(server.port, "localhost", options).unwrap();
    let (data, end) = within("the exchange", move || {
        tls.write_all(b"late".to_vec()).unwrap();
        tls.close().unwrap();
        reads(&tls)
    });
    assert_eq!(end, Ok(()));
    assert_eq!(data, b"late");
}

/// An idle timeout set before the handshake is the stream's after it.
#[test]
fn the_idle_timeout_set_before_the_handshake_is_restored() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::QuietAfterHandshake,
    );
    let stream = connect(server.port);
    stream.set_idle_timeout(150).unwrap();
    let tls = within("the handshake", move || {
        TlsStream::client(stream, "localhost".into(), trusting(&authority))
    })
    .unwrap();
    let started = Instant::now();
    let read = within("the idle read", move || tls.read(16).map_err(|e| e.message));
    let waited = started.elapsed();
    assert_eq!(read, Ok(Input::Eof));
    assert!(waited < Duration::from_secs(3), "{waited:?}");
}

// ---------------------------------------------------------------------
// 3. ByteStream contract
// ---------------------------------------------------------------------

#[test]
fn a_tcp_close_without_close_notify_is_an_error_not_an_end_of_stream() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Truncate(b"partial"),
    );
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    let (data, end) = within("the reads", move || reads(&tls));
    assert_eq!(data, b"partial");
    assert_eq!(
        end,
        Err("tls localhost: the server closed the connection without close_notify".to_string())
    );
}

#[test]
fn a_reset_is_an_error_not_an_end_of_stream() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Reset(b"partial"),
    );
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    let (_, end) = within("the reads", move || reads(&tls));
    let end = end.expect_err("a reset read as an end of stream");
    assert!(end.starts_with("tls localhost: "), "{end}");
}

#[test]
fn a_tls12_truncation_is_an_error_too() {
    let authority = Authority::new();
    let identity = authority.server(&["localhost"], Validity::Now);
    let config = rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS12])
        .with_no_client_auth()
        .with_single_cert(identity.chain, identity.key)
        .unwrap();
    let server = serve_with(config, Behaviour::Truncate(b"partial"));
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    let (data, end) = within("the reads", move || reads(&tls));
    assert_eq!(data, b"partial");
    assert!(
        end.is_err(),
        "a TLS 1.2 truncation read as an end of stream"
    );
}

#[test]
fn a_zero_length_write_changes_nothing() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    let (data, end) = within("the exchange", move || {
        tls.write_all(Vec::new()).unwrap();
        tls.write_all(b"a".to_vec()).unwrap();
        tls.write_all(Vec::new()).unwrap();
        tls.close().unwrap();
        reads(&tls)
    });
    assert_eq!(end, Ok(()));
    assert_eq!(data, b"a");
}

#[test]
fn a_write_after_close_is_refused() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    tls.close().unwrap();
    assert_eq!(
        tls.write_all(b"x".to_vec()).unwrap_err().message,
        "tls localhost: write to a closed stream"
    );
    assert!(tls.close().is_ok(), "a second close is not an error");
}

/// 10 MiB in writes of 64 KiB (each more than one 16 KiB record) beside a
/// reader thread, back intact.
#[test]
fn ten_mebibytes_in_large_writes_beside_a_reader_come_back_intact() {
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let tls = open(server.port, "localhost", trusting(&authority)).unwrap();
    let reader = tls.clone();
    let (done, got) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(reads(&reader));
    });
    let block: Vec<u8> = (0..=250u8).cycle().take(10 * 1024 * 1024).collect();
    let sent = block.clone();
    within("the writes", move || {
        for chunk in sent.chunks(64 * 1024) {
            tls.write_all(chunk.to_vec()).unwrap();
        }
        tls.close().unwrap();
    });
    let (echoed, end) = got.recv_timeout(LIMIT).expect("the reader ends");
    assert_eq!(end, Ok(()));
    assert_eq!(echoed.len(), block.len());
    assert!(echoed == block, "the echo differs");
}

// ---------------------------------------------------------------------
// 2 and 3 under both IO modes, inside tasks
// ---------------------------------------------------------------------

fn child(name: &str, mode: Option<&str>) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS");
    if let Some(mode) = mode {
        command.env("IGNITE_IO", mode);
    }
    command.output().unwrap()
}

fn passed(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "child failed: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );
}

#[test]
fn in_tasks_under_readiness() {
    passed(&child("tls_adversary::child_in_tasks", None));
}

#[test]
fn in_tasks_under_threads() {
    passed(&child("tls_adversary::child_in_tasks", Some("threads")));
}

/// The deadline, the truncation and a large exchange, each in a spawned
/// task; the outcomes come back over a channel and are asserted here.
#[test]
fn child_in_tasks() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let authority = Arc::new(Authority::new());
    let (done, got) = mpsc::channel::<(&'static str, String)>();

    let trickle = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Trickle,
    );
    let truncate = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Truncate(b"partial"),
    );
    let echo_server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let quiet = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::QuietAfterHandshake,
    );

    {
        let done = done.clone();
        let options = TlsOptions {
            handshake_timeout: 300_000_000,
            ..trusting(&authority)
        };
        ignite_io::spawn(move || {
            let started = Instant::now();
            let got = refusal(TlsStream::client(
                connect(trickle.port),
                "localhost".into(),
                options,
            ));
            let waited = started.elapsed();
            let _ = done.send((
                "trickle",
                format!("{got}; under 3 s: {}", waited < Duration::from_secs(3)),
            ));
        });
    }
    {
        let done = done.clone();
        let options = trusting(&authority);
        ignite_io::spawn(move || {
            let outcome =
                match TlsStream::client(connect(truncate.port), "localhost".into(), options) {
                    Ok(tls) => format!("{:?}", reads(&tls)),
                    Err(e) => e.message,
                };
            let _ = done.send(("truncate", outcome));
        });
    }
    {
        let done = done.clone();
        let options = trusting(&authority);
        ignite_io::spawn(move || {
            let tls =
                match TlsStream::client(connect(echo_server.port), "localhost".into(), options) {
                    Ok(tls) => tls,
                    Err(e) => {
                        let _ = done.send(("echo", e.message));
                        return;
                    }
                };
            let reader = tls.clone();
            let (count, counted) = ignite_io::Channel::bounded::<String>(1);
            ignite_io::spawn(move || {
                let (data, end) = reads(&reader);
                let ok = data.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8);
                count.send(format!("{} bytes, in order {ok}, end {end:?}", data.len()));
            });
            let block: Vec<u8> = (0..=250u8).cycle().take(10 * 1024 * 1024).collect();
            for chunk in block.chunks(100 * 1024) {
                if let Err(e) = tls.write_all(chunk.to_vec()) {
                    let _ = done.send(("echo", e.message));
                    return;
                }
            }
            let _ = tls.close();
            let outcome = counted
                .receive()
                .unwrap_or_else(|| "the reader sent nothing".to_string());
            let _ = done.send(("echo", outcome));
        });
    }
    {
        let done = done.clone();
        let stream = connect(quiet.port);
        stream.set_idle_timeout(150).unwrap();
        let options = TlsOptions {
            handshake_timeout: 5_000_000_000,
            ..trusting(&authority)
        };
        ignite_io::spawn(move || {
            let outcome = match TlsStream::client(stream, "localhost".into(), options) {
                Ok(tls) => {
                    let started = Instant::now();
                    let read = tls.read(16);
                    format!(
                        "{:?}; under 3 s: {}",
                        read.map_err(|e| e.message),
                        started.elapsed() < Duration::from_secs(3)
                    )
                }
                Err(e) => e.message,
            };
            let _ = done.send(("idle", outcome));
        });
    }
    drop(done);
    let mut outcomes = std::collections::BTreeMap::new();
    let until = Instant::now() + LIMIT;
    while outcomes.len() < 4 {
        let left = until.saturating_duration_since(Instant::now());
        match got.recv_timeout(left) {
            Ok((name, outcome)) => {
                outcomes.insert(name, outcome);
            }
            Err(_) => break,
        }
    }
    let expected: std::collections::BTreeMap<&str, String> = [
        (
            "echo",
            format!("{} bytes, in order true, end Ok(())", 10 * 1024 * 1024),
        ),
        ("idle", "Ok(Eof); under 3 s: true".to_string()),
        (
            "trickle",
            "tls localhost: handshake timed out after 300 ms; under 3 s: true".to_string(),
        ),
        (
            "truncate",
            "([112, 97, 114, 116, 105, 97, 108], Err(\"tls localhost: the server closed the connection without close_notify\"))".to_string(),
        ),
    ]
    .into_iter()
    .collect();
    assert_eq!(outcomes, expected);
}

/// One writer task and one reader task on one session, `total` bytes in
/// writes of `chunk`, under the IO mode `mode` with `workers` workers (0:
/// the default). The child prints how far each side got when the echo did
/// not come back.
fn echo_in_tasks(mode: Option<&str>, workers: usize, total: usize, chunk: usize) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "tls_adversary::child_echo_in_tasks",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env("TLS_ADVERSARY_TOTAL", total.to_string())
        .env("TLS_ADVERSARY_CHUNK", chunk.to_string())
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS");
    if let Some(mode) = mode {
        command.env("IGNITE_IO", mode);
    }
    if workers > 0 {
        command.env("IGNITE_WORKERS", workers.to_string());
    }
    command.output().unwrap()
}

/// The control of [`ten_mebibytes_in_100_kib_writes_in_tasks_under_readiness`]:
/// the same tasks on a plain `TcpStream` to a plain echo server.
#[test]
fn control_ten_mebibytes_over_plain_tcp_in_tasks_under_readiness() {
    passed(&tcp_echo_in_tasks(false));
}

/// The root cause outside TLS: a short write left in the send buffer
/// before the large ones, as the handshake's last record is.
#[test]
fn control_plain_tcp_with_a_short_write_first_in_tasks_under_readiness() {
    passed(&tcp_echo_in_tasks(true));
}

fn tcp_echo_in_tasks(small_first: bool) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "tls_adversary::child_tcp_echo_in_tasks",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, "1")
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS");
    if small_first {
        command.env("TLS_ADVERSARY_SMALL_FIRST", "1");
    }
    command.output().unwrap()
}

#[test]
fn child_tcp_echo_in_tasks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let total = 10 * 1024 * 1024;
    let greeting = std::env::var_os("TLS_ADVERSARY_GREETING").is_some();
    let small_first = std::env::var_os("TLS_ADVERSARY_SMALL_FIRST").is_some();
    let expected = total + usize::from(small_first);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = i64::from(listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        if greeting {
            let _ = socket.write_all(b"hello");
        }
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            match socket.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if socket.write_all(&buffer[..n]).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = socket.shutdown(Shutdown::Write);
    });
    let written = Arc::new(AtomicUsize::new(0));
    let read = Arc::new(AtomicUsize::new(0));
    let (done, got) = mpsc::channel::<String>();
    {
        let (written, read) = (written.clone(), read.clone());
        ignite_io::spawn(move || {
            let tcp = connect(port);
            if greeting {
                let _ = tcp.read(5);
            }
            if small_first {
                // A short write stays in the send buffer, registered with
                // the worker thread's pending list, as the TLS handshake's
                // last record does.
                let _ = tcp.write_all(vec![0u8]);
            }
            let reader = tcp.clone();
            ignite_io::spawn(move || {
                let end = loop {
                    match reader.read(64 * 1024) {
                        Ok(Input::Data(bytes)) => {
                            read.fetch_add(bytes.len(), Ordering::SeqCst);
                        }
                        Ok(Input::Eof) => break Ok(()),
                        Err(e) => break Err(e.message),
                    }
                };
                let _ = done.send(format!(
                    "{} bytes, end {end:?}",
                    read.load(Ordering::SeqCst)
                ));
            });
            let block: Vec<u8> = (0..=250u8).cycle().take(total).collect();
            for piece in block.chunks(100 * 1024) {
                if tcp.write_all(piece.to_vec()).is_err() {
                    return;
                }
                written.fetch_add(piece.len(), Ordering::SeqCst);
            }
            let _ = tcp.close();
        });
    }
    let outcome = got.recv_timeout(LIMIT).unwrap_or_else(|_| {
        format!(
            "no outcome within {LIMIT:?}: written {} of {total}, read {}",
            written.load(Ordering::SeqCst),
            read.load(Ordering::SeqCst)
        )
    });
    assert_eq!(outcome, format!("{expected} bytes, end Ok(())"));
}

#[test]
fn ten_mebibytes_in_100_kib_writes_in_tasks_under_readiness() {
    passed(&echo_in_tasks(None, 0, 10 * 1024 * 1024, 100 * 1024));
}

#[test]
fn ten_mebibytes_in_100_kib_writes_in_tasks_under_threads() {
    passed(&echo_in_tasks(
        Some("threads"),
        0,
        10 * 1024 * 1024,
        100 * 1024,
    ));
}

#[test]
fn child_echo_in_tasks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    let number = |name: &str| -> usize { std::env::var(name).unwrap().parse().unwrap() };
    let total = number("TLS_ADVERSARY_TOTAL");
    let chunk = number("TLS_ADVERSARY_CHUNK");
    let authority = Authority::new();
    let server = serve(
        authority.server(&["localhost"], Validity::Now),
        Behaviour::Echo,
    );
    let written = Arc::new(AtomicUsize::new(0));
    let read = Arc::new(AtomicUsize::new(0));
    let (done, got) = mpsc::channel::<String>();
    {
        let (written, read) = (written.clone(), read.clone());
        let options = trusting(&authority);
        ignite_io::spawn(move || {
            let tls = match TlsStream::client(connect(server.port), "localhost".into(), options) {
                Ok(tls) => tls,
                Err(e) => {
                    let _ = done.send(e.message);
                    return;
                }
            };
            let reader = tls.clone();
            let reader_done = done.clone();
            ignite_io::spawn(move || {
                let mut ok = true;
                let end = loop {
                    match reader.read(64 * 1024) {
                        Ok(Input::Data(bytes)) => {
                            let at = read.fetch_add(bytes.len(), Ordering::SeqCst);
                            ok &= bytes
                                .iter()
                                .enumerate()
                                .all(|(i, b)| *b == ((at + i) % 251) as u8);
                        }
                        Ok(Input::Eof) => break Ok(()),
                        Err(e) => break Err(e.message),
                    }
                };
                let _ = reader_done.send(format!(
                    "{} bytes, in order {ok}, end {end:?}",
                    read.load(Ordering::SeqCst)
                ));
            });
            let block: Vec<u8> = (0..=250u8).cycle().take(total).collect();
            for piece in block.chunks(chunk) {
                if let Err(e) = tls.write_all(piece.to_vec()) {
                    let _ = done.send(format!("write: {}", e.message));
                    return;
                }
                written.fetch_add(piece.len(), Ordering::SeqCst);
            }
            if let Err(e) = tls.close() {
                let _ = done.send(format!("close: {}", e.message));
            }
        });
    }
    let outcome = got.recv_timeout(LIMIT).unwrap_or_else(|_| {
        format!(
            "no outcome within {LIMIT:?}: written {} of {total}, read {}",
            written.load(Ordering::SeqCst),
            read.load(Ordering::SeqCst)
        )
    });
    assert_eq!(outcome, format!("{total} bytes, in order true, end Ok(())"));
}
