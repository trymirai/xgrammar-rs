//! The EBNF lexer — a port of `EBNFLexer` in `cpp/grammar_parser.cc`.

use super::{
    lexer_error::LexerError,
    token::{Token, TokenValue},
    token_type::TokenType,
};
use crate::support::{
    CharHandlingError, Codepoint, char_to_utf8_bytes,
    parse_next_utf8_or_escaped,
};

/// Largest integer literal permitted in a grammar (`1e15`).
const MAX_INTEGER_IN_GRAMMAR: i64 = 1_000_000_000_000_000;

/// Escapes recognized inside a character class (each maps to itself), e.g. `\.`, `\-`, `\*`.
const REGEX_ESCAPE_CHARS: &[(u8, Codepoint)] = &[
    (b'^', 0x5E),
    (b'$', 0x24),
    (b'\\', 0x5C),
    (b'.', 0x2E),
    (b'*', 0x2A),
    (b'+', 0x2B),
    (b'?', 0x3F),
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

fn is_regex_special_escape(c: u8) -> bool {
    matches!(c, b'd' | b'D' | b's' | b'S' | b'w' | b'W')
}

/// Tokenizes an EBNF grammar string.
///
/// # Errors
/// Returns a [`LexerError`] (with source position) on malformed input.
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexerError> {
    Lexer::new(input.as_bytes()).run()
}

enum Next {
    One(Token),
    Many(Vec<Token>),
}

struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    line: i32,
    column: i32,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(
        &self,
        delta: isize,
    ) -> u8 {
        let idx = self.pos as isize + delta;
        if idx < 0 {
            return 0;
        }
        self.input.get(idx as usize).copied().unwrap_or(0)
    }

    fn cur(&self) -> u8 {
        self.peek(0)
    }

    fn consume(
        &mut self,
        cnt: usize,
    ) {
        for _ in 0..cnt {
            let b = self.cur();
            if b == b'\n' || (b == b'\r' && self.peek(1) != b'\n') {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.pos += 1;
        }
    }

    fn err(
        &self,
        message: impl Into<String>,
    ) -> LexerError {
        LexerError {
            line: self.line,
            column: self.column,
            message: message.into(),
        }
    }

    fn char_err(
        &self,
        e: CharHandlingError,
    ) -> LexerError {
        self.err(match e {
            CharHandlingError::InvalidUtf8 => "Invalid UTF8 sequence",
            CharHandlingError::InvalidEscape => "Invalid escape sequence",
            CharHandlingError::InvalidLatin1 => "Invalid Latin-1 sequence",
        })
    }

    fn slice_string(
        &self,
        start: usize,
        end: usize,
    ) -> String {
        String::from_utf8_lossy(&self.input[start..end]).into_owned()
    }

    fn punct(
        &self,
        ty: TokenType,
        lexeme: &str,
        line: i32,
        column: i32,
    ) -> Token {
        Token {
            ty,
            lexeme: lexeme.to_owned(),
            value: TokenValue::None,
            line,
            column,
        }
    }

    fn consume_space(&mut self) {
        while self.cur() != 0
            && matches!(self.cur(), b' ' | b'\t' | b'#' | b'\n' | b'\r')
        {
            self.consume(1);
            if self.peek(-1) == b'#' {
                while self.cur() != 0
                    && self.cur() != b'\n'
                    && self.cur() != b'\r'
                {
                    self.consume(1);
                }
                if self.cur() == 0 {
                    return;
                }
                self.consume(1);
                if self.peek(-1) == b'\r' && self.cur() == b'\n' {
                    self.consume(1);
                }
            }
        }
    }

    fn is_name_char(
        c: u8,
        is_first: bool,
    ) -> bool {
        c == b'_'
            || c == b'-'
            || c == b'.'
            || c.is_ascii_lowercase()
            || c.is_ascii_uppercase()
            || (!is_first && c.is_ascii_digit())
    }

    fn parse_identifier(&mut self) -> Result<String, LexerError> {
        let start = self.pos;
        let mut first = true;
        while self.cur() != 0 && Self::is_name_char(self.cur(), first) {
            self.consume(1);
            first = false;
        }
        if self.pos == start {
            return Err(self.err("Expect identifier"));
        }
        Ok(self.slice_string(start, self.pos))
    }

    fn parse_identifier_or_boolean(&mut self) -> Result<Token, LexerError> {
        let line = self.line;
        let column = self.column;
        let id = self.parse_identifier()?;
        if id == "true" || id == "false" {
            let value = id == "true";
            return Ok(Token {
                ty: TokenType::BooleanLiteral,
                lexeme: id,
                value: TokenValue::Bool(value),
                line,
                column,
            });
        }
        Ok(Token {
            ty: TokenType::Identifier,
            lexeme: id.clone(),
            value: TokenValue::Str(id),
            line,
            column,
        })
    }

    fn parse_string(&mut self) -> Result<Token, LexerError> {
        let line = self.line;
        let column = self.column;
        let start_pos = self.pos;
        self.consume(1); // opening quote

        let mut bytes = Vec::new();
        while self.cur() != 0
            && self.cur() != b'"'
            && self.cur() != b'\n'
            && self.cur() != b'\r'
        {
            let (codepoint, len) =
                parse_next_utf8_or_escaped(&self.input[self.pos..], &[])
                    .map_err(|e| self.char_err(e))?;
            self.consume(len);
            bytes.extend_from_slice(&char_to_utf8_bytes(codepoint));
        }
        if self.cur() != b'"' {
            return Err(self.err("Expect \" in string literal"));
        }
        self.consume(1); // closing quote

        Ok(Token {
            ty: TokenType::StringLiteral,
            lexeme: self.slice_string(start_pos, self.pos),
            value: TokenValue::Str(
                String::from_utf8_lossy(&bytes).into_owned(),
            ),
            line,
            column,
        })
    }

    fn parse_char_class(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens =
            vec![self.punct(TokenType::LBracket, "[", self.line, self.column)];
        self.consume(1); // '['

        if self.cur() == b'^' {
            tokens.push(self.punct(
                TokenType::Caret,
                "^",
                self.line,
                self.column,
            ));
            self.consume(1);
        }

        while self.cur() != 0 && self.cur() != b']' {
            if self.cur() == b'\r' || self.cur() == b'\n' {
                return Err(
                    self.err("Character class should not contain newline")
                );
            } else if self.cur() == b'-' {
                tokens.push(self.punct(
                    TokenType::Dash,
                    "-",
                    self.line,
                    self.column,
                ));
                self.consume(1);
            } else if self.cur() == b'\\'
                && is_regex_special_escape(self.peek(1))
            {
                let (line, column) = (self.line, self.column);
                let lexeme = self.slice_string(self.pos, self.pos + 2);
                let value = self.slice_string(self.pos + 1, self.pos + 2);
                tokens.push(Token {
                    ty: TokenType::EscapeInCharClass,
                    lexeme,
                    value: TokenValue::Str(value),
                    line,
                    column,
                });
                self.consume(2);
            } else {
                let (line, column) = (self.line, self.column);
                let (codepoint, len) = parse_next_utf8_or_escaped(
                    &self.input[self.pos..],
                    REGEX_ESCAPE_CHARS,
                )
                .map_err(|e| self.char_err(e))?;
                let lexeme = self.slice_string(self.pos, self.pos + len);
                tokens.push(Token {
                    ty: TokenType::CharInCharClass,
                    lexeme,
                    value: TokenValue::Codepoint(codepoint),
                    line,
                    column,
                });
                self.consume(len);
            }
        }

        if self.cur() == 0 {
            return Err(self.err("Unterminated character class"));
        }
        tokens.push(self.punct(
            TokenType::RBracket,
            "]",
            self.line,
            self.column,
        ));
        self.consume(1); // ']'
        Ok(tokens)
    }

    fn parse_integer(&mut self) -> Result<Token, LexerError> {
        let line = self.line;
        let column = self.column;
        let start_pos = self.pos;

        let mut is_negative = false;
        if self.cur() == b'-' {
            is_negative = true;
            self.consume(1);
        } else if self.cur() == b'+' {
            self.consume(1);
        }

        let mut num: i64 = 0;
        while self.cur() != 0 && self.cur().is_ascii_digit() {
            num = num * 10 + i64::from(self.cur() - b'0');
            self.consume(1);
            if num > MAX_INTEGER_IN_GRAMMAR {
                return Err(self.err(format!(
                    "Integer is too large: parsed {num}, max allowed is {MAX_INTEGER_IN_GRAMMAR}"
                )));
            }
        }

        Ok(Token {
            ty: TokenType::IntegerLiteral,
            lexeme: self.slice_string(start_pos, self.pos),
            value: TokenValue::Int(if is_negative {
                -num
            } else {
                num
            }),
            line,
            column,
        })
    }

    fn next_token(&mut self) -> Result<Next, LexerError> {
        self.consume_space();
        let (line, column) = (self.line, self.column);

        if self.cur() == 0 {
            return Ok(Next::One(self.punct(
                TokenType::EndOfFile,
                "",
                line,
                column,
            )));
        }

        let one = |ty, lexeme, lexer: &Lexer| {
            Next::One(lexer.punct(ty, lexeme, line, column))
        };
        Ok(match self.cur() {
            b'(' => {
                if self.peek(1) == b'=' {
                    self.consume(2);
                    one(TokenType::LookaheadLParen, "(=", self)
                } else {
                    self.consume(1);
                    one(TokenType::LParen, "(", self)
                }
            },
            b')' => {
                self.consume(1);
                one(TokenType::RParen, ")", self)
            },
            b'{' => {
                self.consume(1);
                one(TokenType::LBrace, "{", self)
            },
            b'}' => {
                self.consume(1);
                one(TokenType::RBrace, "}", self)
            },
            b'|' => {
                self.consume(1);
                one(TokenType::Pipe, "|", self)
            },
            b',' => {
                self.consume(1);
                one(TokenType::Comma, ",", self)
            },
            b'*' => {
                self.consume(1);
                one(TokenType::Star, "*", self)
            },
            b'+' => {
                self.consume(1);
                one(TokenType::Plus, "+", self)
            },
            b'?' => {
                self.consume(1);
                one(TokenType::Question, "?", self)
            },
            b'=' => {
                self.consume(1);
                one(TokenType::Equal, "=", self)
            },
            b':' => {
                if self.peek(1) == b':' && self.peek(2) == b'=' {
                    self.consume(3);
                    one(TokenType::Assign, "::=", self)
                } else {
                    return Err(self.err("Unexpected character: ':'"));
                }
            },
            b'"' => Next::One(self.parse_string()?),
            b'[' => Next::Many(self.parse_char_class()?),
            c => {
                if Self::is_name_char(c, true) {
                    // Note: `-` is a name char, so negatives like `-1` lex as identifiers here.
                    Next::One(self.parse_identifier_or_boolean()?)
                } else if c.is_ascii_digit() || c == b'+' {
                    Next::One(self.parse_integer()?)
                } else {
                    return Err(self
                        .err(format!("Unexpected character: {}", c as char)));
                }
            },
        })
    }

    /// Promotes each identifier immediately left of `::=` to a [`TokenType::RuleName`].
    fn convert_identifier_to_rule_name(
        tokens: &mut [Token]
    ) -> Result<(), LexerError> {
        for i in 0..tokens.len() {
            if tokens[i].ty != TokenType::Assign {
                continue;
            }
            if i == 0 {
                return Err(LexerError {
                    line: tokens[i].line,
                    column: tokens[i].column,
                    message: "Assign should not be the first token".to_owned(),
                });
            }
            if tokens[i - 1].ty != TokenType::Identifier {
                return Err(LexerError {
                    line: tokens[i - 1].line,
                    column: tokens[i - 1].column,
                    message: "Assign should be preceded by an identifier"
                        .to_owned(),
                });
            }
            if i >= 2 && tokens[i - 2].line == tokens[i - 1].line {
                return Err(LexerError {
                    line: tokens[i - 1].line,
                    column: tokens[i - 1].column,
                    message:
                        "The rule name should be at the beginning of the line"
                            .to_owned(),
                });
            }
            tokens[i - 1].ty = TokenType::RuleName;
        }
        Ok(())
    }

    fn run(mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();
        loop {
            match self.next_token()? {
                Next::One(token) => {
                    let is_eof = token.ty == TokenType::EndOfFile;
                    tokens.push(token);
                    if is_eof {
                        break;
                    }
                },
                Next::Many(many) => tokens.extend(many),
            }
        }
        Self::convert_identifier_to_rule_name(&mut tokens)?;
        Ok(tokens)
    }
}
