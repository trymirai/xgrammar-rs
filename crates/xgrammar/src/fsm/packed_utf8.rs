//! Packed UTF-8 character ranges.
//!
//! The packed format stores the UTF-8 bytes of one codepoint left-aligned to its encoded length,
//! big-endian in a `u32`: a 2-byte character is `(byte0 << 8) | byte1`. Ranges of packed values
//! become FSM edges, with intermediate states for the continuation bytes.

use super::fsm::Fsm;

/// The largest 1-byte (ASCII) packed value.
pub const MAX_1_BYTE_UNICODE: u32 = 0x7F;
const MIN_2_BYTES_UNICODE: u32 = 0xC080;
const MAX_2_BYTES_UNICODE: u32 = 0xDFBF;
const MIN_3_BYTES_UNICODE: u32 = 0x00E0_8080;
const MAX_3_BYTES_UNICODE: u32 = 0x00EF_BFBF;
const MIN_4_BYTES_UNICODE: u32 = 0xF080_8080;
const MAX_4_BYTES_UNICODE: u32 = 0xF7BF_BFBF;

/// Converts a Unicode codepoint to the packed UTF-8 format.
#[must_use]
pub fn codepoint_to_packed_utf8(codepoint: u32) -> u32 {
    if codepoint <= 0x7F {
        codepoint
    } else if codepoint <= 0x7FF {
        let byte0 = 0xC0 | ((codepoint >> 6) & 0x1F);
        let byte1 = 0x80 | (codepoint & 0x3F);
        (byte0 << 8) | byte1
    } else if codepoint <= 0xFFFF {
        let byte0 = 0xE0 | ((codepoint >> 12) & 0x0F);
        let byte1 = 0x80 | ((codepoint >> 6) & 0x3F);
        let byte2 = 0x80 | (codepoint & 0x3F);
        (byte0 << 16) | (byte1 << 8) | byte2
    } else {
        let byte0 = 0xF0 | ((codepoint >> 18) & 0x07);
        let byte1 = 0x80 | ((codepoint >> 12) & 0x3F);
        let byte2 = 0x80 | ((codepoint >> 6) & 0x3F);
        let byte3 = 0x80 | (codepoint & 0x3F);
        (byte0 << 24) | (byte1 << 16) | (byte2 << 8) | byte3
    }
}

/// Adds edges (with intermediate states for multi-byte characters) from `from` to `to`
/// accepting every UTF-8 encoded character in the packed range `[min, max]`. Both bounds are
/// clamped to valid packed values first.
///
/// # Panics
/// Panics if `min > max`.
pub fn add_packed_utf8_range_edges(
    fsm: &mut Fsm,
    from: i32,
    to: i32,
    mut min: u32,
    mut max: u32,
) {
    assert!(min <= max, "Invalid character range: min ({min}) > max ({max})");
    if max > MAX_4_BYTES_UNICODE {
        max = MAX_4_BYTES_UNICODE;
    } else if max > MAX_3_BYTES_UNICODE {
        if max < MIN_4_BYTES_UNICODE {
            max = MAX_3_BYTES_UNICODE;
        }
    } else if max > MAX_2_BYTES_UNICODE {
        if max < MIN_3_BYTES_UNICODE {
            max = MAX_2_BYTES_UNICODE;
        }
    } else if max < MIN_2_BYTES_UNICODE && max > MAX_1_BYTE_UNICODE {
        max = MAX_1_BYTE_UNICODE;
    }

    if min > MAX_4_BYTES_UNICODE {
        min = MAX_4_BYTES_UNICODE;
    } else if min > MAX_3_BYTES_UNICODE {
        if min < MIN_4_BYTES_UNICODE {
            min = MIN_4_BYTES_UNICODE;
        }
    } else if min > MAX_2_BYTES_UNICODE {
        if min < MIN_3_BYTES_UNICODE {
            min = MIN_3_BYTES_UNICODE;
        }
    } else if min < MIN_2_BYTES_UNICODE && min > MAX_1_BYTE_UNICODE {
        min = MIN_2_BYTES_UNICODE;
    }

    // Split the range at the encoded-length boundaries.
    let length_ranges = [
        (0, MAX_1_BYTE_UNICODE),
        (MIN_2_BYTES_UNICODE, MAX_2_BYTES_UNICODE),
        (MIN_3_BYTES_UNICODE, MAX_3_BYTES_UNICODE),
        (MIN_4_BYTES_UNICODE, MAX_4_BYTES_UNICODE),
    ];
    for (length_min, length_max) in length_ranges {
        if min <= length_max && max >= length_min {
            add_same_length_character_range(fsm, from, to, min.max(length_min), max.min(length_max));
        }
    }
}

/// Adds the packed range `[min, max]` whose bounds have the same encoded length.
fn add_same_length_character_range(
    fsm: &mut Fsm,
    from: i32,
    to: i32,
    min: u32,
    max: u32,
) {
    let byte = |value: u32, i: u32| ((value >> (8 * i)) & 0xFF) as i32;
    // ASCII.
    if byte(max, 1) == 0 {
        fsm.add_edge(from, to, byte(min, 0), byte(max, 0));
        return;
    }
    // The index of the leading byte, and the masks of the trailing bytes.
    let lead = if byte(max, 3) != 0 {
        3
    } else if byte(max, 2) != 0 {
        2
    } else {
        1
    };
    let tail_mask = (1u32 << (8 * lead)) - 1;
    let tail_min = 0x8080_8080 & tail_mask;
    let tail_max = 0xBFBF_BFBF & tail_mask;
    let (mut lead_min, mut lead_max) = (byte(min, lead), byte(max, lead));

    if lead_min == lead_max {
        let next = fsm.add_state();
        fsm.add_edge(from, next, lead_min, lead_max);
        add_same_length_character_range(fsm, next, to, min & tail_mask, max & tail_mask);
        return;
    }
    if min & tail_mask == tail_min {
        lead_min -= 1;
    } else {
        let next = fsm.add_state();
        fsm.add_edge(from, next, lead_min, lead_min);
        add_same_length_character_range(fsm, next, to, min & tail_mask, tail_max);
    }
    if max & tail_mask == tail_max {
        lead_max += 1;
    } else {
        let next = fsm.add_state();
        fsm.add_edge(from, next, lead_max, lead_max);
        add_same_length_character_range(fsm, next, to, tail_min, max & tail_mask);
    }
    if lead_max - lead_min > 1 {
        // Full lead bytes in between, each followed by any continuation bytes.
        let mut current = fsm.add_state();
        fsm.add_edge(from, current, lead_min + 1, lead_max - 1);
        for _ in 1..lead {
            let next = fsm.add_state();
            fsm.add_edge(current, next, 0x80, 0xBF);
            current = next;
        }
        fsm.add_edge(current, to, 0x80, 0xBF);
    }
}
