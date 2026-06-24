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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(hash_combine(&[]), 0);
    }

    #[test]
    fn order_sensitive() {
        assert_ne!(hash_combine(&[1, 2]), hash_combine(&[2, 1]));
    }

    #[test]
    fn deterministic() {
        assert_eq!(hash_combine(&[7, 8, 9]), hash_combine(&[7, 8, 9]));
    }

    #[test]
    fn matches_reference_mixing() {
        // Reproduce the C++ formula by hand for a single combine from seed 0.
        let mut seed = 0u64;
        hash_combine_binary(&mut seed, 42);
        let expected = 42u64
            .wrapping_add(HASH_MIX)
            .wrapping_add(0 << 6)
            .wrapping_add(0 >> 2);
        assert_eq!(seed, 0 ^ expected);
    }

    #[test]
    fn no_overflow_panic_on_large_values() {
        // wrapping arithmetic must not panic in debug builds.
        let _ = hash_combine(&[u64::MAX, u64::MAX, u64::MAX]);
    }
}
