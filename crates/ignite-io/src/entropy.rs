//! Random bytes from the operating system (`docs/slice.md` 21.7): the
//! capability `Entropy`.
//!
//! One descriptor of `/dev/urandom`, opened close-on-exec (Rust's
//! `File::open` sets `O_CLOEXEC`), so no child inherits it (21.0 row 14).
//! Reads of `&File` need no lock, so the clones of one `Entropy` draw
//! concurrently.

use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use crate::IoError;

const SOURCE: &str = "/dev/urandom";
const MAXIMUM_COUNT: i64 = 65536;

/// A source of random bytes. A cheap clone; several tasks may draw at once.
#[derive(Clone, Debug)]
pub struct Entropy {
    source: Arc<Result<File, String>>,
}

impl Default for Entropy {
    fn default() -> Entropy {
        Entropy::new()
    }
}

impl Entropy {
    /// `Entropy.new()`, test-only. A failure to open the source is
    /// reported by the first draw.
    pub fn new() -> Entropy {
        Entropy {
            source: Arc::new(File::open(SOURCE).map_err(|e| format!("{SOURCE}: {e}"))),
        }
    }

    /// The resource `Entropy` at launch: fails when the source cannot be
    /// opened, before the program binds a port.
    pub fn open() -> Result<Entropy, IoError> {
        let entropy = Entropy::new();
        match entropy.source.as_ref() {
            Ok(_) => Ok(entropy),
            Err(message) => Err(IoError::new(message.clone())),
        }
    }

    /// `count` random bytes, `count` in `0..=65536`.
    pub fn random_bytes(&self, count: i64) -> Result<Vec<u8>, IoError> {
        if !(0..=MAXIMUM_COUNT).contains(&count) {
            return Err(IoError::new(format!(
                "random_bytes: count must be in 0..={MAXIMUM_COUNT}"
            )));
        }
        let mut file = match self.source.as_ref() {
            Ok(file) => file,
            Err(message) => return Err(IoError::new(message.clone())),
        };
        let mut data = vec![0; count as usize];
        file.read_exact(&mut data)
            .map_err(|e| IoError::new(format!("{SOURCE}: {e}")))?;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_have_the_count_and_differ() {
        let entropy = Entropy::open().unwrap();
        let first = entropy.random_bytes(32).unwrap();
        let second = entropy.clone().random_bytes(32).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        assert_ne!(first, second);
        assert!(entropy.random_bytes(0).unwrap().is_empty());
        assert_eq!(entropy.random_bytes(65536).unwrap().len(), 65536);
    }

    #[test]
    fn counts_outside_the_bound_are_refused() {
        let entropy = Entropy::new();
        for count in [-1, 65537, i64::MIN, i64::MAX] {
            assert_eq!(
                entropy.random_bytes(count).unwrap_err().message,
                "random_bytes: count must be in 0..=65536"
            );
        }
    }

    #[test]
    fn the_source_is_close_on_exec() {
        let entropy = Entropy::open().unwrap();
        let file = entropy.source.as_ref().as_ref().unwrap();
        let flags = nix::fcntl::fcntl(file, nix::fcntl::FcntlArg::F_GETFD).unwrap();
        assert_ne!(flags & nix::fcntl::FdFlag::FD_CLOEXEC.bits(), 0);
    }
}
