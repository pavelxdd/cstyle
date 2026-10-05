//! Hash maps and sets keyed by token, line or group indices.
//!
//! Such keys come from the formatter itself, so they need no protection
//! against crafted collisions, and the standard SipHash costs more than the
//! lookups it serves.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

pub(crate) type IndexMap<K, V> = HashMap<K, V, BuildHasherDefault<IndexHasher>>;
pub(crate) type IndexSet<K> = HashSet<K, BuildHasherDefault<IndexHasher>>;

/// A multiplicative hash of the integers written to it.
#[derive(Default)]
pub(crate) struct IndexHasher(u64);

impl IndexHasher {
    fn add(&mut self, value: u64) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(SEED);
    }
}

impl Hasher for IndexHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.add(u64::from(byte));
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}
