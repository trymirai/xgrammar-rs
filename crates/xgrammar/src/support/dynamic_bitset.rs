//! Runtime-sized bitset over a `u32` word array — a port of `cpp/support/dynamic_bitset.h`.
//!
//! This is the owned form; the matcher's external-buffer (DLTensor) view lands in M6.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
    pub fn get(
        &self,
        index: usize,
    ) -> bool {
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
    pub fn set(
        &mut self,
        index: usize,
        value: bool,
    ) {
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
    pub fn reset(
        &mut self,
        index: usize,
    ) {
        self.set(index, false);
    }

    /// Bitwise-ORs `other` into `self`.
    ///
    /// # Panics
    /// Panics if `self`'s buffer is larger than `other`'s (mirrors the C++ contract that
    /// `self` must be no larger than `other`).
    pub fn or_assign(
        &mut self,
        other: &DynamicBitset,
    ) {
        assert!(self.data.len() <= other.data.len(), "or_assign target buffer must not exceed the source buffer");
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
    pub fn find_next_one(
        &self,
        pos: usize,
    ) -> Option<usize> {
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
    pub fn find_next_zero(
        &self,
        pos: usize,
    ) -> Option<usize> {
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

    fn do_find_one_from(
        &self,
        first_block: usize,
    ) -> Option<usize> {
        let pos = self.data[first_block..].iter().position(|&w| w != 0)? + first_block;
        Some(pos * BITS_PER_BLOCK + self.data[pos].trailing_zeros() as usize)
    }

    fn do_find_zero_from(
        &self,
        first_block: usize,
    ) -> Option<usize> {
        let pos = self.data[first_block..].iter().position(|&w| w != u32::MAX)? + first_block;
        Some(pos * BITS_PER_BLOCK + (!self.data[pos]).trailing_zeros() as usize)
    }

    /// Serializes in the C++ JSON format: `[size, buffer_size, ...u32 words]`.
    #[must_use]
    pub fn serialize_json_value_cpp(&self) -> Value {
        let mut arr = vec![json!(self.size), json!(Self::buffer_size(self.size))];
        for &word in &self.data {
            arr.push(json!(word));
        }
        Value::Array(arr)
    }

    /// Deserializes the C++ JSON array format.
    ///
    /// # Errors
    /// Returns an error string when the JSON shape is invalid.
    pub fn deserialize_json_value_cpp(value: &Value) -> Result<Self, String> {
        let arr = value.as_array().ok_or_else(|| "dynamic bitset expects a JSON array".to_owned())?;
        if arr.len() < 2 {
            return Err("dynamic bitset array must have at least 2 elements".to_owned());
        }
        let size = arr[0].as_i64().ok_or_else(|| "dynamic bitset size must be an integer".to_owned())?;
        if size < 0 {
            return Err("dynamic bitset size must be non-negative".to_owned());
        }
        let size = size as usize;
        let buffer_size = arr[1].as_i64().ok_or_else(|| "dynamic bitset buffer_size must be an integer".to_owned())?;
        if buffer_size < 0 {
            return Err("dynamic bitset buffer_size must be non-negative".to_owned());
        }
        let buffer_size = buffer_size as usize;
        let expected = Self::buffer_size(size);
        if buffer_size != expected {
            return Err(format!("dynamic bitset buffer_size {buffer_size} does not match ceil(size/32) = {expected}"));
        }
        if arr.len() != 2 + buffer_size {
            return Err(format!(
                "dynamic bitset array length {} does not match 2 + buffer_size = {}",
                arr.len(),
                2 + buffer_size
            ));
        }
        let mut data = Vec::with_capacity(buffer_size);
        for item in &arr[2..] {
            let word = item.as_u64().ok_or_else(|| "dynamic bitset word must be a non-negative integer".to_owned())?;
            if word > u32::MAX as u64 {
                return Err(format!("dynamic bitset word {word} is out of u32 range"));
            }
            data.push(word as u32);
        }
        Ok(Self {
            size,
            data,
        })
    }
}
