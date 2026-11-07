//! Maps keyed by the [`NodeId`] of an AST node (`ignite_syntax::ast`).
//!
//! The ids are the parser's, so a clone of a `Program` answers the same
//! lookups and a typed module does not depend on where its AST lives.
//! [`NodeId`] hashes as one `u64` (file in the high half, index in the
//! low); the hasher mixes both halves so the bucket index and the
//! hash-table fingerprint each see every bit.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use ignite_syntax::ast::NodeId;

pub(crate) type NodeMap<V> = HashMap<NodeId, V, BuildHasherDefault<NodeHasher>>;

#[derive(Default)]
pub(crate) struct NodeHasher(u64);

impl Hasher for NodeHasher {
    fn finish(&self) -> u64 {
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.rotate_left(5) ^ u64::from(*byte);
        }
    }
}
