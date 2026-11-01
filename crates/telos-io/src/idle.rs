//! `TcpStream.set_idle_timeout` (`docs/slice.md` section 11.5): a read
//! that waits longer than the timeout ends as if the peer had closed
//! (`Input::Eof`; `TcpStream::read_into` maps the socket's timeout error to
//! 0 bytes). The timeout belongs to the socket, so every clone of the
//! stream shares it.

use std::time::Duration;

use crate::{IoError, TcpStream};

impl TcpStream {
    /// Set the idle timeout to `ms` milliseconds; 0 or less removes it.
    pub fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        let timeout = u64::try_from(ms)
            .ok()
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis);
        self.socket()
            .set_read_timeout(timeout)
            .map_err(|e| IoError::new(format!("set idle timeout: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{Input, TcpListener, TcpStream};

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let client = TcpStream::connect("127.0.0.1".to_string(), listener.local_port()).unwrap();
        (listener.accept().unwrap(), client)
    }

    #[test]
    fn an_idle_read_ends_with_eof() {
        let (server, _client) = pair();
        server.set_idle_timeout(100).unwrap();
        let started = Instant::now();
        assert_eq!(server.read(16).unwrap(), Input::Eof);
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(90), "{waited:?}");
        assert!(waited < Duration::from_secs(5), "{waited:?}");
    }

    #[test]
    fn data_before_the_timeout_is_read() {
        let (server, client) = pair();
        server.set_idle_timeout(2000).unwrap();
        client.write_all(b"hi".to_vec()).unwrap();
        crate::flush();
        assert_eq!(server.read(16).unwrap(), Input::Data(b"hi".to_vec()));
    }

    #[test]
    fn zero_removes_the_timeout() {
        let (server, _client) = pair();
        server.set_idle_timeout(50).unwrap();
        server.set_idle_timeout(0).unwrap();
        assert_eq!(server.socket().read_timeout().unwrap(), None);
    }
}
