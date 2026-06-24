//! Converts a (JavaScript-flavored) regex into EBNF — a port of `RegexConverter` in
//! `cpp/regex_converter.cc`.
//!
//! The grammar reference is
//! <https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions>.

use super::regex_error::RegexError;
use crate::grammar::Grammar;
use crate::support::{Codepoint, escape_codepoint, parse_next_escaped};

impl Grammar {
    /// Builds a grammar from a regular expression.
    ///
    /// # Errors
    /// Returns a [`RegexError`] if the regex is malformed or uses an unsupported feature.
    pub fn from_regex(regex: &str) -> Result<Grammar, RegexError> {
        let ebnf = regex_to_ebnf(regex, true)?;
        Grammar::from_ebnf(&ebnf, "root").map_err(|e| RegexError {
            position: 0,
            message: e.to_string(),
        })
    }
}

/// Escapes recognized in `HandleCharEscape` (each maps to itself); takes precedence over
/// the built-in C-style escapes in [`parse_next_escaped`].
const CHAR_ESCAPE_MAP: &[(u8, Codepoint)] = &[
    (b'^', 0x5E),
    (b'$', 0x24),
    (b'.', 0x2E),
    (b'*', 0x2A),
    (b'+', 0x2B),
    (b'?', 0x3F),
    (b'\\', 0x5C),
    (b'(', 0x28),
    (b')', 0x29),
    (b'[', 0x5B),
    (b']', 0x5D),
    (b'{', 0x7B),
    (b'}', 0x7D),
    (b'|', 0x7C),
    (b'/', 0x2F),
    (b'-', 0x2D),
];

/// Converts `regex` to an EBNF body. With `with_rule_name`, wraps it as a full `root` rule.
///
/// # Errors
/// Returns a [`RegexError`] (with position) on malformed or unsupported regex.
pub fn regex_to_ebnf(regex: &str, with_rule_name: bool) -> Result<String, RegexError> {
    let body = RegexConverter::new(regex).convert()?;
    if with_rule_name {
        Ok(format!("root ::= {body}\n"))
    } else {
        Ok(body)
    }
}

struct RegexConverter {
    chars: Vec<char>,
    cur: usize,
    end: usize,
    result: String,
    parenthesis_level: i32,
}

impl RegexConverter {
    fn new(regex: &str) -> Self {
        let chars: Vec<char> = regex.chars().collect();
        let end = chars.len();
        Self {
            chars,
            cur: 0,
            end,
            result: String::new(),
            parenthesis_level: 0,
        }
    }

    fn peek(&self, delta: isize) -> char {
        let idx = self.cur as isize + delta;
        if idx < 0 {
            return '\0';
        }
        self.chars.get(idx as usize).copied().unwrap_or('\0')
    }

    fn cur_char(&self) -> char {
        self.peek(0)
    }

    fn remaining(&self) -> usize {
        self.end - self.cur
    }

    fn error(&self, message: &str) -> RegexError {
        RegexError {
            position: self.cur + 1,
            message: message.to_owned(),
        }
    }

    /// Appends an EBNF segment, prefixing a space when the result is non-empty.
    fn add_segment(&mut self, element: &str) {
        if !self.result.is_empty() {
            self.result.push(' ');
        }
        self.result.push_str(element);
    }

    fn handle_character_class(&mut self) -> Result<String, RegexError> {
        let mut char_class = String::from("[");
        self.cur += 1;
        if self.cur_char() == ']' {
            return Err(self.error("Empty character class is not allowed in regex."));
        }
        while self.cur_char() != ']' && self.cur != self.end {
            if self.cur_char() == '\\' {
                char_class.push_str(&self.handle_escape_in_char_class()?);
            } else {
                char_class.push(self.cur_char());
                self.cur += 1;
            }
        }
        if self.cur == self.end {
            return Err(self.error("Unclosed '['"));
        }
        char_class.push(']');
        self.cur += 1;
        Ok(char_class)
    }

    // {x}, {x,}, {x,y}
    fn handle_repetition_range(&mut self) -> Result<String, RegexError> {
        let mut result = String::from("{");
        self.cur += 1;
        if !self.cur_char().is_ascii_digit() {
            return Err(self.error("Invalid repetition count."));
        }
        while self.cur_char().is_ascii_digit() {
            result.push(self.cur_char());
            self.cur += 1;
        }
        if self.cur_char() != ',' && self.cur_char() != '}' {
            return Err(self.error("Invalid repetition count."));
        }
        result.push(self.cur_char());
        self.cur += 1;
        if self.peek(-1) == '}' {
            return Ok(result);
        }
        if !self.cur_char().is_ascii_digit() && self.cur_char() != '}' {
            return Err(self.error("Invalid repetition count."));
        }
        while self.cur_char().is_ascii_digit() {
            result.push(self.cur_char());
            self.cur += 1;
        }
        if self.cur_char() != '}' {
            return Err(self.error("Invalid repetition count."));
        }
        result.push('}');
        self.cur += 1;
        Ok(result)
    }

    /// Parses an escape sequence beginning at the current `\`, mirroring `ParseNextEscaped`.
    fn try_parse_escaped(&self) -> Option<(Codepoint, usize)> {
        let bytes: Vec<u8> = self.chars[self.cur..self.end]
            .iter()
            .map(|&c| if (c as u32) < 128 { c as u8 } else { 0xFF })
            .collect();
        parse_next_escaped(&bytes, CHAR_ESCAPE_MAP).ok()
    }

    fn handle_char_escape(&mut self) -> Result<String, RegexError> {
        let c1 = self.peek(1);
        let rem = self.remaining();
        if rem < 2
            || (c1 == 'u' && rem < 5)
            || (c1 == 'x' && rem < 4)
            || (c1 == 'c' && rem < 3)
        {
            return Err(self.error("Escape sequence is not finished."));
        }

        if let Some((codepoint, len)) = self.try_parse_escaped() {
            self.cur += len;
            Ok(escape_codepoint(codepoint, &[]))
        } else if c1 == 'u' && self.peek(2) == '{' {
            self.cur += 3;
            let mut len = 0usize;
            let mut value: Codepoint = 0;
            while len <= 6 {
                match self.peek(len as isize).to_digit(16) {
                    Some(d) => {
                        value = value * 16 + d as Codepoint;
                        len += 1;
                    }
                    None => break,
                }
            }
            if len == 0 || len > 6 || self.peek(len as isize) != '}' {
                return Err(self.error("Invalid Unicode escape sequence."));
            }
            self.cur += len + 1;
            Ok(escape_codepoint(value, &[]))
        } else if c1 == 'c' {
            self.cur += 2;
            if !self.cur_char().is_ascii_alphabetic() {
                return Err(self.error("Invalid control character escape sequence."));
            }
            self.cur += 1;
            Ok(escape_codepoint((self.peek(-1) as Codepoint) % 32, &[]))
        } else {
            // Unrecognized escape: match the character itself.
            self.cur += 2;
            Ok(escape_codepoint(self.peek(-1) as Codepoint, &[]))
        }
    }

    fn handle_escape_in_char_class(&mut self) -> Result<String, RegexError> {
        if self.remaining() < 2 {
            return Err(self.error("Escape sequence is not finished."));
        }
        match self.peek(1) {
            'd' => {
                self.cur += 2;
                Ok("0-9".to_owned())
            }
            'D' => {
                self.cur += 2;
                Ok("\\x00-\\x2F\\x3A-\\U0010FFFF".to_owned())
            }
            'w' => {
                self.cur += 2;
                Ok("a-zA-Z0-9_".to_owned())
            }
            'W' => {
                self.cur += 2;
                Ok("\\x00-\\x2F\\x3A-\\x40\\x5B-\\x5E\\x60\\x7B-\\U0010FFFF".to_owned())
            }
            's' => {
                self.cur += 2;
                Ok("\\f\\n\\r\\t\\v\\u0020\\u00a0".to_owned())
            }
            'S' => {
                self.cur += 2;
                Ok("\\x00-\\x08\\x0E-\\x1F\\x21-\\x9F\\xA1-\\U0010FFFF".to_owned())
            }
            _ => {
                let res = self.handle_char_escape()?;
                if res == "]" || res == "-" {
                    Ok(format!("\\{res}"))
                } else {
                    Ok(res)
                }
            }
        }
    }

    fn handle_escape(&mut self) -> Result<String, RegexError> {
        if self.remaining() < 2 {
            return Err(self.error("Escape sequence is not finished."));
        }
        match self.peek(1) {
            'd' => {
                self.cur += 2;
                Ok("[0-9]".to_owned())
            }
            'D' => {
                self.cur += 2;
                Ok("[^0-9]".to_owned())
            }
            'w' => {
                self.cur += 2;
                Ok("[a-zA-Z0-9_]".to_owned())
            }
            'W' => {
                self.cur += 2;
                Ok("[^a-zA-Z0-9_]".to_owned())
            }
            's' => {
                self.cur += 2;
                Ok("[\\f\\n\\r\\t\\v\\u0020\\u00a0]".to_owned())
            }
            'S' => {
                self.cur += 2;
                Ok("[^[\\f\\n\\r\\t\\v\\u0020\\u00a0]".to_owned())
            }
            c if ('1'..='9').contains(&c) || c == 'k' => {
                Err(self.error("Backreference is not supported yet."))
            }
            'p' | 'P' => {
                Err(self.error("Unicode character class escape sequence is not supported yet."))
            }
            'b' | 'B' => Err(self.error("Word boundary is not supported yet.")),
            _ => Ok(format!("\"{}\"", self.handle_char_escape()?)),
        }
    }

    fn handle_group_modifier(&mut self) -> Result<(), RegexError> {
        if self.cur == self.end {
            return Err(self.error("Group modifier is not finished."));
        }
        match self.cur_char() {
            ':' => {
                self.cur += 1;
            }
            '=' | '!' => return Err(self.error("Lookahead is not supported yet.")),
            '<' if self.cur + 1 != self.end && matches!(self.peek(1), '=' | '!') => {
                return Err(self.error("Lookbehind is not supported yet."));
            }
            '<' => {
                self.cur += 1;
                while self.cur != self.end && self.cur_char().is_ascii_alphabetic() {
                    self.cur += 1;
                }
                if self.cur == self.end || self.cur_char() != '>' {
                    return Err(self.error("Invalid named capturing group."));
                }
                self.cur += 1;
            }
            _ => return Err(self.error("Group modifier flag is not supported yet.")),
        }
        Ok(())
    }

    fn convert(mut self) -> Result<String, RegexError> {
        let mut is_empty = true;
        while self.cur != self.end {
            match self.cur_char() {
                '^' => {
                    // '^' is only meaningful at the start; elsewhere it is ignored.
                    self.cur += 1;
                }
                '$' => {
                    // '$' is only meaningful at the end; elsewhere it is ignored.
                    self.cur += 1;
                }
                '[' => {
                    is_empty = false;
                    let class = self.handle_character_class()?;
                    self.add_segment(&class);
                }
                '(' => {
                    is_empty = false;
                    self.cur += 1;
                    self.parenthesis_level += 1;
                    self.add_segment("(");
                    if self.cur != self.end && self.cur_char() == '?' {
                        self.cur += 1;
                        self.handle_group_modifier()?;
                    }
                }
                ')' => {
                    is_empty = false;
                    if self.parenthesis_level == 0 {
                        return Err(self.error("Unmatched ')'"));
                    }
                    // If the previous character was '|', the alternative is empty.
                    if self.cur != 0 && self.peek(-1) == '|' {
                        self.add_segment("\"\"");
                    }
                    self.parenthesis_level -= 1;
                    self.add_segment(")");
                    self.cur += 1;
                }
                c @ ('*' | '+' | '?') => {
                    is_empty = false;
                    self.result.push(c);
                    self.cur += 1;
                    if self.cur != self.end && self.cur_char() == '?' {
                        // Ignore the non-greedy modifier; repetition is non-deterministic.
                        self.cur += 1;
                    }
                    if self.cur != self.end
                        && matches!(self.cur_char(), '{' | '*' | '+' | '?')
                    {
                        return Err(self.error("Two consecutive repetition modifiers are not allowed."));
                    }
                }
                '{' => {
                    is_empty = false;
                    let range = self.handle_repetition_range()?;
                    self.result.push_str(&range);
                    if self.cur != self.end && self.cur_char() == '?' {
                        self.cur += 1;
                    }
                    if self.cur != self.end
                        && matches!(self.cur_char(), '{' | '*' | '+' | '?')
                    {
                        return Err(self.error("Two consecutive repetition modifiers are not allowed."));
                    }
                }
                '|' => {
                    is_empty = false;
                    self.add_segment("|");
                    self.cur += 1;
                }
                '\\' => {
                    is_empty = false;
                    let escape = self.handle_escape()?;
                    self.add_segment(&escape);
                }
                '.' => {
                    is_empty = false;
                    self.add_segment("[\\u0000-\\U0010FFFF]");
                    self.cur += 1;
                }
                c => {
                    is_empty = false;
                    let segment = format!("\"{}\"", escape_codepoint(c as Codepoint, &[]));
                    self.add_segment(&segment);
                    self.cur += 1;
                }
            }
        }
        if self.parenthesis_level != 0 {
            return Err(self.error("The parenthesis is not closed."));
        }
        if is_empty {
            self.add_segment("\"\"");
        }
        Ok(self.result)
    }
}
