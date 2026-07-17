//! Small byte-sequence helpers shared by the matcher and adaptive-mask builder.

/// Length of the longest common prefix of `a` and `b`.
#[must_use]
pub fn common_prefix_len(
    a: &[u8],
    b: &[u8],
) -> i32 {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count() as i32
}
