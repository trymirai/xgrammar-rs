//! UTF-8 and escape-sequence encoding/decoding — a port of `cpp/support/encoding.h`.
//!
//! Upstream signals failures via negative sentinel codepoints; this port uses
//! [`Result`] with [`CharHandlingError`] instead, and operates on byte slices rather than
//! NUL-terminated pointers.

use std::fmt::Write as _;

/// A Unicode codepoint (or a raw byte value treated as one).
pub type Codepoint = i32;

/// Failure when handling characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CharHandlingError {
    /// The UTF-8 byte sequence is invalid.
    #[error("invalid UTF-8")]
    InvalidUtf8,
    /// The escape sequence is invalid.
    #[error("invalid escape sequence")]
    InvalidEscape,
    /// The Latin-1 string is invalid.
    #[error("invalid Latin-1")]
    InvalidLatin1,
}

/// Encodes a codepoint to its raw UTF-8 bytes (returned rather than appended, and never
/// panicking on surrogates, unlike a `String`).
#[must_use]
pub fn char_to_utf8_bytes(codepoint: Codepoint) -> Vec<u8> {
    debug_assert!((0..=0x10_FFFF).contains(&codepoint), "invalid codepoint");
    let cp = codepoint as u32;
    if cp <= 0x7F {
        vec![cp as u8]
    } else if cp <= 0x7FF {
        vec![0xC0 | ((cp >> 6) & 0x1F) as u8, 0x80 | (cp & 0x3F) as u8]
    } else if cp <= 0xFFFF {
        vec![
            0xE0 | ((cp >> 12) & 0x0F) as u8,
            0x80 | ((cp >> 6) & 0x3F) as u8,
            0x80 | (cp & 0x3F) as u8,
        ]
    } else {
        vec![
            0xF0 | ((cp >> 18) & 0x07) as u8,
            0x80 | ((cp >> 12) & 0x3F) as u8,
            0x80 | ((cp >> 6) & 0x3F) as u8,
            0x80 | (cp & 0x3F) as u8,
        ]
    }
}

/// Decodes a UTF-8 leading byte into `(total_bytes, initial_codepoint_bits)`, or `None`
/// if the byte cannot start a sequence.
#[must_use]
pub fn handle_utf8_first_byte(byte: u8) -> Option<(usize, Codepoint)> {
    let (num_bytes, mask): (usize, u8) = match byte {
        0x00..=0x7F => (1, 0x7F),
        0xC0..=0xDF => (2, 0x1F),
        0xE0..=0xEF => (3, 0x0F),
        0xF0..=0xF7 => (4, 0x07),
        _ => return None,
    };
    Some((num_bytes, Codepoint::from(byte & mask)))
}

/// Decodes the first codepoint of `bytes`, returning it and how many bytes it consumed.
///
/// # Errors
/// Returns [`CharHandlingError::InvalidUtf8`] if `bytes` is empty or not valid UTF-8 at
/// its start.
pub fn parse_next_utf8(bytes: &[u8]) -> Result<(Codepoint, usize), CharHandlingError> {
    let &first = bytes.first().ok_or(CharHandlingError::InvalidUtf8)?;
    let (num_bytes, mut res) =
        handle_utf8_first_byte(first).ok_or(CharHandlingError::InvalidUtf8)?;
    for i in 1..num_bytes {
        match bytes.get(i) {
            Some(&b) if (b & 0xC0) == 0x80 => res = (res << 6) | Codepoint::from(b & 0x3F),
            _ => return Err(CharHandlingError::InvalidUtf8),
        }
    }
    Ok((res, num_bytes))
}

/// Decodes every codepoint in `bytes`.
///
/// On invalid UTF-8: if `preserve_invalid_bytes`, the offending byte is emitted verbatim
/// as a codepoint and decoding continues; otherwise the whole call fails.
///
/// # Errors
/// Returns [`CharHandlingError::InvalidUtf8`] on invalid input when not preserving bytes.
pub fn parse_utf8(
    bytes: &[u8],
    preserve_invalid_bytes: bool,
) -> Result<Vec<Codepoint>, CharHandlingError> {
    let mut codepoints = Vec::new();
    let mut rest = bytes;
    while let Some(&first) = rest.first() {
        match parse_next_utf8(rest) {
            Ok((cp, n)) => {
                codepoints.push(cp);
                rest = &rest[n..];
            }
            Err(_) if preserve_invalid_bytes => {
                codepoints.push(Codepoint::from(first));
                rest = &rest[1..];
            }
            Err(e) => return Err(e),
        }
    }
    Ok(codepoints)
}

/// Converts a hex digit (`0-9a-fA-F`) to its value.
#[must_use]
pub fn hex_char_to_int(c: u8) -> Option<u32> {
    match c {
        b'0'..=b'9' => Some(u32::from(c - b'0')),
        b'a'..=b'f' => Some(u32::from(c - b'a') + 10),
        b'A'..=b'F' => Some(u32::from(c - b'A') + 10),
        _ => None,
    }
}

fn default_codepoint_escape(codepoint: Codepoint) -> Option<&'static str> {
    Some(match codepoint {
        0x27 => "\\'",
        0x22 => "\\\"",
        0x3F => "\\?",
        0x5C => "\\\\",
        0x07 => "\\a",
        0x08 => "\\b",
        0x0C => "\\f",
        0x0A => "\\n",
        0x0D => "\\r",
        0x09 => "\\t",
        0x0B => "\\v",
        0x00 => "\\0",
        0x1B => "\\e",
        _ => return None,
    })
}

/// Escapes a codepoint into a printable string. `additional_escape_map` (codepoint →
/// escape) takes precedence over the built-in C-style escapes; non-printable codepoints
/// fall back to `\xNN` / `\uNNNN` / `\UNNNNNNNN`.
#[must_use]
pub fn escape_codepoint(codepoint: Codepoint, additional_escape_map: &[(Codepoint, &str)]) -> String {
    if let Some((_, s)) = additional_escape_map.iter().find(|(c, _)| *c == codepoint) {
        return (*s).to_owned();
    }
    if let Some(s) = default_codepoint_escape(codepoint) {
        return s.to_owned();
    }
    if (0x20..=0x7E).contains(&codepoint) {
        return (codepoint as u8 as char).to_string();
    }
    let cp = codepoint as u32;
    let (prefix, width) = if cp <= 0xFF {
        ('x', 2)
    } else if cp <= 0xFFFF {
        ('u', 4)
    } else {
        ('U', 8)
    };
    let mut out = String::with_capacity(2 + width);
    out.push('\\');
    out.push(prefix);
    let _ = write!(out, "{cp:0width$x}");
    out
}

/// Escapes a single raw byte into a printable string.
#[must_use]
pub fn escape_byte(raw_char: u8) -> String {
    escape_codepoint(Codepoint::from(raw_char), &[])
}

/// Escapes a raw byte sequence (decoding UTF-8, preserving invalid bytes) into a printable
/// form.
#[must_use]
pub fn escape_bytes(raw: &[u8]) -> String {
    let codepoints = parse_utf8(raw, true).expect("preserve_invalid_bytes never errors");
    let mut out = String::new();
    for cp in codepoints {
        out.push_str(&escape_codepoint(cp, &[]));
    }
    out
}

/// Escapes a whole string (decoding UTF-8, preserving invalid bytes) into a printable form.
#[must_use]
pub fn escape_str(raw: &str) -> String {
    escape_bytes(raw.as_bytes())
}

fn default_escape_to_codepoint(escape_char: u8) -> Option<Codepoint> {
    Some(match escape_char {
        b'\'' => 0x27,
        b'"' => 0x22,
        b'?' => 0x3F,
        b'\\' => 0x5C,
        b'/' => 0x2F,
        b'a' => 0x07,
        b'b' => 0x08,
        b'f' => 0x0C,
        b'n' => 0x0A,
        b'r' => 0x0D,
        b't' => 0x09,
        b'v' => 0x0B,
        b'0' => 0x00,
        b'e' => 0x1B,
        _ => return None,
    })
}

/// Parses the first escape sequence in `data`, which must begin with `\`.
///
/// `additional_escape_map` (escape char → codepoint) takes precedence over the built-in
/// C-style escapes. Supports `\xHH...` (arbitrary-length hex), `\uHHHH`, and `\UHHHHHHHH`.
///
/// # Errors
/// Returns [`CharHandlingError::InvalidEscape`] if `data` does not start with a valid
/// escape sequence.
pub fn parse_next_escaped(
    data: &[u8],
    additional_escape_map: &[(u8, Codepoint)],
) -> Result<(Codepoint, usize), CharHandlingError> {
    if data.first() != Some(&b'\\') {
        return Err(CharHandlingError::InvalidEscape);
    }
    let &second = data.get(1).ok_or(CharHandlingError::InvalidEscape)?;
    if second > 128 {
        return Err(CharHandlingError::InvalidEscape);
    }

    if let Some((_, cp)) = additional_escape_map.iter().find(|(c, _)| *c == second) {
        return Ok((*cp, 2));
    }
    if let Some(cp) = default_escape_to_codepoint(second) {
        return Ok((cp, 2));
    }

    match second {
        b'x' => {
            // arbitrary-length hex
            let mut len = 0usize;
            let mut codepoint: Codepoint = 0;
            while let Some(digit) = data.get(2 + len).copied().and_then(hex_char_to_int) {
                codepoint = codepoint * 16 + digit as Codepoint;
                len += 1;
            }
            if len == 0 {
                return Err(CharHandlingError::InvalidEscape);
            }
            Ok((codepoint, len + 2))
        }
        b'u' | b'U' => {
            let len = if second == b'u' { 4 } else { 8 };
            let mut codepoint: Codepoint = 0;
            for i in 0..len {
                let digit = data
                    .get(2 + i)
                    .copied()
                    .and_then(hex_char_to_int)
                    .ok_or(CharHandlingError::InvalidEscape)?;
                codepoint = codepoint * 16 + digit as Codepoint;
            }
            Ok((codepoint, len + 2))
        }
        _ => Err(CharHandlingError::InvalidEscape),
    }
}

/// Decodes the first codepoint of `data`, transparently handling a leading `\` escape.
///
/// # Errors
/// Returns [`CharHandlingError::InvalidUtf8`] or [`CharHandlingError::InvalidEscape`]
/// depending on which form failed.
pub fn parse_next_utf8_or_escaped(
    data: &[u8],
    additional_escape_map: &[(u8, Codepoint)],
) -> Result<(Codepoint, usize), CharHandlingError> {
    if data.first() == Some(&b'\\') {
        parse_next_escaped(data, additional_escape_map)
    } else {
        parse_next_utf8(data)
    }
}

/// Converts a Latin-1 string (whose non-ASCII chars are UTF-8-encoded in `latin1`) to its
/// raw byte sequence.
///
/// # Errors
/// Returns [`CharHandlingError::InvalidLatin1`] on malformed input.
pub fn latin1_to_bytes(latin1: &[u8]) -> Result<Vec<u8>, CharHandlingError> {
    let mut result = Vec::with_capacity(latin1.len());
    let mut i = 0;
    while i < latin1.len() {
        let c1 = latin1[i];
        if c1 < 0x80 {
            result.push(c1);
            i += 1;
        } else {
            let c2 = *latin1.get(i + 1).ok_or(CharHandlingError::InvalidLatin1)?;
            if (c2 & 0xC0) != 0x80 {
                return Err(CharHandlingError::InvalidLatin1);
            }
            let code = (u32::from(c1 & 0x1F) << 6) | u32::from(c2 & 0x3F);
            if !(0x80..=0xFF).contains(&code) {
                return Err(CharHandlingError::InvalidLatin1);
            }
            result.push(code as u8);
            i += 2;
        }
    }
    Ok(result)
}

/// Converts a raw byte sequence to a Latin-1 string (bytes ≥ 0x80 become two UTF-8 bytes).
#[must_use]
pub fn byte_to_latin1(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if b <= 0x7F {
            result.push(b);
        } else {
            result.push(0xC0 | (b >> 6));
            result.push(0x80 | (b & 0x3F));
        }
    }
    result
}
