//! Stable FNV-1a 64 hash for on-disk cache file names.
//!
//! `std::collections::hash_map::DefaultHasher` is unspecified and must not
//! back persistent file names. This hasher is byte-stable across processes
//! and Rust versions.

/// FNV-1a 64 over raw bytes.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Incremental stable hasher (FNV-1a 64).
#[derive(Debug, Clone)]
pub struct StableHasher(u64);

impl StableHasher {
    pub fn new() -> Self {
        Self(14695981039346656037)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        const PRIME: u64 = 1099511628211;
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    pub fn write_u64_le(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    pub fn write_u128_le(&mut self, value: u128) {
        self.write(&value.to_le_bytes());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

impl Default for StableHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hash_is_offset_basis() {
        assert_eq!(fnv1a64(&[]), 14695981039346656037);
    }

    #[test]
    fn known_vector_is_stable() {
        // FNV-1a 64 of "foobar" (well-known test vector).
        assert_eq!(fnv1a64(b"foobar"), 0x85944171_f73967e8);
    }

    #[test]
    fn incremental_matches_oneshot() {
        let mut hasher = StableHasher::new();
        hasher.write(b"foo");
        hasher.write(b"bar");
        assert_eq!(hasher.finish(), fnv1a64(b"foobar"));
    }
}
