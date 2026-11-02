//! `ignite-load`: a load generator for the terminal example's `terminald`.
//!
//! ```text
//! ignite-load --workload rr|pipe|connect --connections N [--port P]
//!            [--server PATH | --pid PID] [--duration S] [--warmup S]
//!            [--payload BYTES] [--depth N] [--label TEXT] [--out FILE] [--quickack]
//! ```
//!
//! One std thread per connection. Every connection is opened and has seen
//! the prompt before the warm-up starts; replies completed during the
//! warm-up are not counted. Workloads:
//!
//! - `rr`: send `echo <payload>\r\n`, wait for `<payload>\r\nterminal> `,
//!   repeat. Latency is send to last byte of the prompt.
//! - `pipe`: keep up to `--depth` requests in flight per connection.
//!   Latency is send to last byte of the reply's prompt.
//! - `connect`: connect, wait for the prompt, send `quit\r\n`, wait for
//!   `bye\r\n` and the server's close. Latency is connect to close.
//!
//! Telnet: the server does not start negotiation; any `IAC DO x` is
//! answered `IAC WONT x`, `IAC WILL x` with `IAC DONT x`, other commands and
//! subnegotiations are dropped from the data.
//!
//! With `--server` the binary is started with `--port P` and killed at the
//! end; with `--pid` an already running server is measured. The result is
//! one JSON object on stdout (and appended to `--out`): replies/s, latency
//! percentiles, reply bytes/s, server CPU seconds in the measured window
//! (utime + stime from `/proc/<pid>/stat`, all threads including exited
//! ones), peak RSS (`VmHWM`), the largest thread count seen (sampled every
//! 50 ms), `nproc`, the 1-minute load average and the mean number of
//! runnable tasks on the machine in the second before the run.
//!
//! The client sets `TCP_NODELAY`. A server that writes one reply in two
//! sends on a socket without `TCP_NODELAY` has Nagle hold the second send
//! until the client acknowledges the first, which a delayed ACK does after
//! about 40 ms. `--quickack` sets `TCP_QUICKACK` on the client socket after
//! every read, which removes that stall from the client side and leaves the
//! server's own cost to be measured.
//!
//! `ignite-load matrix ...` runs a whole matrix of these runs against one or
//! more servers and prints a table per workload (`matrix.rs`).

mod matrix;

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

const PROMPT: &[u8] = b"terminal> ";
const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

static QUICKACK: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn setsockopt(fd: i32, level: i32, name: i32, value: *const i32, length: u32) -> i32;
}

const IPPROTO_TCP: i32 = 6;
const TCP_QUICKACK: i32 = 12;

fn quickack(socket: &TcpStream) {
    if QUICKACK.load(Ordering::Relaxed) {
        let on: i32 = 1;
        // SAFETY: a valid socket descriptor and a pointer to a live i32 of the given length.
        unsafe {
            setsockopt(socket.as_raw_fd(), IPPROTO_TCP, TCP_QUICKACK, &on, 4);
        }
    }
}

const WARMUP: u8 = 0;
const MEASURE: u8 = 1;
const STOP: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Workload {
    Rr,
    Pipe,
    Connect,
}

impl Workload {
    fn name(self) -> &'static str {
        match self {
            Workload::Rr => "rr",
            Workload::Pipe => "pipe",
            Workload::Connect => "connect",
        }
    }
}

struct Options {
    port: u16,
    workload: Workload,
    connections: usize,
    duration: f64,
    warmup: f64,
    payload: usize,
    depth: usize,
    server: Option<String>,
    pid: Option<u32>,
    label: String,
    out: Option<String>,
}

fn usage(message: &str) -> ! {
    eprintln!("ignite-load: {message}");
    eprintln!(
        "usage: ignite-load --workload rr|pipe|connect --connections N [--port P] [--server PATH | --pid PID] [--duration S] [--warmup S] [--payload BYTES] [--depth N] [--label TEXT] [--out FILE] [--quickack]"
    );
    std::process::exit(2);
}

fn parse_options() -> Options {
    let mut options = Options {
        port: 2400,
        workload: Workload::Rr,
        connections: 1,
        duration: 10.0,
        warmup: 1.0,
        payload: 64,
        depth: 64,
        server: None,
        pid: None,
        label: String::new(),
        out: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| usage(&format!("{flag} needs a value")))
        };
        match flag.as_str() {
            "--port" => options.port = value().parse().unwrap_or_else(|_| usage("bad --port")),
            "--workload" => {
                options.workload = match value().as_str() {
                    "rr" => Workload::Rr,
                    "pipe" => Workload::Pipe,
                    "connect" => Workload::Connect,
                    other => usage(&format!("unknown workload {other}")),
                }
            }
            "--connections" => {
                options.connections = value()
                    .parse()
                    .unwrap_or_else(|_| usage("bad --connections"))
            }
            "--duration" => {
                options.duration = value().parse().unwrap_or_else(|_| usage("bad --duration"))
            }
            "--warmup" => {
                options.warmup = value().parse().unwrap_or_else(|_| usage("bad --warmup"))
            }
            "--payload" => {
                options.payload = value().parse().unwrap_or_else(|_| usage("bad --payload"))
            }
            "--depth" => options.depth = value().parse().unwrap_or_else(|_| usage("bad --depth")),
            "--server" => options.server = Some(value()),
            "--pid" => options.pid = Some(value().parse().unwrap_or_else(|_| usage("bad --pid"))),
            "--label" => options.label = value(),
            "--out" => options.out = Some(value()),
            "--quickack" => QUICKACK.store(true, Ordering::Relaxed),
            other => usage(&format!("unknown argument {other}")),
        }
    }
    if options.connections == 0 || options.depth == 0 {
        usage("--connections and --depth must be positive");
    }
    if options.server.is_some() && options.pid.is_some() {
        usage("--server and --pid exclude each other");
    }
    options
}

/// A connection with a Telnet filter in front of its receive buffer.
struct Connection {
    socket: TcpStream,
    raw: Vec<u8>,
    data: Vec<u8>,
    start: usize,
    telnet: TelnetState,
}

#[derive(Clone, Copy)]
enum TelnetState {
    Ground,
    Interpret,
    Negotiation(u8),
    Subnegotiation,
    SubnegotiationInterpret,
}

#[derive(Debug)]
enum Failure {
    Io(std::io::Error),
    Mismatch,
    Closed,
}

impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Failure {
        Failure::Io(error)
    }
}

impl Connection {
    fn open(address: SocketAddr) -> Result<Connection, Failure> {
        let socket = TcpStream::connect(address)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_nodelay(true)?;
        quickack(&socket);
        Ok(Connection {
            socket,
            raw: vec![0; 64 * 1024],
            data: Vec::with_capacity(64 * 1024),
            start: 0,
            telnet: TelnetState::Ground,
        })
    }

    fn available(&self) -> usize {
        self.data.len() - self.start
    }

    /// One read; the data part is appended to the buffer. `Ok(false)` at
    /// end of stream.
    fn fill(&mut self) -> Result<bool, Failure> {
        if self.start > 0 && self.start == self.data.len() {
            self.data.clear();
            self.start = 0;
        } else if self.start > 32 * 1024 {
            self.data.drain(..self.start);
            self.start = 0;
        }
        let count = loop {
            match self.socket.read(&mut self.raw) {
                Ok(count) => {
                    quickack(&self.socket);
                    break count;
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        };
        if count == 0 {
            return Ok(false);
        }
        if matches!(self.telnet, TelnetState::Ground) && !self.raw[..count].contains(&IAC) {
            self.data.extend_from_slice(&self.raw[..count]);
            return Ok(true);
        }
        let mut answers = Vec::new();
        for index in 0..count {
            let b = self.raw[index];
            self.telnet = match self.telnet {
                TelnetState::Ground if b == IAC => TelnetState::Interpret,
                TelnetState::Ground => {
                    self.data.push(b);
                    TelnetState::Ground
                }
                TelnetState::Interpret if b == IAC => {
                    self.data.push(IAC);
                    TelnetState::Ground
                }
                TelnetState::Interpret if (WILL..=DONT).contains(&b) => TelnetState::Negotiation(b),
                TelnetState::Interpret if b == SB => TelnetState::Subnegotiation,
                TelnetState::Interpret => TelnetState::Ground,
                TelnetState::Negotiation(verb) => {
                    if verb == DO {
                        answers.extend_from_slice(&[IAC, WONT, b]);
                    } else if verb == WILL {
                        answers.extend_from_slice(&[IAC, DONT, b]);
                    }
                    TelnetState::Ground
                }
                TelnetState::Subnegotiation if b == IAC => TelnetState::SubnegotiationInterpret,
                TelnetState::Subnegotiation => TelnetState::Subnegotiation,
                TelnetState::SubnegotiationInterpret if b == SE => TelnetState::Ground,
                TelnetState::SubnegotiationInterpret => TelnetState::Subnegotiation,
            };
        }
        if !answers.is_empty() {
            self.socket.write_all(&answers)?;
        }
        Ok(true)
    }

    /// Wait for exactly `expected` and consume it.
    fn expect(&mut self, expected: &[u8]) -> Result<(), Failure> {
        while self.available() < expected.len() {
            if !self.fill()? {
                return Err(Failure::Closed);
            }
        }
        if &self.data[self.start..self.start + expected.len()] != expected {
            return Err(Failure::Mismatch);
        }
        self.start += expected.len();
        Ok(())
    }

    /// Wait for end of stream; any data before it is a mismatch.
    fn expect_close(&mut self) -> Result<(), Failure> {
        loop {
            if self.available() > 0 {
                return Err(Failure::Mismatch);
            }
            if !self.fill()? {
                return Ok(());
            }
        }
    }
}

#[derive(Default)]
struct Tally {
    replies: u64,
    reply_bytes: u64,
    latencies: Vec<u64>,
    errors: u64,
    first_error: Option<String>,
}

impl Tally {
    fn fail(&mut self, failure: Failure) {
        self.errors += 1;
        if self.first_error.is_none() {
            self.first_error = Some(match failure {
                Failure::Io(error) => error.to_string(),
                Failure::Mismatch => "unexpected reply".to_string(),
                Failure::Closed => "closed by the server".to_string(),
            });
        }
    }
}

fn payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|i| b"abcdefghijklmnopqrstuvwxyz0123456789"[i % 36])
        .collect()
}

fn echo_request(payload: &[u8]) -> Vec<u8> {
    let mut request = b"echo ".to_vec();
    request.extend_from_slice(payload);
    request.extend_from_slice(b"\r\n");
    request
}

fn echo_reply(payload: &[u8]) -> Vec<u8> {
    let mut reply = payload.to_vec();
    reply.extend_from_slice(b"\r\n");
    reply.extend_from_slice(PROMPT);
    reply
}

fn rr(address: SocketAddr, options: &Options, phase: &AtomicU8, ready: &Barrier) -> Tally {
    let mut tally = Tally::default();
    let payload = payload(options.payload);
    let request = echo_request(&payload);
    let reply = echo_reply(&payload);
    let opened = Connection::open(address).and_then(|mut c| c.expect(PROMPT).map(|()| c));
    ready.wait();
    let mut connection = match opened {
        Ok(connection) => connection,
        Err(failure) => {
            tally.fail(failure);
            return tally;
        }
    };
    loop {
        let started_in = phase.load(Ordering::Relaxed);
        if started_in == STOP {
            break;
        }
        let sent = Instant::now();
        if let Err(failure) = connection
            .socket
            .write_all(&request)
            .map_err(Failure::from)
            .and_then(|()| connection.expect(&reply))
        {
            tally.fail(failure);
            break;
        }
        let elapsed = sent.elapsed().as_nanos() as u64;
        if started_in == MEASURE && phase.load(Ordering::Relaxed) == MEASURE {
            tally.replies += 1;
            tally.reply_bytes += reply.len() as u64;
            tally.latencies.push(elapsed);
        }
    }
    tally
}

fn pipe(address: SocketAddr, options: &Options, phase: &AtomicU8, ready: &Barrier) -> Tally {
    let mut tally = Tally::default();
    let payload = payload(options.payload);
    let request = echo_request(&payload);
    let reply = echo_reply(&payload);
    let batch: Vec<u8> = request.repeat(options.depth);
    let opened = Connection::open(address).and_then(|mut c| c.expect(PROMPT).map(|()| c));
    ready.wait();
    let mut connection = match opened {
        Ok(connection) => connection,
        Err(failure) => {
            tally.fail(failure);
            return tally;
        }
    };
    let mut in_flight = std::collections::VecDeque::with_capacity(options.depth);
    let send = |connection: &mut Connection,
                in_flight: &mut std::collections::VecDeque<(Instant, u8)>,
                count: usize|
     -> Result<(), Failure> {
        let now = Instant::now();
        let started_in = phase.load(Ordering::Relaxed);
        for _ in 0..count {
            in_flight.push_back((now, started_in));
        }
        connection
            .socket
            .write_all(&batch[..count * request.len()])?;
        Ok(())
    };
    if let Err(failure) = send(&mut connection, &mut in_flight, options.depth) {
        tally.fail(failure);
        return tally;
    }
    loop {
        if phase.load(Ordering::Relaxed) == STOP {
            break;
        }
        match connection.fill() {
            Ok(true) => {}
            Ok(false) => {
                tally.fail(Failure::Closed);
                break;
            }
            Err(failure) => {
                tally.fail(failure);
                break;
            }
        }
        let mut done = 0;
        let now_phase = phase.load(Ordering::Relaxed);
        let now = Instant::now();
        while connection.available() >= reply.len() {
            let at = connection.start;
            if connection.data[at..at + reply.len()] != reply[..] {
                tally.fail(Failure::Mismatch);
                return tally;
            }
            connection.start += reply.len();
            let (sent, started_in) = in_flight.pop_front().unwrap_or((now, WARMUP));
            if started_in == MEASURE && now_phase == MEASURE {
                tally.replies += 1;
                tally.reply_bytes += reply.len() as u64;
                tally
                    .latencies
                    .push(now.duration_since(sent).as_nanos() as u64);
            }
            done += 1;
        }
        if done > 0
            && let Err(failure) = send(&mut connection, &mut in_flight, done)
        {
            tally.fail(failure);
            break;
        }
    }
    tally
}

fn connect(address: SocketAddr, phase: &AtomicU8, ready: &Barrier) -> Tally {
    let mut tally = Tally::default();
    ready.wait();
    loop {
        let started_in = phase.load(Ordering::Relaxed);
        if started_in == STOP {
            break;
        }
        let started = Instant::now();
        let session = Connection::open(address).and_then(|mut connection| {
            connection.expect(PROMPT)?;
            connection.socket.write_all(b"quit\r\n")?;
            connection.expect(b"bye\r\n")?;
            connection.expect_close()
        });
        match session {
            Ok(()) => {
                let elapsed = started.elapsed().as_nanos() as u64;
                if started_in == MEASURE && phase.load(Ordering::Relaxed) == MEASURE {
                    tally.replies += 1;
                    tally.reply_bytes += (PROMPT.len() + 5) as u64;
                    tally.latencies.push(elapsed);
                }
            }
            Err(failure) => {
                tally.fail(failure);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    tally
}

/// `utime + stime` of the process in clock ticks (USER_HZ, 100 on Linux).
fn cpu_ticks(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime + stime)
}

fn status_field(pid: u32, name: &str) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix(name)?.strip_prefix(':')?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

fn load_average() -> f64 {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|text| text.split_whitespace().next()?.parse().ok())
        .unwrap_or(-1.0)
}

/// Mean of the running-task count of `/proc/loadavg` over one second, the
/// reader itself excluded.
fn runnable_mean() -> f64 {
    let mut total = 0.0;
    let samples = 20;
    for _ in 0..samples {
        let running = fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|text| {
                let field = text.split_whitespace().nth(3)?;
                field.split('/').next()?.parse::<f64>().ok()
            })
            .unwrap_or(0.0);
        total += (running - 1.0).max(0.0);
        std::thread::sleep(Duration::from_millis(50));
    }
    total / samples as f64
}

struct Server {
    child: Child,
    session_errors: Arc<AtomicU64>,
}

fn start_server(path: &str, port: u16) -> Server {
    let mut child = Command::new(path)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| usage(&format!("cannot start {path}: {e}")));
    let stderr = child.stderr.take().expect("piped stderr");
    let session_errors = Arc::new(AtomicU64::new(0));
    let (listening, wait) = std::sync::mpsc::channel();
    let errors = Arc::clone(&session_errors);
    std::thread::spawn(move || {
        let mut listening = Some(listening);
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if line.starts_with("listening on") {
                if let Some(sender) = listening.take() {
                    let _ = sender.send(());
                }
            } else if line.starts_with("session ended with error") {
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    if wait.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        usage(&format!("{path} did not report listening within 10 s"));
    }
    Server {
        child,
        session_errors,
    }
}

fn percentile(sorted: &[u64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((q * sorted.len() as f64) as usize).min(sorted.len() - 1);
    sorted[index] as f64 / 1000.0
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("matrix") {
        matrix::main(std::env::args().skip(2));
        return;
    }
    let options = Arc::new(parse_options());
    let nproc = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let load1 = load_average();
    let runnable = runnable_mean();
    let address: SocketAddr = format!("127.0.0.1:{}", options.port)
        .parse()
        .expect("address");

    let mut server = options
        .server
        .as_deref()
        .map(|path| start_server(path, options.port));
    let pid = server
        .as_ref()
        .map(|server| server.child.id())
        .or(options.pid)
        .unwrap_or(0);

    let phase = Arc::new(AtomicU8::new(WARMUP));
    let ready = Arc::new(Barrier::new(options.connections + 1));
    let mut workers = Vec::with_capacity(options.connections);
    for _ in 0..options.connections {
        let options = Arc::clone(&options);
        let phase = Arc::clone(&phase);
        let ready = Arc::clone(&ready);
        let worker = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || match options.workload {
                Workload::Rr => rr(address, &options, &phase, &ready),
                Workload::Pipe => pipe(address, &options, &phase, &ready),
                Workload::Connect => connect(address, &phase, &ready),
            })
            .expect("worker thread");
        workers.push(worker);
    }

    let sampling = Arc::new(AtomicBool::new(true));
    let peak = Arc::new(Mutex::new((0u64, 0u64)));
    let sampler = {
        let sampling = Arc::clone(&sampling);
        let peak = Arc::clone(&peak);
        std::thread::spawn(move || {
            while sampling.load(Ordering::Relaxed) && pid != 0 {
                let threads = status_field(pid, "Threads").unwrap_or(0);
                let rss = status_field(pid, "VmRSS").unwrap_or(0);
                let mut peak = peak.lock().unwrap();
                peak.0 = peak.0.max(threads);
                peak.1 = peak.1.max(rss);
                drop(peak);
                std::thread::sleep(Duration::from_millis(50));
            }
        })
    };

    ready.wait();
    std::thread::sleep(Duration::from_secs_f64(options.warmup));
    let cpu_start = cpu_ticks(pid);
    let measure_start = Instant::now();
    phase.store(MEASURE, Ordering::Relaxed);
    std::thread::sleep(Duration::from_secs_f64(options.duration));
    phase.store(STOP, Ordering::Relaxed);
    let elapsed = measure_start.elapsed().as_secs_f64();
    let cpu_end = cpu_ticks(pid);
    let hwm = status_field(pid, "VmHWM").unwrap_or(0);
    sampling.store(false, Ordering::Relaxed);
    let _ = sampler.join();

    let mut total = Tally::default();
    for worker in workers {
        let tally = worker.join().unwrap_or_default();
        total.replies += tally.replies;
        total.reply_bytes += tally.reply_bytes;
        total.errors += tally.errors;
        if total.first_error.is_none() {
            total.first_error = tally.first_error;
        }
        total.latencies.extend(tally.latencies);
    }
    let session_errors = server
        .as_ref()
        .map(|server| server.session_errors.load(Ordering::Relaxed))
        .unwrap_or(0);
    if let Some(server) = server.as_mut() {
        let _ = server.child.kill();
        let _ = server.child.wait();
    }

    total.latencies.sort_unstable();
    let cpu_seconds = match (cpu_start, cpu_end) {
        (Some(a), Some(b)) => (b - a) as f64 / 100.0,
        _ => -1.0,
    };
    let rate = total.replies as f64 / elapsed;
    let cpu_ms_per_1000 = if total.replies > 0 {
        cpu_seconds * 1000.0 / (total.replies as f64 / 1000.0)
    } else {
        -1.0
    };
    let (threads, rss_peak_sampled) = *peak.lock().unwrap();
    let json = format!(
        concat!(
            "{{\"label\":\"{}\",\"quickack\":{},\"workload\":\"{}\",\"connections\":{},\"duration_s\":{:.3},",
            "\"replies\":{},\"replies_per_s\":{:.1},\"reply_mb_per_s\":{:.3},",
            "\"p50_us\":{:.1},\"p99_us\":{:.1},\"p999_us\":{:.1},",
            "\"server_cpu_s\":{:.2},\"cpu_ms_per_1000\":{:.3},\"peak_rss_kib\":{},",
            "\"rss_sampled_kib\":{},\"threads_max\":{},\"errors\":{},\"first_error\":\"{}\",",
            "\"server_session_errors\":{},\"nproc\":{},\"load1\":{:.2},\"runnable_mean\":{:.2}}}"
        ),
        options.label,
        QUICKACK.load(Ordering::Relaxed),
        options.workload.name(),
        options.connections,
        elapsed,
        total.replies,
        rate,
        total.reply_bytes as f64 / elapsed / 1e6,
        percentile(&total.latencies, 0.50),
        percentile(&total.latencies, 0.99),
        percentile(&total.latencies, 0.999),
        cpu_seconds,
        cpu_ms_per_1000,
        hwm,
        rss_peak_sampled,
        threads,
        total.errors,
        total
            .first_error
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace('"', "'"),
        session_errors,
        nproc,
        load1,
        runnable,
    );
    println!("{json}");
    if let Some(path) = &options.out {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap_or_else(|e| usage(&format!("cannot open {path}: {e}")));
        let _ = writeln!(file, "{json}");
    }
}
