//! Runtime-sized bitset — a port of `cpp/support/dynamic_bitset.h`.
//!
//! The backing buffer is a `u32` word array, 32 bits per word. This is the owned
//! ("internal buffer") form used by the compiler's token-mask storage. The matcher's
//! external-buffer form — writing bits directly into a caller-provided DLTensor — is
//! added as a borrowed view alongside the matcher (M6); the bit-scan logic here is the
//! shared source of truth.

use serde::{Deserialize, Serialize};

/// Bits packed per backing word.
pub const BITS_PER_BLOCK: usize = 32;

/// A bitset whose length is fixed at construction time.
///
/// Bits `0..len` are addressable; the final word may contain unused high padding bits
/// (e.g. after [`DynamicBitset::set_all`]). Scans that could return a padding index are
/// bounded by `len`, matching the C++ behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "DynamicBitsetRepr")]
pub struct DynamicBitset {
    size: usize,
    data: Vec<u32>,
}

#[derive(Deserialize)]
struct DynamicBitsetRepr {
    size: usize,
    data: Vec<u32>,
}

impl TryFrom<DynamicBitsetRepr> for DynamicBitset {
    type Error = String;

    fn try_from(repr: DynamicBitsetRepr) -> Result<Self, Self::Error> {
        let expected = Self::buffer_size(repr.size);
        if repr.data.len() != expected {
            return Err(format!(
                "dynamic bitset buffer length {} does not match ceil(size/32) = {expected}",
                repr.data.len()
            ));
        }
        Ok(Self {
            size: repr.size,
            data: repr.data,
        })
    }
}

impl DynamicBitset {
    /// Minimal `u32` buffer length needed to hold `element_size` bits.
    #[must_use]
    pub const fn buffer_size(element_size: usize) -> usize {
        element_size.div_ceil(BITS_PER_BLOCK)
    }

    /// Creates a bitset of `size` bits, all cleared.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            size,
            data: vec![0; Self::buffer_size(size)],
        }
    }

    /// Number of addressable bits.
    #[must_use]
    pub fn len(&self) -> usize {
        self.size
    }

    /// Whether the bitset holds zero bits.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// The raw backing words.
    #[must_use]
    pub fn as_words(&self) -> &[u32] {
        &self.data
    }

    /// Returns the bit at `index`.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    #[must_use]
    pub fn get(&self, index: usize) -> bool {
        assert!(index < self.size, "bit index out of bounds");
        (self.data[index / BITS_PER_BLOCK] >> (index % BITS_PER_BLOCK)) & 1 == 1
    }

    /// Sets every bit (including padding bits in the final word) to `true`.
    pub fn set_all(&mut self) {
        self.data.fill(u32::MAX);
    }

    /// Clears every bit.
    pub fn reset_all(&mut self) {
        self.data.fill(0);
    }

    /// Sets the bit at `index` to `value`.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    pub fn set(&mut self, index: usize, value: bool) {
        assert!(index < self.size, "bit index out of bounds");
        let word = &mut self.data[index / BITS_PER_BLOCK];
        let mask = 1u32 << (index % BITS_PER_BLOCK);
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
    }

    /// Clears the bit at `index`.
    ///
    /// # Panics
    /// Panics if `index >= len`.
    pub fn reset(&mut self, index: usize) {
        self.set(index, false);
    }

    /// Bitwise-ORs `other` into `self`.
    ///
    /// # Panics
    /// Panics if `self`'s buffer is larger than `other`'s (mirrors the C++ contract that
    /// `self` must be no larger than `other`).
    pub fn or_assign(&mut self, other: &DynamicBitset) {
        assert!(
            self.data.len() <= other.data.len(),
            "or_assign target buffer must not exceed the source buffer"
        );
        for (dst, src) in self.data.iter_mut().zip(&other.data) {
            *dst |= *src;
        }
    }

    /// Index of the first set bit, or `None`.
    #[must_use]
    pub fn find_first_one(&self) -> Option<usize> {
        self.do_find_one_from(0)
    }

    /// Index of the first set bit strictly after `pos`, or `None`.
    #[must_use]
    pub fn find_next_one(&self, pos: usize) -> Option<usize> {
        if self.size == 0 || pos >= self.size - 1 {
            return None;
        }
        let pos = pos + 1;
        let blk = pos / BITS_PER_BLOCK;
        let ind = pos % BITS_PER_BLOCK;
        let fore = self.data[blk] >> ind;
        let result = if fore != 0 {
            Some(pos + fore.trailing_zeros() as usize)
        } else {
            self.do_find_one_from(blk + 1)
        };
        result.filter(|&r| r < self.size)
    }

    /// Index of the first cleared bit, or `None`.
    #[must_use]
    pub fn find_first_zero(&self) -> Option<usize> {
        self.do_find_zero_from(0)
    }

    /// Index of the first cleared bit strictly after `pos`, or `None`.
    #[must_use]
    pub fn find_next_zero(&self, pos: usize) -> Option<usize> {
        if self.size == 0 || pos >= self.size - 1 {
            return None;
        }
        let pos = pos + 1;
        let blk = pos / BITS_PER_BLOCK;
        let ind = pos % BITS_PER_BLOCK;
        let fore = (!self.data[blk]) >> ind;
        let result = if fore != 0 {
            Some(pos + fore.trailing_zeros() as usize)
        } else {
            self.do_find_zero_from(blk + 1)
        };
        result.filter(|&r| r < self.size)
    }

    /// Number of set bits across the whole backing buffer (including any padding bits).
    #[must_use]
    pub fn count(&self) -> usize {
        self.data.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether every addressable bit is set.
    #[must_use]
    pub fn all(&self) -> bool {
        if self.size == 0 {
            return true;
        }
        let last = self.data.len() - 1;
        if self.data[..last].iter().any(|&w| w != u32::MAX) {
            return false;
        }
        let remaining = self.size % BITS_PER_BLOCK;
        let last_mask = if remaining == 0 {
            u32::MAX
        } else {
            (1u32 << remaining) - 1
        };
        (self.data[last] & last_mask) == last_mask
    }

    fn do_find_one_from(&self, first_block: usize) -> Option<usize> {
        let pos = self.data[first_block..].iter().position(|&w| w != 0)? + first_block;
        Some(pos * BITS_PER_BLOCK + self.data[pos].trailing_zeros() as usize)
    }

    fn do_find_zero_from(&self, first_block: usize) -> Option<usize> {
        let pos = self.data[first_block..]
            .iter()
            .position(|&w| w != u32::MAX)?
            + first_block;
        Some(pos * BITS_PER_BLOCK + (!self.data[pos]).trailing_zeros() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_size_is_ceil_div_32() {
        assert_eq!(DynamicBitset::buffer_size(0), 0);
        assert_eq!(DynamicBitset::buffer_size(1), 1);
        assert_eq!(DynamicBitset::buffer_size(32), 1);
        assert_eq!(DynamicBitset::buffer_size(33), 2);
        assert_eq!(DynamicBitset::buffer_size(64), 2);
        assert_eq!(DynamicBitset::buffer_size(65), 3);
    }

    #[test]
    fn set_get_reset() {
        let mut b = DynamicBitset::new(70);
        assert_eq!(b.len(), 70);
        assert!(!b.get(5));
        b.set(5, true);
        b.set(33, true);
        b.set(69, true);
        assert!(b.get(5) && b.get(33) && b.get(69));
        b.reset(33);
        assert!(!b.get(33));
        assert_eq!(b.count(), 2);
    }

    #[test]
    fn set_all_reset_all_and_all() {
        let mut b = DynamicBitset::new(40);
        assert!(!b.all());
        b.set_all();
        assert!(b.all()); // padding bits ignored by `all`
        b.reset_all();
        assert!(!b.all());
        assert!(DynamicBitset::new(0).all());
    }

    #[test]
    fn find_first_and_next_one() {
        let mut b = DynamicBitset::new(100);
        assert_eq!(b.find_first_one(), None);
        for i in [3usize, 31, 32, 64, 99] {
            b.set(i, true);
        }
        assert_eq!(b.find_first_one(), Some(3));
        assert_eq!(b.find_next_one(3), Some(31));
        assert_eq!(b.find_next_one(31), Some(32));
        assert_eq!(b.find_next_one(32), Some(64));
        assert_eq!(b.find_next_one(64), Some(99));
        assert_eq!(b.find_next_one(99), None);
    }

    #[test]
    fn find_first_and_next_zero() {
        let mut b = DynamicBitset::new(100);
        b.set_all();
        // padding beyond 100 is set too, but scans are bounded by len.
        for i in [0usize, 31, 32, 70] {
            b.set(i, false);
        }
        assert_eq!(b.find_first_zero(), Some(0));
        assert_eq!(b.find_next_zero(0), Some(31));
        assert_eq!(b.find_next_zero(31), Some(32));
        assert_eq!(b.find_next_zero(32), Some(70));
        assert_eq!(b.find_next_zero(70), None);
    }

    #[test]
    fn all_set_has_no_zero() {
        let mut b = DynamicBitset::new(64);
        b.set_all();
        assert_eq!(b.find_first_zero(), None);
        let mut full = DynamicBitset::new(50);
        full.set_all();
        assert_eq!(full.find_first_zero(), None);
    }

    #[test]
    fn or_assign_unions_bits() {
        let mut a = DynamicBitset::new(64);
        let mut b = DynamicBitset::new(64);
        a.set(1, true);
        a.set(40, true);
        b.set(2, true);
        b.set(40, true);
        a.or_assign(&b);
        assert!(a.get(1) && a.get(2) && a.get(40));
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn serde_roundtrip_and_validation() {
        let mut b = DynamicBitset::new(40);
        b.set(7, true);
        b.set(39, true);
        let json = serde_json::to_string(&b).unwrap();
        let back: DynamicBitset = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);

        // mismatched buffer length is rejected
        let bad = r#"{"size":40,"data":[0]}"#;
        assert!(serde_json::from_str::<DynamicBitset>(bad).is_err());
    }
}
