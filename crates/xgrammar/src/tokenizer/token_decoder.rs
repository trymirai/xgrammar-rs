//! Decodes raw vocabulary tokens into their byte strings — a port of `TokenDecoder` in
//! `cpp/tokenizer_info.cc`.

use super::vocab_type::VocabType;
use crate::support::parse_utf8;

/// Decodes an encoded vocabulary `token` into its byte string per `vocab_type`.
#[must_use]
pub fn decode_token(
    token: &str,
    vocab_type: VocabType,
) -> Vec<u8> {
    match vocab_type {
        VocabType::ByteFallback => {
            space_replacer_decoder(&byte_fallback_decoder(token))
        },
        VocabType::ByteLevel => byte_level_decoder(token),
        VocabType::Raw => token.as_bytes().to_vec(),
    }
}

/// Transforms `<0xNN>` byte tokens into the raw byte `NN`; other tokens pass through.
fn byte_fallback_decoder(token: &str) -> Vec<u8> {
    let bytes = token.as_bytes();
    if bytes.len() == 6 && &bytes[0..3] == b"<0x" && bytes[5] == b'>' {
        let hi = hex_val(bytes[3]);
        let lo = hex_val(bytes[4]);
        if let (Some(hi), Some(lo)) = (hi, lo) {
            return vec![(hi * 16 + lo) as u8];
        }
    }
    bytes.to_vec()
}

/// Hex digit value, accepting `0-9` and uppercase `A-F` (matching the C++).
fn hex_val(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some(u32::from(c - b'0')),
        b'A'..=b'F' => Some(u32::from(c - b'A') + 10),
        _ => None,
    }
}

/// Replaces the `▁` block (U+2581, UTF-8 `E2 96 81`) with a space.
fn space_replacer_decoder(token: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(token.len());
    let mut i = 0;
    while i < token.len() {
        if i + 2 < token.len()
            && token[i] == 0xE2
            && token[i + 1] == 0x96
            && token[i + 2] == 0x81
        {
            result.push(b' ');
            i += 3;
        } else {
            result.push(token[i]);
            i += 1;
        }
    }
    result
}

/// Inverts the GPT-2 bytes-to-unicode mapping; tokens with unmapped codepoints pass through.
fn byte_level_decoder(token: &str) -> Vec<u8> {
    let Ok(codepoints) = parse_utf8(token.as_bytes(), false) else {
        return token.as_bytes().to_vec();
    };
    let mut decoded = Vec::with_capacity(codepoints.len());
    for cp in codepoints {
        if cp < 0
            || cp as usize >= CHAR_TO_BYTE_MAP.len()
            || CHAR_TO_BYTE_MAP[cp as usize] == -1
        {
            return token.as_bytes().to_vec();
        }
        decoded.push(CHAR_TO_BYTE_MAP[cp as usize] as u8);
    }
    decoded
}

/// The inverse of GPT-2's `bytes_to_unicode`; `-1` marks codepoints with no mapping.
#[rustfmt::skip]
const CHAR_TO_BYTE_MAP: [i32; 324] = [
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
    46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68,
    69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91,
    92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
    112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, -1,
    174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209,
    210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227,
    228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245,
    246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 127, 128,
    129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146,
    147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 173,
];
