//! Tokenizer for the Lark grammar syntax.

use serde_json::Value;

use super::{
    lark_error::LarkError, lark_location::LarkLocation, lark_token::LarkToken, lark_token_type::LarkTokenType,
};
use crate::support::parse_json;

/// Splits a Lark source into [`LarkToken`]s, tracking 1-based line and byte columns.
#[derive(Debug)]
pub struct LarkLexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    position: usize,
    line: i32,
    column: i32,
}

/// Whether `byte` is whitespace in the C locale (`std::isspace`).
pub fn is_c_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

impl<'a> LarkLexer<'a> {
    /// Creates a lexer over `source`.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// Tokenizes the whole source, ending with an [`LarkTokenType::End`] token.
    ///
    /// # Errors
    /// Returns a located [`LarkError`] on a malformed token.
    pub fn tokenize(mut self) -> Result<Vec<LarkToken>, LarkError> {
        let mut result = Vec::new();
        while self.position < self.bytes.len() {
            let c = self.bytes[self.position];
            if matches!(c, b' ' | b'\t' | 0x0C) {
                self.advance();
                continue;
            }
            if c == b'\r' || c == b'\n' {
                let location = self.current_location();
                self.advance();
                if c == b'\r' && self.peek_char(0) == b'\n' {
                    self.advance();
                }
                result.push(Self::token(LarkTokenType::Newline, "\n", location));
                continue;
            }
            if c == b'#' || (c == b'/' && self.peek_char(1) == b'/') {
                self.skip_comment();
                continue;
            }

            let location = self.current_location();
            let next = self.peek_char(1);
            let token = match c {
                b':' if next == b':' => self.simple_token(LarkTokenType::DoubleColon, 2),
                b':' => self.simple_token(LarkTokenType::Colon, 1),
                b',' => self.simple_token(LarkTokenType::Comma, 1),
                b'.' if next == b'.' => self.simple_token(LarkTokenType::DotDot, 2),
                b'.' => self.simple_token(LarkTokenType::Dot, 1),
                b'-' if next == b'>' => self.simple_token(LarkTokenType::Arrow, 2),
                b'-' if next.is_ascii_digit() => self.lex_number(),
                b'-' => return Err(self.error(location, "unexpected '-' character")),
                b'+' if next.is_ascii_digit() => self.lex_number(),
                b'+' => self.simple_token(LarkTokenType::Plus, 1),
                b'=' => self.simple_token(LarkTokenType::Equals, 1),
                b'(' => self.simple_token(LarkTokenType::LParen, 1),
                b')' => self.simple_token(LarkTokenType::RParen, 1),
                b'[' => self.simple_token(LarkTokenType::LBracket, 1),
                b']' => self.simple_token(LarkTokenType::RBracket, 1),
                b'{' => self.simple_token(LarkTokenType::LBrace, 1),
                b'}' => self.simple_token(LarkTokenType::RBrace, 1),
                b'|' => self.simple_token(LarkTokenType::Pipe, 1),
                b'&' => self.simple_token(LarkTokenType::And, 1),
                b'~' => self.simple_token(LarkTokenType::Tilde, 1),
                b'?' if next.is_ascii_alphabetic() || next == b'_' => self.lex_name(),
                b'?' => self.simple_token(LarkTokenType::Question, 1),
                b'*' => self.simple_token(LarkTokenType::Star, 1),
                b'!' if next.is_ascii_alphabetic() || next == b'_' => self.lex_name(),
                b'!' => return Err(self.error(location, "unexpected '!' character")),
                b'"' => self.lex_string()?,
                b'/' => self.lex_regex()?,
                b'<' => self.lex_special_token()?,
                b'@' => self.lex_grammar_ref()?,
                b'%' => self.lex_directive()?,
                _ if c.is_ascii_alphabetic() || c == b'_' => self.lex_name(),
                _ if c.is_ascii_digit() => self.lex_number(),
                _ => {
                    let character = self.source.get(self.position..).and_then(|rest| rest.chars().next());
                    let character = character.unwrap_or(char::REPLACEMENT_CHARACTER);
                    return Err(self.error(location, format!("unexpected character '{character}'")));
                },
            };
            result.push(token);
        }
        result.push(Self::token(LarkTokenType::End, "", self.current_location()));
        Ok(result)
    }

    fn token(
        ty: LarkTokenType,
        text: impl Into<String>,
        location: LarkLocation,
    ) -> LarkToken {
        LarkToken {
            ty,
            text: text.into(),
            flags: String::new(),
            location,
        }
    }

    fn error(
        &self,
        location: LarkLocation,
        message: impl Into<String>,
    ) -> LarkError {
        LarkError::at(self.source, location, message)
    }

    fn peek_char(
        &self,
        offset: usize,
    ) -> u8 {
        self.bytes.get(self.position + offset).copied().unwrap_or(0)
    }

    fn current_location(&self) -> LarkLocation {
        LarkLocation {
            line: self.line,
            column: self.column,
        }
    }

    fn advance(&mut self) {
        let Some(&c) = self.bytes.get(self.position) else {
            return;
        };
        self.position += 1;
        if c == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }

    fn advance_to(
        &mut self,
        end_position: usize,
    ) {
        while self.position < end_position {
            self.advance();
        }
    }

    fn slice(
        &self,
        start: usize,
        end: usize,
    ) -> String {
        String::from_utf8_lossy(&self.bytes[start..end]).into_owned()
    }

    fn simple_token(
        &mut self,
        ty: LarkTokenType,
        length: usize,
    ) -> LarkToken {
        let location = self.current_location();
        let text = self.slice(self.position, self.position + length);
        self.advance_to(self.position + length);
        Self::token(ty, text, location)
    }

    fn skip_comment(&mut self) {
        while self.position < self.bytes.len() && !matches!(self.bytes[self.position], b'\n' | b'\r') {
            self.advance();
        }
    }

    fn lex_name(&mut self) -> LarkToken {
        let location = self.current_location();
        let start = self.position;
        if matches!(self.bytes[self.position], b'!' | b'?') {
            self.advance();
        }
        while self.position < self.bytes.len() {
            let c = self.bytes[self.position];
            if !c.is_ascii_alphanumeric() && c != b'_' && c != b'-' {
                break;
            }
            self.advance();
        }
        Self::token(LarkTokenType::Name, self.slice(start, self.position), location)
    }

    fn advance_digits(&mut self) {
        while self.peek_char(0).is_ascii_digit() {
            self.advance();
        }
    }

    fn lex_number(&mut self) -> LarkToken {
        let location = self.current_location();
        let start = self.position;
        if matches!(self.bytes[self.position], b'+' | b'-') {
            self.advance();
        }
        self.advance_digits();
        if self.peek_char(0) == b'.' && self.peek_char(1) != b'.' {
            self.advance();
            self.advance_digits();
        }
        if matches!(self.peek_char(0), b'e' | b'E') {
            self.advance();
            if matches!(self.peek_char(0), b'+' | b'-') {
                self.advance();
            }
            self.advance_digits();
        }
        Self::token(LarkTokenType::Number, self.slice(start, self.position), location)
    }

    fn lex_string(&mut self) -> Result<LarkToken, LarkError> {
        let location = self.current_location();
        let start = self.position;
        self.advance();
        let mut escaped = false;
        while self.position < self.bytes.len() {
            let c = self.bytes[self.position];
            if !escaped && c == b'"' {
                self.advance();
                if self.peek_char(0) == b'i' {
                    self.advance();
                }
                return Ok(Self::token(LarkTokenType::String, self.slice(start, self.position), location));
            }
            if c == b'\n' || c == b'\r' {
                break;
            }
            escaped = !escaped && c == b'\\';
            self.advance();
        }
        Err(self.error(location, "unterminated string literal"))
    }

    fn lex_regex(&mut self) -> Result<LarkToken, LarkError> {
        let location = self.current_location();
        self.advance();
        let pattern_start = self.position;
        let mut escaped = false;
        while self.position < self.bytes.len() {
            let c = self.bytes[self.position];
            if !escaped && c == b'/' {
                let pattern = self.slice(pattern_start, self.position);
                self.advance();
                let flags_start = self.position;
                while self.peek_char(0).is_ascii_alphabetic() {
                    self.advance();
                }
                return Ok(LarkToken {
                    ty: LarkTokenType::Regex,
                    text: pattern,
                    flags: self.slice(flags_start, self.position),
                    location,
                });
            }
            escaped = !escaped && c == b'\\';
            self.advance();
        }
        Err(self.error(location, "unterminated regular expression"))
    }

    fn lex_special_token(&mut self) -> Result<LarkToken, LarkError> {
        let location = self.current_location();
        let start = self.position;
        self.advance();
        while self.position < self.bytes.len() && self.bytes[self.position] != b'>' {
            let c = self.bytes[self.position];
            if is_c_space(c) || c == b'<' {
                return Err(self.error(location, "invalid special token"));
            }
            self.advance();
        }
        if self.position == self.bytes.len() {
            return Err(self.error(location, "unterminated special token"));
        }
        self.advance();
        Ok(Self::token(LarkTokenType::SpecialToken, self.slice(start, self.position), location))
    }

    fn lex_grammar_ref(&mut self) -> Result<LarkToken, LarkError> {
        let location = self.current_location();
        let start = self.position;
        self.advance();
        while self.peek_char(0).is_ascii_alphanumeric() || matches!(self.peek_char(0), b'_' | b'-') {
            self.advance();
        }
        if self.position == start + 1 {
            return Err(self.error(location, "empty grammar reference"));
        }
        Ok(Self::token(LarkTokenType::GrammarRef, self.slice(start, self.position), location))
    }

    fn lex_directive(&mut self) -> Result<LarkToken, LarkError> {
        let location = self.current_location();
        let start = self.position;
        self.advance();
        while self.peek_char(0).is_ascii_alphabetic() || self.peek_char(0) == b'_' {
            self.advance();
        }
        let directive = self.slice(start, self.position);
        let ty = match directive.as_str() {
            "%json" => return self.lex_json_value(LarkTokenType::Json, location, &directive),
            "%regex" => return self.lex_json_value(LarkTokenType::RegexExt, location, &directive),
            "%grammar_options" => return self.lex_json_value(LarkTokenType::GrammarOptions, location, &directive),
            "%import" => LarkTokenType::Import,
            "%ignore" => LarkTokenType::Ignore,
            "%lark" => LarkTokenType::Lark,
            "%if" => LarkTokenType::If,
            _ => LarkTokenType::UnsupportedDirective,
        };
        Ok(Self::token(ty, directive, location))
    }

    /// Parses one JSON value from the remaining source and re-serializes it as the token text.
    fn lex_json_value(
        &mut self,
        ty: LarkTokenType,
        location: LarkLocation,
        directive: &str,
    ) -> Result<LarkToken, LarkError> {
        while self.position < self.bytes.len() && is_c_space(self.bytes[self.position]) {
            self.advance();
        }
        let rest = self.source.get(self.position..).unwrap_or_default();
        let mut stream = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
        let parsed = match stream.next() {
            Some(Ok(_)) => {
                let end = stream.byte_offset();
                parse_json(&rest[..end]).map(|value| (value, end))
            },
            Some(Err(error)) => Err(error.to_string()),
            None => Err("EOF while parsing a value".to_owned()),
        };
        let (value, end) = parsed
            .map_err(|error| self.error(location, format!("failed to parse JSON value after {directive}: {error}")))?;
        self.advance_to(self.position + end);
        Ok(Self::token(ty, value.to_string(), location))
    }
}
