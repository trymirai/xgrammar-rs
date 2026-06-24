//! The EBNF parser — a port of `EBNFParser` in `cpp/grammar_parser.cc`.
//!
//! Produces the raw (un-normalized) BNF AST. The four macros (`TagDispatch`, `Token`,
//! `ExcludeToken`, `TokenTagDispatch`) are not yet ported and are reported as errors;
//! they are a follow-up.

use super::ebnf_error::EbnfError;
use super::ebnf_lexer::tokenize;
use super::parse_error::ParserError;
use super::token::Token;
use super::token_type::TokenType;
use crate::grammar::{CharacterClassElement, Grammar, GrammarBuilder, GrammarExprType, NO_EXPR};

const MAX_NEST_LAYER: i32 = 1000;

/// Macro names that are reserved and not yet supported by this parser.
const MACRO_NAMES: &[&str] = &["TagDispatch", "Token", "ExcludeToken", "TokenTagDispatch"];

/// Parses an EBNF grammar string into the raw (un-normalized) BNF AST.
///
/// # Errors
/// Returns [`EbnfError`] on a lexing or parsing failure.
pub fn ebnf_to_grammar_no_normalization(
    ebnf_string: &str,
    root_rule_name: &str,
) -> Result<Grammar, EbnfError> {
    let tokens = tokenize(ebnf_string)?;
    Ok(Parser::new(tokens, root_rule_name).parse()?)
}

struct Parser {
    builder: GrammarBuilder,
    tokens: Vec<Token>,
    pos: usize,
    cur_rule_name: String,
    root_rule_name: String,
    nest_layer_guard: i32,
}

impl Parser {
    fn new(tokens: Vec<Token>, root_rule_name: &str) -> Self {
        Self {
            builder: GrammarBuilder::new(),
            tokens,
            pos: 0,
            cur_rule_name: String::new(),
            root_rule_name: root_rule_name.to_owned(),
            nest_layer_guard: 0,
        }
    }

    fn peek(&self, delta: isize) -> &Token {
        let last = self.tokens.len() as isize - 1;
        let idx = (self.pos as isize + delta).clamp(0, last) as usize;
        &self.tokens[idx]
    }

    fn cur(&self) -> &Token {
        self.peek(0)
    }

    fn consume(&mut self, cnt: usize) {
        self.pos += cnt;
    }

    fn error(&self, message: impl Into<String>) -> ParserError {
        self.error_at(0, message)
    }

    fn error_at(&self, delta: isize, message: impl Into<String>) -> ParserError {
        let token = self.peek(delta);
        ParserError {
            line: token.line,
            column: token.column,
            message: message.into(),
        }
    }

    fn expect(&mut self, ty: TokenType, message: &str) -> Result<(), ParserError> {
        if self.cur().ty != ty {
            return Err(self.error(message));
        }
        self.consume(1);
        Ok(())
    }

    fn cur_str(&self) -> String {
        self.cur().value.as_str().unwrap_or_default().to_owned()
    }

    fn parse_identifier(&mut self) -> Result<String, ParserError> {
        if self.cur().ty != TokenType::Identifier {
            return Err(self.error("Expect identifier"));
        }
        let id = self.cur_str();
        self.consume(1);
        Ok(id)
    }

    fn parse_char_class(&mut self) -> Result<i32, ParserError> {
        self.expect(TokenType::LBracket, "Expect [ in character class")?;

        let mut elements = Vec::new();
        let mut is_negated = false;
        if self.cur().ty == TokenType::Caret {
            is_negated = true;
            self.consume(1);
        }

        while self.cur().ty != TokenType::RBracket && self.cur().ty != TokenType::EndOfFile {
            if self.cur().ty == TokenType::EscapeInCharClass {
                return Err(self.error("Character class escape is not supported yet in EBNF"));
            }
            let lower = match self.cur().ty {
                TokenType::CharInCharClass => self.cur().value.as_codepoint().unwrap_or(0),
                TokenType::Dash => i32::from(b'-'),
                _ => {
                    return Err(self.error(format!(
                        "Unexpected character in character class: {}",
                        self.cur().lexeme
                    )));
                }
            };
            self.consume(1);

            let is_range = self.cur().ty == TokenType::Dash
                && matches!(
                    self.peek(1).ty,
                    TokenType::CharInCharClass | TokenType::Dash
                );
            if is_range {
                let upper = match self.peek(1).ty {
                    TokenType::CharInCharClass => self.peek(1).value.as_codepoint().unwrap_or(0),
                    _ => i32::from(b'-'),
                };
                if lower > upper {
                    return Err(self.error_at(
                        -1,
                        "Invalid character class: lower bound is larger than upper bound",
                    ));
                }
                elements.push(CharacterClassElement::new(lower, upper));
                self.consume(2);
            } else {
                elements.push(CharacterClassElement::new(lower, lower));
            }
        }

        self.expect(TokenType::RBracket, "Expect ] in character class")?;
        Ok(self.builder.add_character_class(&elements, is_negated))
    }

    fn parse_string(&mut self) -> Result<i32, ParserError> {
        if self.cur().ty != TokenType::StringLiteral {
            return Err(self.error("Expect string literal"));
        }
        let value = self.cur_str();
        self.consume(1);
        if value.is_empty() {
            Ok(self.builder.add_empty_str())
        } else {
            Ok(self.builder.add_byte_string(&value))
        }
    }

    fn parse_rule_ref(&mut self) -> Result<i32, ParserError> {
        let name = self.parse_identifier()?;
        let rule_id = self.builder.get_rule_id(&name);
        if rule_id == NO_EXPR {
            return Err(self.error_at(-1, format!("Rule \"{name}\" is not defined")));
        }
        Ok(self.builder.add_rule_ref(rule_id))
    }

    fn parse_element(&mut self) -> Result<i32, ParserError> {
        match self.cur().ty {
            TokenType::LParen => {
                self.nest_layer_guard += 1;
                if self.nest_layer_guard > MAX_NEST_LAYER {
                    return Err(self.error_at(-1, "Nest layer exceeded the maximum limit"));
                }
                self.consume(1);
                if self.cur().ty == TokenType::RParen {
                    self.consume(1);
                    self.nest_layer_guard -= 1;
                    return Ok(self.builder.add_empty_str());
                }
                let expr = self.parse_choices()?;
                self.expect(TokenType::RParen, "Expect )")?;
                self.nest_layer_guard -= 1;
                Ok(expr)
            }
            TokenType::LBracket => self.parse_char_class(),
            TokenType::StringLiteral => self.parse_string(),
            TokenType::Identifier => {
                let id = self.cur_str();
                if MACRO_NAMES.contains(&id.as_str()) {
                    Err(self.error(format!("macro \"{id}\" is not yet supported")))
                } else {
                    self.parse_rule_ref()
                }
            }
            _ => Err(self.error(format!("Expect element, but got {}", self.cur().lexeme))),
        }
    }

    fn parse_integer(&mut self) -> Result<i64, ParserError> {
        if self.cur().ty != TokenType::IntegerLiteral {
            return Err(self.error(format!("Expect integer, but got {}", self.cur().lexeme)));
        }
        let num = self.cur().value.as_int().unwrap_or(0);
        self.consume(1);
        Ok(num)
    }

    /// Parses `{m}`, `{m,}`, `{m,n}`, or the printer's `{m, -1}` (unbounded), returning
    /// `(lower, upper)` with `upper == -1` meaning unbounded.
    fn parse_repetition_range(&mut self) -> Result<(i64, i64), ParserError> {
        self.expect(TokenType::LBrace, "Expect {")?;
        let lower = self.parse_integer()?;
        if lower < 0 {
            return Err(self.error_at(-1, "Lower bound cannot be negative"));
        }

        match self.cur().ty {
            TokenType::Comma => {
                self.consume(1);
                if self.cur().ty == TokenType::RBrace {
                    self.consume(1);
                    return Ok((lower, -1));
                }
                // The printer emits `{n, -1}` for unbounded upper bounds, and `-` is a
                // name char, so the lexer yields Identifier("-1"). Accept it as `{n,}`.
                if self.cur().ty == TokenType::Identifier && self.cur().lexeme == "-1" {
                    self.consume(1);
                    self.expect(TokenType::RBrace, "Expect }")?;
                    return Ok((lower, -1));
                }
                let upper = self.parse_integer()?;
                if upper < lower {
                    return Err(self.error_at(
                        -1,
                        format!("Lower bound is larger than upper bound: {lower} > {upper}"),
                    ));
                }
                self.expect(TokenType::RBrace, "Expect }")?;
                Ok((lower, upper))
            }
            TokenType::RBrace => {
                self.consume(1);
                Ok((lower, lower))
            }
            _ => Err(self.error("Expect ',' or '}' in repetition range")),
        }
    }

    fn handle_star_quantifier(&mut self, grammar_expr_id: i32) -> i32 {
        // A character-class star has a dedicated expr type: [a-z]*
        let char_class = {
            let expr = self.builder.grammar_expr(grammar_expr_id);
            (expr.ty == GrammarExprType::CharacterClass).then(|| expr.character_class())
        };
        if let Some((is_negative, ranges)) = char_class {
            return self.builder.add_character_class_star(&ranges, is_negative);
        }
        // Otherwise: a*  -->  rule ::= a rule | ""
        let new_rule_name = self.builder.get_new_rule_name(&self.cur_rule_name);
        let new_rule_id = self.builder.add_empty_rule(new_rule_name);
        let ref_to_new_rule = self.builder.add_rule_ref(new_rule_id);
        let empty = self.builder.add_empty_str();
        let seq = self.builder.add_sequence(&[grammar_expr_id, ref_to_new_rule]);
        let body = self.builder.add_choices(&[empty, seq]);
        self.builder.update_rule_body(new_rule_id, body);
        self.builder.add_rule_ref(new_rule_id)
    }

    fn handle_plus_quantifier(&mut self, grammar_expr_id: i32) -> i32 {
        // a+  -->  rule ::= a rule | a
        let new_rule_name = self.builder.get_new_rule_name(&self.cur_rule_name);
        let new_rule_id = self.builder.add_empty_rule(new_rule_name);
        let ref_to_new_rule = self.builder.add_rule_ref(new_rule_id);
        let seq = self.builder.add_sequence(&[grammar_expr_id, ref_to_new_rule]);
        let body = self.builder.add_choices(&[seq, grammar_expr_id]);
        self.builder.update_rule_body(new_rule_id, body);
        self.builder.add_rule_ref(new_rule_id)
    }

    fn handle_question_quantifier(&mut self, grammar_expr_id: i32) -> i32 {
        // a?  -->  rule ::= a | empty
        let new_rule_name = self.builder.get_new_rule_name(&self.cur_rule_name);
        let empty = self.builder.add_empty_str();
        let body = self.builder.add_choices(&[empty, grammar_expr_id]);
        let new_rule_id = self.builder.add_rule_named(new_rule_name, body);
        self.builder.add_rule_ref(new_rule_id)
    }

    fn parse_element_with_quantifier(&mut self) -> Result<i32, ParserError> {
        let grammar_expr_id = self.parse_element()?;
        Ok(match self.cur().ty {
            TokenType::Star => {
                self.consume(1);
                self.handle_star_quantifier(grammar_expr_id)
            }
            TokenType::Plus => {
                self.consume(1);
                self.handle_plus_quantifier(grammar_expr_id)
            }
            TokenType::Question => {
                self.consume(1);
                self.handle_question_quantifier(grammar_expr_id)
            }
            TokenType::LBrace => {
                let (lower, upper) = self.parse_repetition_range()?;
                let name = self.cur_rule_name.clone();
                self.builder.add_repeat_from_expr(
                    &name,
                    grammar_expr_id,
                    lower as i32,
                    if upper == -1 { -1 } else { upper as i32 },
                )
            }
            _ => grammar_expr_id,
        })
    }

    fn parse_sequence(&mut self) -> Result<i32, ParserError> {
        let mut elements = vec![self.parse_element_with_quantifier()?];
        while !matches!(
            self.cur().ty,
            TokenType::Pipe
                | TokenType::RParen
                | TokenType::LookaheadLParen
                | TokenType::RuleName
                | TokenType::EndOfFile
        ) {
            elements.push(self.parse_element_with_quantifier()?);
        }
        Ok(self.builder.add_sequence(&elements))
    }

    fn parse_choices(&mut self) -> Result<i32, ParserError> {
        let mut choices = vec![self.parse_sequence()?];
        while self.cur().ty == TokenType::Pipe {
            self.consume(1);
            choices.push(self.parse_sequence()?);
        }
        Ok(self.builder.add_choices(&choices))
    }

    fn parse_lookahead_assertion(&mut self) -> Result<i32, ParserError> {
        self.expect(TokenType::LookaheadLParen, "Expect (= in lookahead assertion")?;
        let result = self.parse_choices()?;
        self.expect(TokenType::RParen, "Expect )")?;
        Ok(result)
    }

    /// Parses one rule, returning `(name, body_expr_id, lookahead_assertion_id)`.
    fn parse_rule(&mut self) -> Result<(String, i32, i32), ParserError> {
        if self.cur().ty != TokenType::RuleName {
            return Err(self.error("Expect rule name"));
        }
        self.cur_rule_name = self.cur_str();
        self.consume(1);
        self.expect(TokenType::Assign, "Expect ::=")?;

        let body_id = self.parse_choices()?;
        let lookahead_id = if self.cur().ty == TokenType::LookaheadLParen {
            self.parse_lookahead_assertion()?
        } else {
            NO_EXPR
        };
        Ok((self.cur_rule_name.clone(), body_id, lookahead_id))
    }

    fn init_rule_names(&mut self) -> Result<(), ParserError> {
        for i in 0..self.tokens.len() {
            if self.tokens[i].ty == TokenType::RuleName {
                let name = self.tokens[i].value.as_str().unwrap_or_default().to_owned();
                if self.builder.get_rule_id(&name) != NO_EXPR {
                    return Err(self.error_at(
                        i as isize - self.pos as isize,
                        format!("Rule \"{name}\" is defined multiple times"),
                    ));
                }
                self.builder.add_empty_rule(name);
            }
        }
        if self.builder.get_rule_id(&self.root_rule_name) == NO_EXPR {
            return Err(self.error(format!(
                "The root rule with name \"{}\" is not found",
                self.root_rule_name
            )));
        }
        Ok(())
    }

    fn parse(mut self) -> Result<Grammar, ParserError> {
        self.init_rule_names()?;
        while self.cur().ty != TokenType::EndOfFile {
            let (name, body, lookahead) = self.parse_rule()?;
            self.builder.update_rule_body_by_name(&name, body);
            self.builder
                .update_lookahead_assertion_by_name(&name, lookahead);
        }
        let Parser {
            builder,
            root_rule_name,
            ..
        } = self;
        builder
            .into_grammar(&root_rule_name)
            .map_err(|message| ParserError {
                line: 0,
                column: 0,
                message,
            })
    }
}
