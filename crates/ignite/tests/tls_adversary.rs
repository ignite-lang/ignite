//! Adversary cases of `story:tls-client` end to end: one Ignite program of
//! inline tests run by `ignite test` on both targets and under both IO
//! modes (`readiness`, the default, and `IGNITE_IO=threads`) against TLS
//! servers on the loopback interface with an authority generated here. No
//! network. The cases: the handshake deadline against a server that
//! trickles bytes, a cut without close_notify, the handshake timeout not
//! outliving the handshake, 10 MiB in writes larger than one record beside
//! a reader task, a zero-length write, a write after close, and the raw
//! stream used after `Tls.client` (the LLVM target keeps the stream's handle
//! with its owner; the Rust target must not move it away).

use std::io::{Read, Write};
use std::net::Shutdown;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use ignite::Target;
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use crate::common;

const PROGRAM: &str = r##"import std.net.{ByteStream}
import std.tls.{TlsOptions, certificate_refused}

const AUTHORITY: Text = r#"@AUTHORITY@"#
const ECHO: Int = @ECHO@
const TRUNCATE: Int = @TRUNCATE@
const TRICKLE: Int = @TRICKLE@
const DELAYED: Int = @DELAYED@

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

test "adversary: a trickling server is cut at the handshake deadline" {
  let started = Clock.monotonic()
  let opened = open(TRICKLE, "localhost", TlsOptions { authorities = AUTHORITY, handshake_timeout = 300 ms })
  let waited = Clock.monotonic() - started
  assert refusal(opened) == "tls localhost: handshake timed out after 300 ms"
  assert waited < 3 seconds
}

test "adversary: a cut without close_notify is an error, not an end of stream" {
  let tls = open(TRUNCATE, "localhost", trusted())?
  assert match read_all(tls) {
    Ok(_) => false
    Err(e) => e.message == "tls localhost: the server closed the connection without close_notify"
  }
}

test "adversary: the handshake timeout does not outlive the handshake" {
  let tls = open(DELAYED, "localhost", TlsOptions { authorities = AUTHORITY, handshake_timeout = 200 ms })?
  assert exchange(tls, b"late")? == b"late"
}

test "adversary: a zero-length write changes nothing" {
  let tls = open(ECHO, "localhost", trusted())?
  tls.write_all(Bytes.empty())?
  assert exchange(tls, b"a")? == b"a"
}

test "adversary: a write after close is refused" {
  let tls = open(ECHO, "localhost", trusted())?
  tls.close()?
  assert match tls.write_all(b"x") {
    Ok(_) => false
    Err(e) => e.message == "tls localhost: write to a closed stream"
  }
}

test "adversary: the raw stream still answers its peer after the handshake" {
  let stream = TcpStream.connect("127.0.0.1", ECHO)?
  let tls = Tls.client(stream, "localhost", trusted())?
  assert stream.peer().port == ECHO
  assert exchange(tls, b"x")? == b"x"
}

test fn count_echo(stream: TlsStream, done: Sender<Int>) -> Result<Unit, IoError> {
  let all = read_all(stream)?
  done.send(all.len())
  Ok(())
}

test "adversary: ten mebibytes in writes larger than a record beside a reader task" {
  let tls = open(ECHO, "localhost", trusted())?
  let (done, finished) = Channel.bounded(1)
  spawn count_echo(tls.copy(), done)
  var chunk = Bytes.empty()
  while chunk.len() < 102400 {
    chunk.push(b'y')
  }
  var i = 0
  while i < 103 {
    tls.write_all(chunk.copy())?
    i = i + 1
  }
  tls.close()?
  assert finished.receive_within(60 seconds) == Some(10547200)
}

test fn session(done: Sender<Int>) -> Result<Unit, IoError> {
  let tls = open(ECHO, "localhost", trusted())?
  spawn count_echo(tls.copy(), done)
  var chunk = Bytes.empty()
  while chunk.len() < 102400 {
    chunk.push(b'z')
  }
  var i = 0
  while i < 103 {
    tls.write_all(chunk.copy())?
    i = i + 1
  }
  tls.close()?
  Ok(())
}

test "adversary: a session task and the reader task it spawns carry ten mebibytes" {
  let (done, finished) = Channel.bounded(1)
  spawn session(done)
  assert finished.receive_within(60 seconds) == Some(10547200)
}
"##;

/// The number of inline tests in [`PROGRAM`].
const CASES: usize = 8;

struct Authority {
    issuer: CertifiedIssuer<'static, KeyPair>,
}

impl Authority {
    fn new() -> Authority {
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "ignite adversary authority");
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let key = KeyPair::generate().unwrap();
        Authority {
            issuer: CertifiedIssuer::self_signed(params, key).unwrap(),
        }
    }

    fn server(&self) -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
        let params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let key = KeyPair::generate().unwrap();
        let cert = params.signed_by(&key, &self.issuer).unwrap();
        (
            vec![cert.der().clone(), self.issuer.der().clone()],
            PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
    }
}

#[derive(Clone, Copy)]
enum Behaviour {
    Echo,
    DelayedEcho,
    Truncate,
    Trickle,
}

type Tls = rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>;

fn echo(mut tls: Tls) {
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

/// A server on `127.0.0.1`, one thread per connection, for the rest of
/// the test process.
fn server(
    identity: (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>),
    behaviour: Behaviour,
) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(identity.0, identity.1)
            .unwrap(),
    );
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else { continue };
            let config = config.clone();
            std::thread::spawn(move || {
                if let Behaviour::Trickle = behaviour {
                    let mut hello = [0u8; 4096];
                    let _ = socket.read(&mut hello);
                    if socket.write_all(&[0x16, 0x03, 0x03, 0x40, 0x00]).is_err() {
                        return;
                    }
                    for _ in 0..400 {
                        std::thread::sleep(Duration::from_millis(50));
                        if socket.write_all(&[0]).is_err() {
                            return;
                        }
                    }
                    return;
                }
                let connection = rustls::ServerConnection::new(config).unwrap();
                let mut tls = rustls::StreamOwned::new(connection, socket);
                while tls.conn.is_handshaking() {
                    if tls.conn.complete_io(&mut tls.sock).is_err() {
                        return;
                    }
                }
                match behaviour {
                    Behaviour::Truncate => {
                        let _ = tls.write_all(b"partial");
                        let _ = tls.flush();
                        std::thread::sleep(Duration::from_millis(100));
                        let _ = tls.sock.shutdown(Shutdown::Both);
                    }
                    Behaviour::DelayedEcho => {
                        std::thread::sleep(Duration::from_millis(700));
                        echo(tls);
                    }
                    _ => echo(tls),
                }
            });
        }
    });
    port
}

fn run(target: Target) {
    let authority = Authority::new();
    let source = PROGRAM
        .replace("@AUTHORITY@", &authority.issuer.pem())
        .replace(
            "@ECHO@",
            &server(authority.server(), Behaviour::Echo).to_string(),
        )
        .replace(
            "@TRUNCATE@",
            &server(authority.server(), Behaviour::Truncate).to_string(),
        )
        .replace(
            "@TRICKLE@",
            &server(authority.server(), Behaviour::Trickle).to_string(),
        )
        .replace(
            "@DELAYED@",
            &server(authority.server(), Behaviour::DelayedEcho).to_string(),
        );
    let name = match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    };
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let dir = tmp.join("tls_adversary").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("adversary.ignite");
    std::fs::write(&file, source).unwrap();
    let mut failures = Vec::new();
    for mode in ["readiness", "threads"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .arg("test")
            .arg(&file)
            .args(["--target", name, "--out"])
            .arg(dir.join("out"))
            .arg("--target-dir")
            .arg(tmp.join("tls").join(name).join("cargo"))
            .arg("--runtime")
            .arg(common::runner_archive())
            .env("IGNITE_IO", mode)
            .output()
            .expect("run ignite test");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.code() != Some(0)
            || !stdout.contains(&format!("test result: ok. {CASES} passed; 0 failed;"))
        {
            failures.push(format!(
                "IGNITE_IO={mode}: {:?}\n{stdout}\n{stderr}",
                output.status
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n----\n"));
}

mod rust {
    #[test]
    fn adversary_cases_run() {
        super::run(ignite::Target::Rust)
    }
}

mod llvm {
    #[test]
    fn adversary_cases_run() {
        super::run(ignite::Target::Llvm)
    }
}
