//! Hash combining — a port of the boost-derived helpers in `cpp/support/utils.h`.

/// The boost `hash_combine` golden-ratio mixing constant.
const HASH_MIX: u64 = 0x9e37_79b9_7f4a_7c15;

/// Mixes `value` into `seed` (boost `hash_combine`).
#[inline]
pub fn hash_combine_binary(seed: &mut u64, value: u64) {
    *seed ^= value
        .wrapping_add(HASH_MIX)
        .wrapping_add(*seed << 6)
        .wrapping_add(*seed >> 2);
}

/// Combines a sequence of (already-hashed) values into a single hash, starting from a
/// zero seed.
#[must_use]
pub fn hash_combine(values: &[u64]) -> u64 {
    let mut seed = 0u64;
    for &value in values {
        hash_combine_binary(&mut seed, value);
    }
    seed
}
