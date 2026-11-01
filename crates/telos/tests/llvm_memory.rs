//! Memory of long sessions on the LLVM target (`docs/llvm.md` section 4):
//! 100 000 lines through one session, one of them 4 096 bytes long
//! (`MAXIMUM_LINE`), must not grow the resident set of `terminald` or `tn`
//! by 2 MB or more between line 10 000 and line 100 000.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use telos::Target;

mod common;

const TIMEOUT: Duration = Duration::from_secs(30);
const PROMPT: &[u8] = b"terminal> ";
const LINES: usize = 100_000;
const BASELINE: usize = 10_000;
const BATCH: usize = 1_000;
const LONG_AT: usize = 50_000;
const MAXIMUM_LINE: usize = 4096;
const BUDGET_KB: u64 = 2048;

struct Executables {
    terminald: PathBuf,
    tn: PathBuf,
}

fn executables() -> &'static Executables {
    static BUILT: OnceLock<Executables> = OnceLock::new();
    BUILT.get_or_init(|| {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/terminal/terminal.telos");
        let options = telos::BuildOptions {
            target: Target::Llvm,
            out: Some(Path::new(env!("CARGO_TARGET_TMPDIR")).join("memory-llvm")),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        };
        let output = telos::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
        Executables {
            terminald: output.executables[0].clone(),
            tn: output.executables[1].clone(),
        }
    })
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server() -> (Running, u16) {
    let mut child = Command::new(&executables().terminald)
        .args(["--port", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn terminald");
    let stderr = child.stderr.take().expect("piped stderr");
    let process = Running(child);
    let (ports, port) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stderr = std::io::BufReader::new(stderr);
        let mut line = String::new();
        while std::io::BufRead::read_line(&mut stderr, &mut line).unwrap_or(0) > 0 {
            if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                let _ = ports.send(rest.parse::<u16>());
            }
            line.clear();
        }
    });
    let port = port
        .recv_timeout(TIMEOUT)
        .expect("terminald printed no listening line")
        .expect("port in the listening line");
    (process, port)
}

/// Resident set size of `pid` in kB.
fn rss_kb(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).expect("proc status");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.trim().trim_end_matches("kB").trim().parse().ok())
        .expect("VmRSS in proc status")
}

/// Line `n` of the session: an `echo`, or at `LONG_AT` one of exactly
/// `MAXIMUM_LINE` bytes.
fn line(n: usize) -> Vec<u8> {
    let mut line = if n == LONG_AT {
        let mut long = b"echo ".to_vec();
        long.resize(MAXIMUM_LINE, b'x');
        long
    } else {
        format!("echo line {n}").into_bytes()
    };
    line.extend_from_slice(b"\r\n");
    line
}

/// Counts prompts in a byte stream split at arbitrary points.
struct Prompts {
    seen: usize,
    tail: Vec<u8>,
}

impl Prompts {
    fn new() -> Prompts {
        Prompts {
            seen: 0,
            tail: Vec::new(),
        }
    }

    fn feed(&mut self, chunk: &[u8]) {
        self.tail.extend_from_slice(chunk);
        let mut start = 0;
        while let Some(at) = self.tail[start..]
            .windows(PROMPT.len())
            .position(|window| window == PROMPT)
        {
            self.seen += 1;
            start += at + PROMPT.len();
        }
        let keep = (self.tail.len() - start).min(PROMPT.len() - 1);
        self.tail.drain(..self.tail.len() - keep);
    }

    /// Reads from `source` until `count` prompts were seen in total.
    fn until(&mut self, source: &mut impl Read, count: usize) {
        let mut chunk = [0u8; 65536];
        while self.seen < count {
            let read = source
                .read(&mut chunk)
                .unwrap_or_else(|e| panic!("no prompt {count} ({e})"));
            assert!(read > 0, "closed before prompt {count}");
            self.feed(&chunk[..read]);
        }
    }
}

/// Sends `LINES` lines in batches through `input`, reads the replies from
/// `output`, and returns the RSS of `pid` after line `BASELINE` and after
/// the last line.
fn drive(pid: u32, input: &mut impl Write, output: &mut impl Read) -> (u64, u64) {
    let mut prompts = Prompts::new();
    prompts.until(output, 1);
    let mut baseline = 0;
    let mut sent = 0;
    while sent < LINES {
        let mut batch = Vec::new();
        for n in sent + 1..=sent + BATCH {
            batch.extend_from_slice(&line(n));
        }
        input.write_all(&batch).expect("send a batch");
        input.flush().expect("flush a batch");
        sent += BATCH;
        prompts.until(output, sent + 1);
        if sent == BASELINE {
            baseline = rss_kb(pid);
        }
    }
    (baseline, rss_kb(pid))
}

#[track_caller]
fn assert_flat(name: &str, (baseline, last): (u64, u64)) {
    eprintln!(
        "{name}: VmRSS {baseline} kB at line {BASELINE}, {last} kB at line {LINES}, growth {} kB",
        last as i64 - baseline as i64
    );
    assert!(
        last < baseline + BUDGET_KB,
        "{name} grew from {baseline} kB to {last} kB between line {BASELINE} and line {LINES}"
    );
}

#[test]
fn terminald_memory_is_flat_over_a_long_session() {
    let (server, port) = start_server();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut reader = stream.try_clone().expect("clone the stream");
    let rss = drive(server.0.id(), &mut stream, &mut reader);
    assert_flat("terminald", rss);
}

#[test]
fn tn_memory_is_flat_over_a_long_session() {
    let (_server, port) = start_server();
    let mut child = Command::new(&executables().tn)
        .args(["--port", &port.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn tn");
    let mut input = child.stdin.take().expect("piped stdin");
    let output = child.stdout.take().expect("piped stdout");
    let tn = Running(child);
    let (chunks, received) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut output = output;
        let mut chunk = [0u8; 65536];
        while let Ok(read) = output.read(&mut chunk) {
            if read == 0 || chunks.send(chunk[..read].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut output = Received {
        chunks: received,
        pending: Vec::new(),
    };
    let rss = drive(tn.0.id(), &mut input, &mut output);
    assert_flat("tn", rss);
}

/// Loops whose iterations allocate far more than a release threshold, so
/// their regions are released many times while they carry texts, lists and
/// records, through `continue`, `break`, `return`, nested loops and a
/// carried list that outgrows the threshold.
const REGIONS: &str = r#"package tests.regions

type Acc {
  words: List<Text>
  count: Int
}

const FILLER: Text = "................................................................"

fn show(text: Text) -> Result<Unit, IoError> requires [stdout.write] {
  var bytes = text.to_bytes()
  bytes.push(10)
  return Stdout.write_all(bytes)
}

fn noise(i: Int) -> Text {
  return FILLER.concat(Int.to_text(i))
}

fn milestones(limit: Int) -> List<Text> {
  var names: List<Text> = []
  var marks = ""
  var i = 0
  while i < limit {
    let junk = noise(i)
    i = i + 1
    if i % 1000 == 0 {
      names.push(Int.to_text(i))
      marks = marks.concat("x")
      continue
    }
  }
  names.push(marks)
  return names
}

fn nested() -> Acc {
  var acc = Acc { words = [], count = 0 }
  for w in ["a", "b", "c"] {
    var j = 0
    while j < 3000 {
      let junk = w.concat(noise(j))
      j = j + 1
      if j == 3000 {
        acc.words.push(w.concat(Int.to_text(j)))
      }
      acc.count = acc.count + 1
    }
  }
  return acc
}

fn last_of(limit: Int) -> Text {
  var last = ""
  var tag = ""
  var k = 0
  loop {
    last = noise(k)
    tag = Int.to_text(k)
    k = k + 1
    if k == limit {
      break
    }
  }
  return tag.concat(" ").concat(Int.to_text(last.len()))
}

fn found(limit: Int) -> Text {
  var best = "none"
  var i = 0
  loop {
    best = "value ".concat(Int.to_text(i))
    let junk = noise(i)
    i = i + 1
    if i == limit {
      return best
    }
  }
}

fn big(limit: Int) -> Int {
  var all: List<Text> = []
  var i = 0
  while i < limit {
    all.push(noise(i))
    i = i + 1
  }
  var total = 0
  for t in all {
    total = total + t.len()
  }
  return total
}

protocol Regions {
  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [stdout.write] {
    show(milestones(20000).join(","))?
    let acc = nested()
    show(acc.words.join(","))?
    show(Int.to_text(acc.count))?
    show(last_of(50000))?
    show(found(30000))?
    show(Int.to_text(big(20000)))?
    return Ok(Unit)
  }
}

system Probes {
  resource target: TcpListener { address = "127.0.0.1", port = 1 }
  component regions: Client { target = target, protocol = Regions }
}
"#;

#[test]
fn loop_regions_keep_carried_values() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("memory-regions");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("regions.telos");
    std::fs::write(&source, REGIONS).unwrap();
    let options = telos::BuildOptions {
        target: Target::Llvm,
        out: Some(dir.join("out")),
        runtime: Some(common::runtime_archive()),
        ..Default::default()
    };
    let output = telos::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let child = Command::new(&output.executables[0])
        .args(["--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn regions");
    let _connection = listener.accept().unwrap();
    let mut running = Running(child);
    let mut stdout = String::new();
    running
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let mut stderr = String::new();
    running
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let status = running.0.wait().unwrap();
    let mut milestones: Vec<String> = (1..=20).map(|n| (n * 1000).to_string()).collect();
    milestones.push("x".repeat(20));
    let expected = [
        milestones.join(","),
        "a3000,b3000,c3000".to_string(),
        "9000".to_string(),
        "49999 69".to_string(),
        "value 29999".to_string(),
        "1368890".to_string(),
    ];
    assert_eq!(stdout.lines().collect::<Vec<_>>(), expected, "{stderr}");
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
}

/// The output of a child process with a timeout per read.
struct Received {
    chunks: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl Read for Received {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.pending.is_empty() {
            match self.chunks.recv_timeout(TIMEOUT) {
                Ok(chunk) => self.pending = chunk,
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "no output from tn",
                    ));
                }
            }
        }
        let count = self.pending.len().min(buffer.len());
        buffer[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        Ok(count)
    }
}

/// A loop that pushes onto a carried list (`docs/llvm.md` section 4: a due
/// release happens only at a doubling of the list, so the copies stay
/// linear in the pushes). The program reads bytes from the test until end
/// of stream and pushes one `Text` per byte.
const GROW: &str = r#"package tests.grow

fn grow(count: Int) -> Int {
  var all: List<Text> = []
  var i = 0
  while i < count {
    all.push(Int.to_text(i))
    i = i + 1
  }
  return all.len()
}

protocol Grow {
  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [stream.read, stdout.write] {
    var count = 0
    var open = true
    while open {
      match stream.read(65536)? {
        Eof => { open = false }
        Data(bytes) => { count = count + bytes.len() }
      }
    }
    var line = Int.to_text(grow(count)).to_bytes()
    line.push(10)
    return Stdout.write_all(line)
  }
}

system Probes {
  resource target: TcpListener { address = "127.0.0.1", port = 1 }
  component grow: Client { target = target, protocol = Grow }
}
"#;

/// Seconds from the end of the input to the exit of `program` pushing
/// `count` items.
fn time_pushes(program: &Path, count: usize) -> f64 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let child = Command::new(program)
        .args(["--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn grow");
    let mut running = Running(child);
    let (mut connection, _) = listener.accept().unwrap();
    connection.write_all(&vec![b'x'; count]).unwrap();
    let start = std::time::Instant::now();
    connection.shutdown(std::net::Shutdown::Write).unwrap();
    let mut stdout = String::new();
    running
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let status = running.0.wait().unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    let mut stderr = String::new();
    running
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(stdout.trim(), count.to_string(), "{stderr}");
    assert_eq!(status.code(), Some(0), "stderr: {stderr}");
    elapsed
}

#[test]
fn pushing_in_a_loop_is_linear() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("memory-grow");
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("grow.telos");
    std::fs::write(&source, GROW).unwrap();
    let options = telos::BuildOptions {
        target: Target::Llvm,
        out: Some(dir.join("out")),
        runtime: Some(common::runtime_archive()),
        ..Default::default()
    };
    let output = telos::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
    let program = &output.executables[0];
    let small = time_pushes(program, 16_384);
    let large = time_pushes(program, 65_536);
    eprintln!("16384 pushes: {small:.3} s, 65536 pushes: {large:.3} s");
    assert!(
        large < 8.0 * small + 0.25,
        "4 times the pushes took {:.1} times as long ({small:.3} s, {large:.3} s)",
        large / small
    );
    assert!(large < 2.0, "65536 pushes took {large:.3} s");
}
