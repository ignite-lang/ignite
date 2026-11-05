//! Random bytes from the operating system (`docs/slice.md` 21.7): the
//! capability `Entropy`.
//!
//! Slice 17 U1 fixes the surface; `random_bytes` is a placeholder (21.13)
//! until U5 reads `/dev/urandom`.

use crate::IoError;

/// A source of random bytes. A cheap clone; several tasks may draw at once.
#[derive(Clone, Debug, Default)]
pub struct Entropy {
    _private: (),
}

impl Entropy {
    /// `Entropy.new()`, test-only.
    pub fn new() -> Entropy {
        Entropy::default()
    }

    /// The resource `Entropy` at launch.
    pub fn open() -> Result<Entropy, IoError> {
        Ok(Entropy::new())
    }

    /// `count` random bytes, `count` in `0..=65536`.
    pub fn random_bytes(&self, count: i64) -> Result<Vec<u8>, IoError> {
        let _ = count;
        unimplemented!("slice 17: Entropy.random_bytes")
    }
}
