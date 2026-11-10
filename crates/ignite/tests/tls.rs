//! TLS client sessions end to end (`docs/slice.md` 38):
//!`Tls.client` and `TlsStream` in inline tests run by
//! `ignite test` on both targets (`tls::rust::<case>`, `tls::llvm::<case>`)
//! against TLS servers this test starts on the loopback interface, with an
//! authority it generates. No network. The program is the template
//! [`SESSIONS`] with the authority's PEM and the servers' ports filled in.

use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use ignite::Target;
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::common;

/// The inline tests; `@AUTHORITY@`, `@ECHO@` and `@EXPIRED@` are replaced.
const SESSIONS: &str = r##"import std.net.{ByteStream}
import std.tls.{TlsOptions, certificate_refused}

const AUTHORITY: Text = r#"@AUTHORITY@"#
const ECHO: Int = @ECHO@
const EXPIRED: Int = @EXPIRED@

test fn trusted() -> TlsOptions = TlsOptions { authorities = AUTHORITY }

test fn open(port: Int, name: Text, options: TlsOptions) -> Result<TlsStream, IoError> {
  let stream = TcpStream.connect("127.0.0.1", port)?
  Tls.client(stream, name, options)
}

test fn read_all<S: ByteStream>(stream: S) -> Result<Bytes, IoError> {
  var all = Bytes.empty()
  var open = true
  while open {
    match stream.read(65536)? {
      Data(more) => all.append(more)
      Eof => {
        open = false
      }
    }
  }
  Ok(all)
}

test fn exchange<S: ByteStream>(stream: S, data: Bytes) -> Result<Bytes, IoError> {
  stream.write_all(data)?
  stream.close()?
  read_all(stream)
}

test fn refusal(opened: Result<TlsStream, IoError>) -> Text = match opened {
  Ok(_) => "the handshake succeeded"
  Err(e) => e.message
}

test "a verified session carries bytes both ways as a ByteStream" {
  let tls = open(ECHO, "localhost", trusted())?
  assert exchange(tls, b"hello over tls")? == b"hello over tls"
}

test "ALPN offers the list and reports the server's choice" {
  let chosen = open(ECHO, "localhost", TlsOptions { authorities = AUTHORITY, alpn = ["http/1.1"] })?
  assert chosen.alpn() == "http/1.1"
  let none = open(ECHO, "localhost", trusted())?
  assert none.alpn() == ""
}

test "a wrong host name is refused" {
  let refused = open(ECHO, "other.test", trusted())
  assert refusal(refused) == "tls other.test: certificate refused: not valid for other.test"
  assert match open(ECHO, "other.test", trusted()) {
    Ok(_) => false
    Err(e) => certificate_refused(e)
  }
}

test "an expired certificate is refused" {
  assert refusal(open(EXPIRED, "localhost", trusted())) == "tls localhost: certificate refused: expired"
}

test "verification is on by default, against the system roots" {
  assert refusal(open(ECHO, "localhost", TlsOptions {})) == "tls localhost: certificate refused: unknown issuer"
}

test "without verification an expired certificate is taken" {
  let tls = open(EXPIRED, "other.test", TlsOptions { verify = false })?
  assert exchange(tls, b"x")? == b"x"
}
"##;

/// The number of inline tests in [`SESSIONS`]. A reader task beside a
/// writer task on one session, with more bytes than the socket buffers
/// hold, is `crates/ignite/tests/tls_adversary.rs`'s, on both targets and
/// in both IO modes.
const CASES: usize = 6;

/// A certificate authority made for one run.
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

    /// The certificate chain and key of a server for `localhost`, valid
    /// now or expired since 2021.
    fn server(&self, expired: bool) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        let mut params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        if expired {
            params.not_before = rcgen::date_time_ymd(2020, 1, 1);
            params.not_after = rcgen::date_time_ymd(2021, 1, 1);
        }
        let key = KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.issuer).unwrap();
        (
            vec![cert.der().clone(), self.issuer.der().clone()],
            PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
    }
}

/// An echo server on `127.0.0.1`, one thread per connection, for the rest
/// of the test process: TLS with `identity`, ALPN `h2` and `http/1.1`,
/// every byte back until the client's close_notify, then close_notify.
fn echo_server(identity: (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(identity.0, identity.1)
        .unwrap();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let config = Arc::new(config);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { continue };
            let config = config.clone();
            std::thread::spawn(move || {
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
            });
        }
    });
    port
}

fn sessions_run(target: Target) {
    let authority = Authority::new();
    let echo = echo_server(authority.server(false));
    let expired = echo_server(authority.server(true));
    let source = SESSIONS
        .replace("@AUTHORITY@", &authority.issuer.pem())
        .replace("@ECHO@", &echo.to_string())
        .replace("@EXPIRED@", &expired.to_string());
    let name = match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("tls")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("sessions.ignite");
    std::fs::write(&file, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(&file)
        .args(["--target", name, "--out"])
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stdout}\n{stderr}");
    assert!(
        stdout.contains(&format!("test result: ok. {CASES} passed; 0 failed;")),
        "{stdout}"
    );
}

mod rust {
    #[test]
    fn sessions_run() {
        super::sessions_run(ignite::Target::Rust)
    }
}

mod llvm {
    #[test]
    fn sessions_run() {
        super::sessions_run(ignite::Target::Llvm)
    }
}
