//! Maps indexed exclusively by addresses of nodes in the live AST.
//!
//! These keys are compiler-owned addresses, rather than source identifiers.
//! Mix both low and high bits so aligned addresses also distribute the
//! bucket indices and hash-table fingerprints.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub(crate) type AddressMap<V> = HashMap<usize, V, BuildHasherDefault<AddressHasher>>;

#[derive(Default)]
pub(crate) struct AddressHasher(u64);

impl Hasher for AddressHasher {
    fn finish(&self) -> u64 {
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn write_usize(&mut self, value: usize) {
        self.0 = value as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.rotate_left(5) ^ u64::from(*byte);
        }
    }
}
