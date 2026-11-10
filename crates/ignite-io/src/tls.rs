//! TLS client sessions (`docs/slice.md`, section "TLS client sessions"):
//! [`TlsStream`], a rustls client session over a [`TcpStream`], and the
//! fourth [`crate::ByteStream`]. The substrate knows the TLS session, not
//! what is spoken over it (`AGENTS.md`, layout rules).
//!
//! The session is rustls' own state machine, which does no IO: the
//! ciphertext goes through the stream's `read_into` and `write_from`, so a
//! TLS read waits as a TCP read does (a task parks, a thread blocks or
//! polls), honours the stream's idle timeout, and a write joins the
//! stream's send buffer and its flush points. Three locks:
//!
//! - `state`, a `std::sync::Mutex` around the rustls connection, is held
//!   only for work that never waits (feeding ciphertext, taking
//!   plaintext, encrypting), so no task parks holding it;
//! - `reading` (a [`Lock`]) is held for a whole read, including its wait
//!   for ciphertext; it holds the read buffer;
//! - `writing` (a [`Lock`]) is held from taking ciphertext out of the
//!   connection until it is in the stream's send buffer, so records reach
//!   the socket in the order the connection made them. A read that must
//!   answer (an alert, a key update) takes it too, never while holding
//!   `state`.
//!
//! A reader task and a writer task can therefore share one session, as
//! they share a `TcpStream`.

use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore,
    SignatureScheme,
};

use crate::wait::Lock;
use crate::{Input, IoError, TcpStream, read_input};

/// The handshake timeout of [`TlsOptions::default`], in nanoseconds.
pub const HANDSHAKE_TIMEOUT: i64 = 10_000_000_000;

/// Ciphertext read from the socket at a time: one TLS record of 16 KiB
/// plaintext with its header and the largest expansion.
const RECORD: usize = 16 * 1024 + 2048;

/// The bundles read for the system roots when `SSL_CERT_FILE` is not set,
/// the first that exists (the paths of the Linux distributions' CA
/// packages).
const SYSTEM_BUNDLES: &[&str] = &[
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/ca-bundle.pem",
    "/etc/pki/tls/cacert.pem",
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
    "/etc/ssl/cert.pem",
];

/// The directories read for the system roots when no bundle was found and
/// `SSL_CERT_DIR` is not set.
const SYSTEM_DIRECTORIES: &[&str] = &["/etc/ssl/certs", "/etc/pki/tls/certs"];

/// What a client session verifies and offers: the substrate's form of the
/// record `std.tls.TlsOptions`, its fields in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsOptions {
    /// The protocols offered by ALPN, most preferred first; none when
    /// empty.
    pub alpn: Vec<String>,
    /// PEM certificates trusted instead of the system roots; the system
    /// roots when empty.
    pub authorities: String,
    /// Whether the server's certificate is verified at all.
    pub verify: bool,
    /// The longest the handshake may take, in nanoseconds.
    pub handshake_timeout: i64,
}

impl Default for TlsOptions {
    fn default() -> TlsOptions {
        TlsOptions {
            alpn: Vec::new(),
            authorities: String::new(),
            verify: true,
            handshake_timeout: HANDSHAKE_TIMEOUT,
        }
    }
}

/// A TLS client session over a TCP stream. Every handle is `Clone`; a
/// clone shares the session and the socket.
#[derive(Clone)]
pub struct TlsStream {
    inner: Arc<Session>,
}

struct Session {
    stream: TcpStream,
    /// The server name the session was opened for, in every message.
    name: String,
    /// The protocol the server chose by ALPN, empty for none.
    alpn: String,
    state: Mutex<State>,
    reading: Lock<Vec<u8>>,
    writing: Lock<()>,
}

struct State {
    connection: ClientConnection,
    /// Ciphertext read from the socket that the connection has not taken
    /// yet (it takes no more while it holds a record of plaintext unread).
    pending: Vec<u8>,
    /// `close` sent close_notify.
    closed: bool,
}

impl TlsStream {
    /// Perform the client handshake on `stream` for `server_name`, sent as
    /// SNI and the name the server's certificate must be valid for, within
    /// `options.handshake_timeout`. The stream's idle timeout is the one
    /// it had before when the call returns.
    pub fn client(
        stream: TcpStream,
        server_name: String,
        options: TlsOptions,
    ) -> Result<TlsStream, IoError> {
        let name = server_name;
        let server = ServerName::try_from(name.clone())
            .map_err(|_| IoError::new(format!("tls: invalid server name `{name}`")))?;
        let timeout_ms = options.handshake_timeout / 1_000_000;
        if options.handshake_timeout <= 0 {
            return Err(IoError::new(format!(
                "tls {name}: handshake timeout {} ms is not positive",
                timeout_ms.max(0)
            )));
        }
        let started = Instant::now();
        let deadline = started
            .checked_add(Duration::from_nanos(options.handshake_timeout as u64))
            .unwrap_or(started + Duration::from_secs(365 * 24 * 3600));
        let config = config(&name, &options)?;
        let connection =
            ClientConnection::new(Arc::new(config), server).map_err(|e| failure(&name, &e))?;
        let idle_before = stream.idle_ms().0.load(Ordering::Acquire);
        let mut session = Session {
            stream,
            name,
            alpn: String::new(),
            state: Mutex::new(State {
                connection,
                pending: Vec::new(),
                closed: false,
            }),
            reading: Lock::new(vec![0; RECORD]),
            writing: Lock::new(()),
        };
        let shaken = session.handshake(deadline, timeout_ms);
        let restored = session.stream.set_idle_timeout(idle_before);
        shaken?;
        restored.map_err(|e| session.error(&e.message))?;
        let alpn = session
            .lock()
            .connection
            .alpn_protocol()
            .map(<[u8]>::to_vec);
        session.alpn = alpn
            .map(|p| String::from_utf8_lossy(&p).into_owned())
            .unwrap_or_default();
        Ok(TlsStream {
            inner: Arc::new(session),
        })
    }

    /// The protocol the server chose from [`TlsOptions::alpn`], empty when
    /// it chose none.
    pub fn alpn(&self) -> String {
        self.inner.alpn.clone()
    }

    /// One read of up to `maximum` bytes of plaintext (clamped to
    /// `1..=1 MiB`). `Input::Eof` after the server's close_notify or when
    /// the stream's idle timeout passed; a connection closed without
    /// close_notify is an error.
    pub fn read(&self, maximum: i64) -> Result<Input, IoError> {
        read_input(maximum, |buffer| self.read_into(buffer))
    }

    /// One read into `buffer`, as [`TlsStream::read`]: the number of bytes,
    /// 0 for `Eof`.
    pub fn read_into(&self, buffer: &mut [u8]) -> Result<usize, IoError> {
        let session = &self.inner;
        let mut chunk = session.reading.lock();
        loop {
            let (read, answer) = {
                let mut state = session.lock();
                session.take_pending(&mut state)?;
                let read = state.connection.reader().read(buffer);
                (read, state.connection.wants_write())
            };
            if answer {
                session.send_pending()?;
            }
            match read {
                Ok(count) => return Ok(count),
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Err(
                        session.error("the server closed the connection without close_notify")
                    );
                }
                Err(e) => return Err(session.error(&e.to_string())),
            }
            let count = session
                .stream
                .read_into(&mut chunk)
                .map_err(|e| session.error(&e.message))?;
            let mut state = session.lock();
            if count == 0 {
                if session.stream.timed_out() {
                    return Ok(0);
                }
                let _ = state.connection.read_tls(&mut &[][..]);
                if let Err(e) = state.connection.process_new_packets() {
                    return Err(failure(&session.name, &e));
                }
            } else {
                state.pending.extend_from_slice(&chunk[..count]);
            }
        }
    }

    /// Write all of `data`: encrypt it and append the records to the
    /// stream's send buffer. Refused after `close`.
    pub fn write_all(&self, data: Vec<u8>) -> Result<(), IoError> {
        self.write_from(&data)
    }

    /// [`TlsStream::write_all`] of a borrowed slice.
    pub fn write_from(&self, data: &[u8]) -> Result<(), IoError> {
        let session = &self.inner;
        let _writing = session.writing.lock();
        let mut records = Vec::new();
        {
            let mut state = session.lock();
            if state.closed {
                return Err(session.error("write to a closed stream"));
            }
            let mut rest = data;
            let mut stalled = false;
            while !rest.is_empty() {
                let taken = state
                    .connection
                    .writer()
                    .write(rest)
                    .map_err(|e| session.error(&e.to_string()))?;
                drain(&mut state.connection, &mut records)?;
                if taken == 0 && stalled {
                    return Err(session.error("the session takes no more data"));
                }
                stalled = taken == 0;
                rest = &rest[taken..];
            }
        }
        session
            .stream
            .write_from(&records)
            .map_err(|e| session.error(&e.message))
    }

    /// Send close_notify and what was written, then close the write half
    /// of the stream. Reads continue until the server's close_notify. A
    /// second close is not an error.
    pub fn close(&self) -> Result<(), IoError> {
        let session = &self.inner;
        let _writing = session.writing.lock();
        let mut records = Vec::new();
        {
            let mut state = session.lock();
            if !state.closed {
                state.closed = true;
                state.connection.send_close_notify();
            }
            drain(&mut state.connection, &mut records)?;
        }
        if !records.is_empty() {
            session
                .stream
                .write_from(&records)
                .map_err(|e| session.error(&e.message))?;
        }
        session
            .stream
            .close()
            .map_err(|e| session.error(&e.message))
    }

    /// Set the idle timeout of the stream underneath, in milliseconds; 0
    /// or less removes it.
    pub fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        self.inner.stream.set_idle_timeout(ms)
    }

    /// Refused: TLS carries no urgent data.
    pub fn write_urgent(&self, _data: Vec<u8>) -> Result<(), IoError> {
        Err(self.inner.error("urgent data is not carried by TLS"))
    }

    /// Always false: TLS carries no urgent data.
    pub fn urgent_pending(&self) -> Result<bool, IoError> {
        Ok(false)
    }
}

impl Session {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `tls <name>: <message>`.
    fn error(&self, message: &str) -> IoError {
        IoError::new(format!("tls {}: {message}", self.name))
    }

    /// Give the connection what it takes of the ciphertext read before,
    /// and process it.
    fn take_pending(&self, state: &mut State) -> Result<(), IoError> {
        while !state.pending.is_empty() {
            let mut rest = &state.pending[..];
            let taken = match state.connection.read_tls(&mut rest) {
                Ok(taken) => taken,
                Err(_) => 0,
            };
            state.pending.drain(..taken);
            state
                .connection
                .process_new_packets()
                .map_err(|e| failure(&self.name, &e))?;
            if taken == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Send the records the connection has to send (an alert, a key
    /// update, a handshake message).
    fn send_pending(&self) -> Result<(), IoError> {
        let _writing = self.writing.lock();
        let mut records = Vec::new();
        drain(&mut self.lock().connection, &mut records)?;
        if records.is_empty() {
            return Ok(());
        }
        self.stream
            .write_from(&records)
            .map_err(|e| self.error(&e.message))
    }

    /// The handshake, reading with the time left before `deadline` as the
    /// stream's idle timeout. A failure sends the connection's alert.
    fn handshake(&self, deadline: Instant, timeout_ms: i64) -> Result<(), IoError> {
        let mut chunk = vec![0u8; RECORD];
        loop {
            self.send_pending()?;
            if !self.lock().connection.is_handshaking() {
                return Ok(());
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(self.timed_out(timeout_ms));
            }
            let ms = i64::try_from(left.as_millis().max(1)).unwrap_or(i64::MAX);
            self.stream
                .set_idle_timeout(ms)
                .map_err(|e| self.error(&e.message))?;
            let count = self
                .stream
                .read_into(&mut chunk)
                .map_err(|e| self.error(&e.message))?;
            if count == 0 {
                if self.stream.timed_out() {
                    return Err(self.timed_out(timeout_ms));
                }
                return Err(self.error("the server closed the connection during the handshake"));
            }
            let processed = {
                let mut state = self.lock();
                state.pending.extend_from_slice(&chunk[..count]);
                self.take_pending(&mut state)
            };
            if let Err(error) = processed {
                let _ = self.send_pending();
                crate::flush();
                return Err(error);
            }
        }
    }

    fn timed_out(&self, timeout_ms: i64) -> IoError {
        self.error(&format!("handshake timed out after {timeout_ms} ms"))
    }
}

/// Move every record `connection` has to send into `records`.
fn drain(connection: &mut ClientConnection, records: &mut Vec<u8>) -> Result<(), IoError> {
    while connection.wants_write() {
        connection
            .write_tls(records)
            .map_err(|e| IoError::new(format!("tls: {e}")))?;
    }
    Ok(())
}

/// The `IoError` of a failed handshake or record: `tls <name>:
/// certificate refused: <reason>` for a certificate the client does not
/// accept, else `tls <name>: <rustls' message>`.
fn failure(name: &str, error: &rustls::Error) -> IoError {
    match error {
        rustls::Error::InvalidCertificate(reason) => IoError::new(format!(
            "tls {name}: certificate refused: {}",
            certificate_reason(name, reason)
        )),
        other => IoError::new(format!("tls {name}: {other}")),
    }
}

fn certificate_reason(name: &str, reason: &CertificateError) -> String {
    match reason {
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => "expired".into(),
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            "not yet valid".into()
        }
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            format!("not valid for {name}")
        }
        CertificateError::UnknownIssuer => "unknown issuer".into(),
        CertificateError::Revoked => "revoked".into(),
        other => format!("{other:?}"),
    }
}

fn provider() -> Arc<CryptoProvider> {
    static PROVIDER: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    PROVIDER
        .get_or_init(|| Arc::new(rustls::crypto::ring::default_provider()))
        .clone()
}

/// The client configuration of `options` for `name`.
fn config(name: &str, options: &TlsOptions) -> Result<ClientConfig, IoError> {
    for protocol in &options.alpn {
        if protocol.is_empty() || protocol.len() > 255 {
            return Err(IoError::new(format!(
                "tls {name}: ALPN protocol `{protocol}` is not 1 to 255 bytes"
            )));
        }
    }
    let builder = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| IoError::new(format!("tls {name}: {e}")))?;
    let mut config = if !options.verify {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Unverified(provider())))
            .with_no_client_auth()
    } else if options.authorities.is_empty() {
        builder
            .with_root_certificates(system_roots()?)
            .with_no_client_auth()
    } else {
        builder
            .with_root_certificates(authorities(name, &options.authorities)?)
            .with_no_client_auth()
    };
    config.alpn_protocols = options.alpn.iter().map(|p| p.as_bytes().to_vec()).collect();
    Ok(config)
}

/// The certificates of the PEM text `pem`.
fn authorities(name: &str, pem: &str) -> Result<RootCertStore, IoError> {
    let mut store = RootCertStore::empty();
    for certificate in CertificateDer::pem_slice_iter(pem.as_bytes()) {
        let certificate =
            certificate.map_err(|e| IoError::new(format!("tls {name}: the authorities: {e}")))?;
        store
            .add(certificate)
            .map_err(|e| IoError::new(format!("tls {name}: the authorities: {e}")))?;
    }
    if store.is_empty() {
        return Err(IoError::new(format!(
            "tls {name}: no certificate in the authorities"
        )));
    }
    Ok(store)
}

/// The system roots, read once per process: `SSL_CERT_FILE` or the first
/// system bundle that exists, else every file of `SSL_CERT_DIR` (a list
/// separated by `:`) or of the system directories.
fn system_roots() -> Result<Arc<RootCertStore>, IoError> {
    static ROOTS: OnceLock<Result<Arc<RootCertStore>, String>> = OnceLock::new();
    ROOTS
        .get_or_init(read_system_roots)
        .clone()
        .map_err(IoError::new)
}

fn read_system_roots() -> Result<Arc<RootCertStore>, String> {
    let mut store = RootCertStore::empty();
    let file = match std::env::var_os("SSL_CERT_FILE") {
        Some(file) => Some(PathBuf::from(file)),
        None => SYSTEM_BUNDLES
            .iter()
            .map(PathBuf::from)
            .find(|path| path.is_file()),
    };
    if let Some(file) = &file {
        add_file(&mut store, file);
    }
    if store.is_empty() {
        let directories: Vec<PathBuf> = match std::env::var_os("SSL_CERT_DIR") {
            Some(list) => std::env::split_paths(&list).collect(),
            None => SYSTEM_DIRECTORIES.iter().map(PathBuf::from).collect(),
        };
        for directory in directories {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                add_file(&mut store, &entry.path());
            }
        }
    }
    if store.is_empty() {
        return Err(
            "tls: no system root certificates (SSL_CERT_FILE, SSL_CERT_DIR, /etc/ssl/certs)".into(),
        );
    }
    Ok(Arc::new(store))
}

/// Add every certificate of the PEM file `path` that parses.
fn add_file(store: &mut RootCertStore, path: &Path) {
    let Ok(mut file) = std::fs::File::open(path) else {
        return;
    };
    let mut pem = Vec::new();
    if file.read_to_end(&mut pem).is_err() {
        return;
    }
    for certificate in CertificateDer::pem_slice_iter(&pem).flatten() {
        let _ = store.add(certificate);
    }
}

/// The verifier of `verify: false`: any certificate for any name, the
/// handshake signatures still checked with the provider's algorithms.
#[derive(Debug)]
struct Unverified(Arc<CryptoProvider>);

impl ServerCertVerifier for Unverified {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
