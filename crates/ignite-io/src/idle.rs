//! `TcpStream.set_idle_timeout` (`docs/slice.md` section 11.5): a read
//! that waits longer than the timeout ends as if the peer had closed
//! (`Input::Eof`; `TcpStream::read_into` returns 0 bytes). On a
//! non-blocking socket (IO mode `readiness`) the timeout is the deadline of
//! the read's wait (a timer on a worker, `poll`'s timeout on a thread); on
//! a blocking one (mode `threads`) it is `SO_RCVTIMEO`. It belongs to the
//! stream, so every clone shares it.

use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::{IoError, TcpStream};

impl TcpStream {
    /// Set the idle timeout to `ms` milliseconds; 0 or less removes it.
    pub fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        let (idle, blocking) = self.idle_ms();
        idle.store(ms.max(0), Ordering::Release);
        let Some(socket) = blocking else {
            return Ok(());
        };
        let timeout = u64::try_from(ms)
            .ok()
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis);
        socket
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
        assert_eq!(
            server
                .idle_ms()
                .0
                .load(std::sync::atomic::Ordering::Acquire),
            0
        );
        let started = Instant::now();
        let reader = server.clone();
        let read = std::thread::spawn(move || reader.read(16));
        std::thread::sleep(Duration::from_millis(150));
        assert!(!read.is_finished(), "a read without timeout waits on");
        drop(_client);
        assert_eq!(read.join().unwrap().unwrap(), Input::Eof);
        assert!(started.elapsed() >= Duration::from_millis(150));
    }
}
