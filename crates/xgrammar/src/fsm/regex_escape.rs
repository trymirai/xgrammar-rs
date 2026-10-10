//! One parsed regex escape sequence.

use super::CodepointRange;
use crate::support::{hex_char_to_int, parse_next_utf8};

/// The largest Unicode codepoint.
pub const MAX_CODEPOINT: u32 = 0x10_FFFF;

/// A parsed regex escape: a single codepoint, or a (possibly negated) set of codepoint ranges
/// for class escapes like `\d`, `\D`, `\w`, `\W`, `\s`, `\S`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexEscape {
    /// A single codepoint.
    Single(u32),
    /// A class escape.
    Class {
        /// The ranges of the (non-negated) class.
        ranges: Vec<CodepointRange>,
        /// Whether the escape matches the complement of `ranges`.
        negated: bool,
    },
}

impl RegexEscape {
    /// Parses the escape sequence starting at `regex[*pos] == b'\\'`, advancing `*pos` past it.
    /// Inside a character class (`in_class`), `\b` is the backspace character instead of a word
    /// boundary assertion.
    ///
    /// # Errors
    /// Returns a message for unsupported (word boundaries, Unicode properties, backreferences)
    /// or malformed escapes.
    pub fn parse(
        regex: &[u8],
        pos: &mut usize,
        in_class: bool,
    ) -> Result<Self, String> {
        debug_assert_eq!(regex[*pos], b'\\');
        let Some(&escaped) = regex.get(*pos + 1) else {
            return Err("Regex ends with a trailing backslash".to_owned());
        };
        *pos += 2;
        let class = |ranges: Vec<CodepointRange>, negated: bool| {
            Ok(Self::Class {
                ranges,
                negated,
            })
        };
        match escaped {
            b'n' => Ok(Self::Single(0x0A)),
            b't' => Ok(Self::Single(0x09)),
            b'r' => Ok(Self::Single(0x0D)),
            b'f' => Ok(Self::Single(0x0C)),
            b'v' => Ok(Self::Single(0x0B)),
            b'a' => Ok(Self::Single(0x07)),
            b'e' => Ok(Self::Single(0x1B)),
            b'0' => Ok(Self::Single(0)),
            b'b' if in_class => Ok(Self::Single(0x08)),
            b'b' => Err("Word boundary assertion \\b is not supported in regex".to_owned()),
            b'B' => Err("Word boundary assertion \\B is not supported in regex".to_owned()),
            b'p' | b'P' => Err("Unicode property escape \\p / \\P is not supported in regex".to_owned()),
            b'k' => Err("Backreference \\k is not supported in regex".to_owned()),
            b'1'..=b'9' => Err(format!("Backreference \\{} is not supported in regex", char::from(escaped))),
            b'd' | b'D' => class(vec![('0' as u32, '9' as u32)], escaped == b'D'),
            b'w' | b'W' => class(
                vec![
                    ('0' as u32, '9' as u32),
                    ('A' as u32, 'Z' as u32),
                    ('_' as u32, '_' as u32),
                    ('a' as u32, 'z' as u32),
                ],
                escaped == b'W',
            ),
            b's' | b'S' => class(vec![(0x09, 0x0D), (' ' as u32, ' ' as u32)], escaped == b'S'),
            b'x' => {
                let codepoint = regex
                    .get(*pos..*pos + 2)
                    .and_then(hex_value)
                    .ok_or("\\x must be followed by two hexadecimal digits in regex")?;
                *pos += 2;
                Ok(Self::Single(codepoint))
            },
            b'u' if regex.get(*pos) == Some(&b'{') => {
                let error = "\\u{...} must contain one to six hexadecimal digits in regex";
                let digits_begin = *pos + 1;
                let close = regex[digits_begin..]
                    .iter()
                    .position(|&c| c == b'}')
                    .map(|offset| digits_begin + offset)
                    .filter(|&close| close > digits_begin && close <= *pos + 7)
                    .ok_or(error)?;
                let codepoint = hex_value(&regex[digits_begin..close]).ok_or(error)?;
                if codepoint > MAX_CODEPOINT {
                    return Err("\\u{...} escape is beyond the Unicode range in regex".to_owned());
                }
                *pos = close + 1;
                Ok(Self::Single(codepoint))
            },
            b'u' => {
                let codepoint = regex
                    .get(*pos..*pos + 4)
                    .and_then(hex_value)
                    .ok_or("\\u must be followed by four hexadecimal digits in regex")?;
                *pos += 4;
                Ok(Self::Single(codepoint))
            },
            b'c' => match regex.get(*pos) {
                Some(&letter) if letter.is_ascii_alphabetic() => {
                    *pos += 1;
                    Ok(Self::Single(u32::from(letter) & 0x1F))
                },
                _ => Err("\\c must be followed by a letter in regex".to_owned()),
            },
            0x80..=0xFF => {
                // A multi-byte UTF-8 character after the backslash matches literally.
                let (codepoint, num_bytes) =
                    parse_next_utf8(&regex[*pos - 1..]).map_err(|_| "Invalid UTF-8 in regex escape sequence")?;
                *pos += num_bytes - 1;
                Ok(Self::Single(codepoint as u32))
            },
            _ => {
                if escaped.is_ascii_alphanumeric() {
                    eprintln!(
                        "Escape sequence \\{} is not recognized in regex; matching the character literally",
                        char::from(escaped)
                    );
                }
                Ok(Self::Single(u32::from(escaped)))
            },
        }
    }
}

/// The value of a non-empty run of hexadecimal digits, or `None` if any byte is not one.
fn hex_value(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0u32, |value, &digit| Some(value * 16 + hex_char_to_int(digit)?))
}
