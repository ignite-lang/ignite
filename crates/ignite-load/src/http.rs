//! The HTTP/1.1 client of the todo workloads and the todo check: one keep-alive connection,
//! a request written whole, a response read by its `Content-Length` (every server measured
//! sends one; a response without it has an empty body).

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    /// Names in lower case, values trimmed, in the order sent.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// The first response in `buffer` and the bytes it took; `Ok(None)` while it is incomplete,
/// `Err` when the bytes are no HTTP/1.1 response.
pub fn parse_response(buffer: &[u8]) -> Result<Option<(Response, usize)>, String> {
    let Some(head_end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") else {
        if buffer.len() >= 8 && !buffer.starts_with(b"HTTP/1.") {
            return Err("not an HTTP/1.1 response".to_string());
        }
        return Ok(None);
    };
    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| "a head that is not UTF-8")?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .strip_prefix("HTTP/1.1 ")
        .or_else(|| status_line.strip_prefix("HTTP/1.0 "))
        .and_then(|rest| rest.get(..3))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("bad status line {status_line:?}"))?;
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("bad header line {line:?}"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    let length = match headers.iter().find(|(n, _)| n == "content-length") {
        Some((_, value)) => value
            .parse::<usize>()
            .map_err(|_| format!("bad content-length {value:?}"))?,
        None => 0,
    };
    let start = head_end + 4;
    if buffer.len() < start + length {
        return Ok(None);
    }
    Ok(Some((
        Response {
            status,
            headers,
            body: buffer[start..start + length].to_vec(),
        },
        start + length,
    )))
}

/// A request as the workloads and the check send it.
pub fn request(method: &str, path: &str, body: &str) -> Vec<u8> {
    let mut out = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    if !body.is_empty() || matches!(method, "POST" | "PATCH" | "PUT") {
        out.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    out.push_str("\r\n");
    out.push_str(body);
    out.into_bytes()
}

/// One keep-alive connection.
pub struct Client {
    pub socket: TcpStream,
    buffer: Vec<u8>,
    raw: Vec<u8>,
}

impl Client {
    pub fn open(address: SocketAddr) -> std::io::Result<Client> {
        let socket = TcpStream::connect(address)?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_nodelay(true)?;
        Ok(Client {
            socket,
            buffer: Vec::with_capacity(16 * 1024),
            raw: vec![0; 16 * 1024],
        })
    }

    /// Send `request` and read its response.
    pub fn exchange(&mut self, request: &[u8]) -> Result<Response, String> {
        self.socket
            .write_all(request)
            .map_err(|e| format!("write: {e}"))?;
        self.read()
    }

    /// Read the next response.
    pub fn read(&mut self) -> Result<Response, String> {
        loop {
            if let Some((response, used)) = parse_response(&self.buffer)? {
                self.buffer.drain(..used);
                crate::quickack(&self.socket);
                return Ok(response);
            }
            let count = loop {
                match self.socket.read(&mut self.raw) {
                    Ok(count) => break count,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(format!("read: {e}")),
                }
            };
            if count == 0 {
                return Err("closed by the server".to_string());
            }
            self.buffer.extend_from_slice(&self.raw[..count]);
        }
    }
}
