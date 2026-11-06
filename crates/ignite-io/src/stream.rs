//! The stream interface `ByteStream` (`docs/slice.md` 18.3, 18.8): what a
//! fn generic over its stream calls. The Rust target emits a bounded type
//! parameter as `S: ByteStream` and calls `<S as ByteStream>::read(..)`,
//! so each instance dispatches statically to the inherent method of its
//! stream type.

use crate::{Input, IoError, PipeStream, TcpStream};

/// A byte stream: [`TcpStream`] and [`PipeStream`]. Each method is the
/// inherent method of the same name.
pub trait ByteStream: Clone + Send + Sync + 'static {
    /// One read of up to `maximum` bytes; zero bytes is `Input::Eof`.
    fn read(&self, maximum: i64) -> Result<Input, IoError>;
    /// Append `data` to the send buffer.
    fn write_all(&self, data: Vec<u8>) -> Result<(), IoError>;
    /// Send what was written, then close the write half.
    fn close(&self) -> Result<(), IoError>;
    /// Set the read timeout in milliseconds; 0 or less removes it.
    fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError>;
    /// Send what was written and `data` at once, the last byte of `data`
    /// as urgent data.
    fn write_urgent(&self, data: Vec<u8>) -> Result<(), IoError>;
    /// Whether an urgent byte is ahead of the reader.
    fn urgent_pending(&self) -> Result<bool, IoError>;
}

impl ByteStream for TcpStream {
    fn read(&self, maximum: i64) -> Result<Input, IoError> {
        TcpStream::read(self, maximum)
    }

    fn write_all(&self, data: Vec<u8>) -> Result<(), IoError> {
        TcpStream::write_all(self, data)
    }

    fn close(&self) -> Result<(), IoError> {
        TcpStream::close(self)
    }

    fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        TcpStream::set_idle_timeout(self, ms)
    }

    fn write_urgent(&self, data: Vec<u8>) -> Result<(), IoError> {
        TcpStream::write_urgent(self, data)
    }

    fn urgent_pending(&self) -> Result<bool, IoError> {
        TcpStream::urgent_pending(self)
    }
}

impl ByteStream for PipeStream {
    fn read(&self, maximum: i64) -> Result<Input, IoError> {
        PipeStream::read(self, maximum)
    }

    fn write_all(&self, data: Vec<u8>) -> Result<(), IoError> {
        PipeStream::write_all(self, data)
    }

    fn close(&self) -> Result<(), IoError> {
        PipeStream::close(self)
    }

    fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        PipeStream::set_idle_timeout(self, ms)
    }

    fn write_urgent(&self, data: Vec<u8>) -> Result<(), IoError> {
        PipeStream::write_urgent(self, data)
    }

    fn urgent_pending(&self) -> Result<bool, IoError> {
        PipeStream::urgent_pending(self)
    }
}
