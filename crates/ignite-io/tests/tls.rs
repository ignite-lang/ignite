//! TLS client sessions (`docs/slice.md`, section "TLS client sessions"):
//! `TlsStream::client` over a `TcpStream` against a TLS server built in
//! the test with a generated authority, on the loopback interface; no
//! network. Verified sessions carry bytes both ways as a `ByteStream`; a
//! wrong host name, an expired certificate, an unknown issuer, a silent
//! server and a peer that speaks no TLS each fail with an `IoError`.

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ignite_io::{ByteStream, Input, IoError, TcpStream, TlsOptions, TlsStream};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

const LIMIT: Duration = Duration::from_secs(10);

/// A certificate authority made for one test.
struct Authority {
    issuer: CertifiedIssuer<'static, KeyPair>,
}

impl Authority {
    fn new() -> Authority {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "ignite test authority");
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let key = KeyPair::generate().unwrap();
        Authority {
            issuer: CertifiedIssuer::self_signed(params, key).unwrap(),
        }
    }

    /// The authority's certificate as PEM text, what `TlsOptions` trusts.
    fn pem(&self) -> String {
        self.issuer.pem()
    }

    /// A server certificate for `names`, valid now or expired a year ago.
    fn server(&self, names: &[&str], expired: bool) -> Identity {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let mut params = CertificateParams::new(names).unwrap();
        if expired {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let key = KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.issuer).unwrap();
        Identity {
            chain: vec![cert.der().clone(), self.issuer.der().clone()],
            key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        }
    }
}

struct Identity {
    chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

/// What the test server does with one connection.
#[derive(Clone, Copy)]
enum Behaviour {
    /// TLS, then echo every byte until the client's close_notify, then
    /// close_notify back.
    Echo,
    /// Accept and say nothing.
    Silent,
    /// Answer the ClientHello with plain HTTP.
    Plain,
    /// Send the first byte of a handshake record 1.6 s after accepting,
    /// then nothing.
    LateByte,
}

/// A server on `127.0.0.1` for one connection; its port.
fn serve(identity: Identity, alpn: &[&str], behaviour: Behaviour) -> i64 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = i64::from(listener.local_addr().unwrap().port());
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(identity.chain, identity.key)
        .unwrap();
    config.alpn_protocols = alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
    let config = Arc::new(config);
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        match behaviour {
            Behaviour::Silent => {
                std::thread::sleep(LIMIT);
            }
            Behaviour::Plain => {
                let mut hello = [0u8; 512];
                let _ = socket.read(&mut hello);
                let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            }
            Behaviour::LateByte => {
                std::thread::sleep(Duration::from_millis(1600));
                let _ = socket.write_all(&[0x16]);
                std::thread::sleep(LIMIT);
            }
            Behaviour::Echo => {
                let connection = rustls::ServerConnection::new(config).unwrap();
                let mut tls = rustls::StreamOwned::new(connection, socket);
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
        }
    });
    port
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

fn message(result: Result<TlsStream, IoError>) -> String {
    match result {
        Ok(_) => panic!("the handshake succeeded"),
        Err(error) => error.message,
    }
}

/// Every byte `stream` reads until end of stream.
fn read_to_end<S: ByteStream>(stream: &S) -> Vec<u8> {
    let mut all = Vec::new();
    loop {
        match stream.read(64 * 1024).unwrap() {
            Input::Data(bytes) => all.extend(bytes),
            Input::Eof => return all,
        }
    }
}

#[test]
fn a_verified_session_carries_bytes_both_ways_as_a_byte_stream() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Echo,
    );
    let tls = TlsStream::client(connect(port), "localhost".into(), trusting(&authority)).unwrap();
    ByteStream::write_all(&tls, b"hello over tls".to_vec()).unwrap();
    ByteStream::close(&tls).unwrap();
    assert_eq!(read_to_end(&tls), b"hello over tls");
    assert_eq!(tls.read(16).unwrap(), Input::Eof);
    assert_eq!(tls.alpn(), "");
}

#[test]
fn a_wrong_host_name_is_refused() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Echo,
    );
    let refused = TlsStream::client(connect(port), "other.test".into(), trusting(&authority));
    assert_eq!(
        message(refused),
        "tls other.test: certificate refused: not valid for other.test"
    );
}

#[test]
fn an_expired_certificate_is_refused() {
    let authority = Authority::new();
    let port = serve(authority.server(&["localhost"], true), &[], Behaviour::Echo);
    let refused = TlsStream::client(connect(port), "localhost".into(), trusting(&authority));
    assert_eq!(
        message(refused),
        "tls localhost: certificate refused: expired"
    );
}

#[test]
fn verification_is_on_by_default_against_the_system_roots() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Echo,
    );
    let options = TlsOptions::default();
    assert!(options.verify);
    assert_eq!(options.authorities, "");
    let refused = TlsStream::client(connect(port), "localhost".into(), options);
    assert_eq!(
        message(refused),
        "tls localhost: certificate refused: unknown issuer"
    );
}

#[test]
fn without_verification_any_certificate_is_taken() {
    let authority = Authority::new();
    let port = serve(authority.server(&["localhost"], true), &[], Behaviour::Echo);
    let options = TlsOptions {
        verify: false,
        ..TlsOptions::default()
    };
    let tls = TlsStream::client(connect(port), "other.test".into(), options).unwrap();
    tls.write_all(b"x".to_vec()).unwrap();
    tls.close().unwrap();
    assert_eq!(read_to_end(&tls), b"x");
}

#[test]
fn an_ip_address_is_a_server_name() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["127.0.0.1"], false),
        &[],
        Behaviour::Echo,
    );
    let tls = TlsStream::client(connect(port), "127.0.0.1".into(), trusting(&authority)).unwrap();
    tls.close().unwrap();
    assert_eq!(read_to_end(&tls), b"");
}

#[test]
fn alpn_offers_the_list_and_reports_the_choice() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &["h2", "http/1.1"],
        Behaviour::Echo,
    );
    let options = TlsOptions {
        alpn: vec!["http/1.1".into()],
        ..trusting(&authority)
    };
    let tls = TlsStream::client(connect(port), "localhost".into(), options).unwrap();
    assert_eq!(tls.alpn(), "http/1.1");
}

#[test]
fn the_handshake_deadline_bounds_a_silent_server() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Silent,
    );
    let options = TlsOptions {
        handshake_timeout: 200_000_000,
        ..trusting(&authority)
    };
    let started = Instant::now();
    let refused = TlsStream::client(connect(port), "localhost".into(), options);
    let waited = started.elapsed();
    assert_eq!(
        message(refused),
        "tls localhost: handshake timed out after 200 ms"
    );
    assert!(waited >= Duration::from_millis(180), "{waited:?}");
    assert!(waited < Duration::from_secs(5), "{waited:?}");
}

#[test]
fn a_handshake_timeout_that_is_not_positive_is_refused() {
    let authority = Authority::new();
    let options = TlsOptions {
        handshake_timeout: 0,
        ..trusting(&authority)
    };
    let listener = ignite_io::TcpListener::bind("127.0.0.1".into(), 0).unwrap();
    let stream = connect(listener.local_port());
    assert_eq!(
        message(TlsStream::client(stream, "localhost".into(), options)),
        "tls localhost: handshake timeout 0 ms is not positive"
    );
}

#[test]
fn a_peer_that_speaks_no_tls_fails_with_an_io_error() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Plain,
    );
    let refused = message(TlsStream::client(
        connect(port),
        "localhost".into(),
        trusting(&authority),
    ));
    assert!(refused.starts_with("tls localhost: "), "{refused}");
    assert!(!refused.contains("certificate refused"), "{refused}");
}

#[test]
fn an_invalid_server_name_is_refused_before_the_handshake() {
    let authority = Authority::new();
    let listener = ignite_io::TcpListener::bind("127.0.0.1".into(), 0).unwrap();
    let stream = connect(listener.local_port());
    assert_eq!(
        message(TlsStream::client(
            stream,
            "not a name".into(),
            trusting(&authority)
        )),
        "tls: invalid server name `not a name`"
    );
}

#[test]
fn authorities_without_a_certificate_are_refused() {
    let listener = ignite_io::TcpListener::bind("127.0.0.1".into(), 0).unwrap();
    let stream = connect(listener.local_port());
    let options = TlsOptions {
        authorities: "no pem here".into(),
        ..TlsOptions::default()
    };
    assert_eq!(
        message(TlsStream::client(stream, "localhost".into(), options)),
        "tls localhost: no certificate in the authorities"
    );
}

/// Each handshake read waits for the time left before the deadline, not
/// the whole timeout again: a server that sends one byte of a record at
/// 1.6 s of a 2 s timeout, then nothing, is cut at about 2 s, where a
/// fresh 2 s wait after that byte would end at about 3.6 s.
#[test]
fn a_handshake_read_waits_only_for_the_time_left() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::LateByte,
    );
    let options = TlsOptions {
        handshake_timeout: 2_000_000_000,
        ..trusting(&authority)
    };
    let started = Instant::now();
    let refused = TlsStream::client(connect(port), "localhost".into(), options);
    let waited = started.elapsed();
    assert_eq!(
        message(refused),
        "tls localhost: handshake timed out after 2000 ms"
    );
    assert!(waited >= Duration::from_millis(1900), "{waited:?}");
    assert!(waited < Duration::from_millis(2900), "{waited:?}");
}

/// Signs with a key that is not the certificate's: the handshake
/// signature (TLS 1.3 CertificateVerify) does not match the certificate.
#[derive(Debug)]
struct WrongKey(Arc<rustls::sign::CertifiedKey>);

impl rustls::server::ResolvesServerCert for WrongKey {
    fn resolve(
        &self,
        _hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        Some(self.0.clone())
    }
}

/// `verify: false` takes any certificate, but the server must still prove
/// it holds the certificate's key: a handshake signature made with
/// another key is refused.
#[test]
fn without_verification_the_handshake_signature_is_still_checked() {
    let authority = Authority::new();
    let identity = authority.server(&["localhost"], false);
    let other = authority.server(&["localhost"], false);
    let signer = rustls::crypto::ring::sign::any_supported_type(&other.key).unwrap();
    let resolver = WrongKey(Arc::new(rustls::sign::CertifiedKey::new(
        identity.chain,
        signer,
    )));
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = i64::from(listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
        let mut tls = rustls::StreamOwned::new(connection, socket);
        let _ = tls.read(&mut [0u8; 64]);
    });
    let options = TlsOptions {
        verify: false,
        ..TlsOptions::default()
    };
    let refused = message(TlsStream::client(
        connect(port),
        "localhost".into(),
        options,
    ));
    assert_eq!(refused, "tls localhost: certificate refused: BadSignature");
}

#[test]
fn urgent_data_is_refused_on_a_tls_stream() {
    let authority = Authority::new();
    let port = serve(
        authority.server(&["localhost"], false),
        &[],
        Behaviour::Echo,
    );
    let tls = TlsStream::client(connect(port), "localhost".into(), trusting(&authority)).unwrap();
    assert!(!tls.urgent_pending().unwrap());
    assert_eq!(
        tls.write_urgent(b"!".to_vec()).unwrap_err().message,
        "tls localhost: urgent data is not carried by TLS"
    );
}
