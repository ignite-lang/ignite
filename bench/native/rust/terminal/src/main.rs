//! terminald of `examples/terminal/terminal.ignite` in plain Rust: the standard library only,
//! one OS thread per running session with blocking reads, finished threads reused for the next
//! connection ([`workers`]), as the Ignite program runs; a reference
//! program for `docs/quality.md` row 3.3.
//!
//! The same bytes on the wire as the Ignite program for every input the benchmark and its
//! functional check send: the prompt `terminal> ` followed by IAC GA, the console's commands
//! (`help`, `echo <text>`, `status`, `quit`, a note for an unknown one, nothing for a blank
//! line), each reply and the next prompt in one write; CR LF, CR NUL, a lone CR and a lone LF
//! end a line; DO is refused with WONT and WILL with DONT; AYT is answered `[yes]` and IAC GA;
//! IP, a line over 4096 bytes and a subnegotiation over 1024 bytes end the session; a client
//! silent for 10 minutes is dropped. The Synch as the Ignite server honours it: `SO_OOBINLINE`
//! on every stream, one `poll` for `POLLPRI` after every read, and urgent data pending starts
//! discarding the line and the data, commands still obeyed, up to IAC DM; so both servers make
//! the same system calls per read.
//! `--port P` (default 2323) on 127.0.0.1; `listening on 127.0.0.1:P` on stderr once bound.

mod workers;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::time::Duration;

const PROMPT: &[u8] = b"terminal> \xff\xf9";
const MAXIMUM_LINE: usize = 4096;
const MAXIMUM_SUBNEGOTIATION: usize = 1024;
const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const GA: u8 = 249;
const AYT: u8 = 246;
const IP: u8 = 244;
const DM: u8 = 242;
const SE: u8 = 240;

const SOL_SOCKET: i32 = 1;
const SO_OOBINLINE: i32 = 10;
const POLLPRI: i16 = 0x002;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn setsockopt(fd: i32, level: i32, name: i32, value: *const i32, length: u32) -> i32;
    fn poll(fds: *mut PollFd, count: u64, timeout: i32) -> i32;
}

/// Keep the urgent byte in the stream at its place, as the Ignite substrate does on every stream.
fn set_oob_inline(stream: &TcpStream) -> std::io::Result<()> {
    let on: i32 = 1;
    // SAFETY: the descriptor belongs to `stream`, and `on` outlives the call with the size given.
    let result = unsafe {
        setsockopt(
            stream.as_raw_fd(),
            SOL_SOCKET,
            SO_OOBINLINE,
            &on,
            std::mem::size_of::<i32>() as u32,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Whether urgent data is ahead of the reader: one `poll` for `POLLPRI` with no wait, the check
/// the Ignite substrate's `urgent_pending` makes after every read.
fn urgent_pending(stream: &TcpStream) -> std::io::Result<bool> {
    let mut fds = PollFd {
        fd: stream.as_raw_fd(),
        events: POLLPRI,
        revents: 0,
    };
    // SAFETY: `fds` is one valid entry for the length of the call.
    if unsafe { poll(&mut fds, 1, 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fds.revents & POLLPRI != 0)
}

#[derive(Clone, Copy)]
enum Decoder {
    Ground,
    CarriageReturn,
    Interpret,
    Negotiation(u8),
    Subnegotiation(usize),
    SubnegotiationInterpret(usize),
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// The console's reply lines to one input line, and whether it closes the session.
fn respond(line: &[u8]) -> (Vec<Vec<u8>>, bool) {
    let start = line
        .iter()
        .position(|b| !is_space(*b))
        .unwrap_or(line.len());
    let end = line
        .iter()
        .rposition(|b| !is_space(*b))
        .map_or(start, |i| i + 1);
    let trimmed = &line[start..end];
    if trimmed.is_empty() {
        return (Vec::new(), false);
    }
    let word_end = trimmed
        .iter()
        .position(|b| is_space(*b))
        .unwrap_or(trimmed.len());
    let word = &trimmed[..word_end];
    match word {
        b"help" => (
            vec![
                b"help".to_vec(),
                b"echo <text>".to_vec(),
                b"status".to_vec(),
                b"quit".to_vec(),
            ],
            false,
        ),
        b"status" => (vec![b"ready".to_vec()], false),
        b"quit" => (vec![b"bye".to_vec()], true),
        b"echo" => {
            let rest = (word_end + 1).min(trimmed.len());
            (vec![trimmed[rest..].to_vec()], false)
        }
        _ => {
            let mut note = b"unknown command: ".to_vec();
            note.extend_from_slice(word);
            (vec![note], false)
        }
    }
}

/// A reply line as NVT data: LF as CR LF, a lone CR as CR NUL, IAC doubled; then CR LF.
fn render(lines: &[Vec<u8>], out: &mut Vec<u8>) {
    for line in lines {
        let mut i = 0;
        while i < line.len() {
            let b = line[i];
            i += 1;
            match b {
                b'\n' => out.extend_from_slice(b"\r\n"),
                b'\r' => {
                    if i >= line.len() || line[i] != b'\n' {
                        out.extend_from_slice(b"\r\0");
                    }
                }
                IAC => out.extend_from_slice(&[IAC, IAC]),
                _ => out.push(b),
            }
        }
        out.extend_from_slice(b"\r\n");
    }
}

fn session(mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_nodelay(true)?;
    set_oob_inline(&stream)?;
    stream.set_read_timeout(Some(Duration::from_secs(600)))?;
    stream.write_all(PROMPT)?;
    let mut buffer = [0u8; 4096];
    let mut state = Decoder::Ground;
    let mut discarding = false;
    let mut line = Vec::with_capacity(256);
    let mut out = Vec::with_capacity(4096);
    loop {
        let count = match stream.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if !discarding && urgent_pending(&stream)? {
            discarding = true;
            line.clear();
        }
        for &b in &buffer[..count] {
            let (next, literal, end_of_line) = match (state, b) {
                (Decoder::Ground, IAC) => (Decoder::Interpret, None, false),
                (Decoder::Ground, b'\r') => (Decoder::CarriageReturn, None, true),
                (Decoder::Ground, b'\n') => (Decoder::Ground, None, true),
                (Decoder::Ground, _) => (Decoder::Ground, Some(b), false),
                (Decoder::CarriageReturn, b'\n' | 0) => (Decoder::Ground, None, false),
                (Decoder::CarriageReturn, IAC) => (Decoder::Interpret, None, false),
                (Decoder::CarriageReturn, b'\r') => (Decoder::CarriageReturn, None, true),
                (Decoder::CarriageReturn, _) => (Decoder::Ground, Some(b), false),
                (Decoder::Interpret, IAC) => (Decoder::Ground, Some(IAC), false),
                (Decoder::Interpret, WILL..=DONT) => (Decoder::Negotiation(b), None, false),
                (Decoder::Interpret, SB) => (Decoder::Subnegotiation(0), None, false),
                (Decoder::Interpret, AYT) => {
                    stream.write_all(&[b'[', b'y', b'e', b's', b']', b'\r', b'\n', IAC, GA])?;
                    (Decoder::Ground, None, false)
                }
                (Decoder::Interpret, IP) => return Ok(()),
                (Decoder::Interpret, DM) => {
                    discarding = false;
                    (Decoder::Ground, None, false)
                }
                (Decoder::Interpret, _) => (Decoder::Ground, None, false),
                (Decoder::Negotiation(verb), _) => {
                    if verb == DO {
                        stream.write_all(&[IAC, WONT, b])?;
                    } else if verb == WILL {
                        stream.write_all(&[IAC, DONT, b])?;
                    }
                    (Decoder::Ground, None, false)
                }
                (Decoder::Subnegotiation(n), _) => {
                    if n + 1 > MAXIMUM_SUBNEGOTIATION {
                        return Ok(());
                    } else if b == IAC {
                        (Decoder::SubnegotiationInterpret(n + 1), None, false)
                    } else {
                        (Decoder::Subnegotiation(n + 1), None, false)
                    }
                }
                (Decoder::SubnegotiationInterpret(_), SE) => (Decoder::Ground, None, false),
                (Decoder::SubnegotiationInterpret(n), _) => {
                    (Decoder::Subnegotiation(n), None, false)
                }
            };
            state = next;
            if discarding {
                continue;
            }
            if let Some(c) = literal {
                line.push(c);
                if line.len() > MAXIMUM_LINE {
                    stream.write_all(b"line too long\r\n")?;
                    return Ok(());
                }
            }
            if end_of_line {
                let (reply, close) = respond(&line);
                line.clear();
                out.clear();
                render(&reply, &mut out);
                if close {
                    stream.write_all(&out)?;
                    return Ok(());
                }
                out.extend_from_slice(PROMPT);
                stream.write_all(&out)?;
            }
        }
    }
}

fn exit(message: &str) -> ! {
    eprintln!("terminal: {message}");
    std::process::exit(2)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut port: u16 = 2323;
    while let Some(arg) = args.next() {
        if arg == "--port" {
            port = args
                .next()
                .and_then(|p| p.parse().ok())
                .unwrap_or_else(|| exit("--port needs a port number"));
        } else {
            exit(&format!("unknown argument {arg}"));
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", port))
        .unwrap_or_else(|e| exit(&format!("bind 127.0.0.1:{port}: {e}")));
    let bound = listener.local_addr().map_or(port, |a| a.port());
    eprintln!("listening on 127.0.0.1:{bound}");
    let workers = workers::Workers::new();
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if workers
            .spawn(move || {
                if let Err(error) = session(stream) {
                    eprintln!("session ended with error: {error}");
                }
            })
            .is_err()
        {
            eprintln!("session ended with error: no thread");
        }
    }
}
