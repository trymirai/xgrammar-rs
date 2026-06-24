//! The EBNF parser — a port of `EBNFParser` in `cpp/grammar_parser.cc`.
//!
//! Produces the raw (un-normalized) BNF AST, including the four macros (`TagDispatch`,
//! `Token`, `ExcludeToken`, `TokenTagDispatch`).

use super::{
    ebnf_error::EbnfError,
    ebnf_lexer::tokenize,
    macro_ir::{MacroArguments, MacroValue},
    parse_error::ParserError,
    token::Token,
    token_type::TokenType,
};
use crate::grammar::{
    CharacterClassElement, Grammar, GrammarBuilder, GrammarExprType, NO_EXPR,
    TagDispatch, TokenTagDispatch,
};

const MAX_NEST_LAYER: i32 = 1000;

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
    fn new(
        tokens: Vec<Token>,
        root_rule_name: &str,
    ) -> Self {
        Self {
            builder: GrammarBuilder::new(),
            tokens,
            pos: 0,
            cur_rule_name: String::new(),
            root_rule_name: root_rule_name.to_owned(),
            nest_layer_guard: 0,
        }
    }

    fn peek(
        &self,
        delta: isize,
    ) -> &Token {
        let last = self.tokens.len() as isize - 1;
        let idx = (self.pos as isize + delta).clamp(0, last) as usize;
        &self.tokens[idx]
    }

    fn cur(&self) -> &Token {
        self.peek(0)
    }

    fn consume(
        &mut self,
        cnt: usize,
    ) {
        self.pos += cnt;
    }

    fn error(
        &self,
        message: impl Into<String>,
    ) -> ParserError {
        self.error_at(0, message)
    }

    fn error_at(
        &self,
        delta: isize,
        message: impl Into<String>,
    ) -> ParserError {
        let token = self.peek(delta);
        ParserError {
            line: token.line,
            column: token.column,
            message: message.into(),
        }
    }

    fn expect(
        &mut self,
        ty: TokenType,
        message: &str,
    ) -> Result<(), ParserError> {
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

        while self.cur().ty != TokenType::RBracket
            && self.cur().ty != TokenType::EndOfFile
        {
            if self.cur().ty == TokenType::EscapeInCharClass {
                return Err(self.error(
                    "Character class escape is not supported yet in EBNF",
                ));
            }
            let lower = match self.cur().ty {
                TokenType::CharInCharClass => {
                    self.cur().value.as_codepoint().unwrap_or(0)
                },
                TokenType::Dash => i32::from(b'-'),
                _ => {
                    return Err(self.error(format!(
                        "Unexpected character in character class: {}",
                        self.cur().lexeme
                    )));
                },
            };
            self.consume(1);

            let is_range = self.cur().ty == TokenType::Dash
                && matches!(
                    self.peek(1).ty,
                    TokenType::CharInCharClass | TokenType::Dash
                );
            if is_range {
                let upper = match self.peek(1).ty {
                    TokenType::CharInCharClass => {
                        self.peek(1).value.as_codepoint().unwrap_or(0)
                    },
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
            return Err(
                self.error_at(-1, format!("Rule \"{name}\" is not defined"))
            );
        }
        Ok(self.builder.add_rule_ref(rule_id))
    }

    /// Parses a single macro value: a string, integer, boolean, identifier, or tuple.
    fn parse_macro_value(&mut self) -> Result<MacroValue, ParserError> {
        match self.cur().ty {
            TokenType::StringLiteral => {
                let value = self.cur_str();
                self.consume(1);
                Ok(MacroValue::Str(value))
            },
            TokenType::IntegerLiteral => {
                let value = self.cur().value.as_int().unwrap_or(0);
                self.consume(1);
                Ok(MacroValue::Int(value))
            },
            TokenType::BooleanLiteral => {
                let value = self.cur().value.as_bool().unwrap_or(false);
                self.consume(1);
                Ok(MacroValue::Bool(value))
            },
            TokenType::Identifier => {
                let name = self.cur_str();
                self.consume(1);
                Ok(MacroValue::Identifier(name))
            },
            TokenType::LParen => {
                self.consume(1);
                let mut elements = Vec::new();
                if self.cur().ty != TokenType::RParen {
                    loop {
                        elements.push(self.parse_macro_value()?);
                        if self.cur().ty == TokenType::Comma {
                            self.consume(1);
                            if self.cur().ty == TokenType::RParen {
                                break;
                            }
                        } else if self.cur().ty == TokenType::RParen {
                            break;
                        } else {
                            return Err(self.error("Expect , or ) in tuple"));
                        }
                    }
                }
                self.consume(1); // Consume )
                Ok(MacroValue::Tuple(elements))
            },
            _ => Err(self.error(
                "Expect string, integer, boolean, or tuple in macro argument",
            )),
        }
    }

    /// Parses a macro argument list (positional and named arguments).
    fn parse_macro_arguments(&mut self) -> Result<MacroArguments, ParserError> {
        self.expect(TokenType::LParen, "Expect ( after macro function name")?;
        let mut args = MacroArguments::default();
        if self.cur().ty != TokenType::RParen {
            loop {
                if self.cur().ty == TokenType::Identifier
                    && self.peek(1).ty == TokenType::Equal
                {
                    let name = self.cur_str();
                    self.consume(2); // Consume identifier and =
                    let value = self.parse_macro_value()?;
                    args.named.push((name, value));
                } else {
                    args.positional.push(self.parse_macro_value()?);
                }
                if self.cur().ty == TokenType::Comma {
                    self.consume(1);
                } else if self.cur().ty == TokenType::RParen {
                    break;
                } else {
                    return Err(self.error("Expect , or ) in macro arguments"));
                }
            }
        }
        self.expect(TokenType::RParen, "Expect ) after macro arguments")?;
        Ok(args)
    }

    fn parse_tag_dispatch(&mut self) -> Result<i32, ParserError> {
        self.consume(1); // Consume TagDispatch operator
        let start = self.pos;
        let args = self.parse_macro_arguments()?;
        let delta = start as isize - self.pos as isize;

        for (name, _) in &args.named {
            if name != "loop_after_dispatch" && name != "excludes" {
                return Err(self.error_at(
                    delta,
                    format!("Unknown named argument for TagDispatch: {name}"),
                ));
            }
        }

        let mut tag_rule_pairs = Vec::new();
        for arg in &args.positional {
            let MacroValue::Tuple(elements) = arg else {
                return Err(self.error_at(
                    delta,
                    "Each tag dispatch element must be a tuple",
                ));
            };
            if elements.len() != 2 {
                return Err(self.error_at(
                    delta,
                    "Each tag dispatch element must be a pair (tag, rule)",
                ));
            }
            let MacroValue::Str(tag) = &elements[0] else {
                return Err(self.error_at(
                    delta,
                    "Tag must be a non-empty string literal",
                ));
            };
            if tag.is_empty() {
                return Err(self.error_at(
                    delta,
                    "Tag must be a non-empty string literal",
                ));
            }
            let MacroValue::Identifier(rule_name) = &elements[1] else {
                return Err(self
                    .error_at(delta, "Rule reference must be an identifier"));
            };
            let rule_id = self.builder.get_rule_id(rule_name);
            if rule_id == NO_EXPR {
                return Err(self.error_at(
                    delta,
                    format!("Rule \"{rule_name}\" is not defined"),
                ));
            }
            tag_rule_pairs.push((tag.clone().into_bytes(), rule_id));
        }

        let mut loop_after_dispatch = true;
        if let Some(value) = args.named("loop_after_dispatch") {
            let MacroValue::Bool(b) = value else {
                return Err(self.error_at(
                    delta,
                    "loop_after_dispatch must be a boolean literal",
                ));
            };
            loop_after_dispatch = *b;
        }

        let mut excludes = Vec::new();
        if let Some(value) = args.named("excludes") {
            let MacroValue::Tuple(elements) = value else {
                return Err(self.error_at(delta, "excludes must be a tuple"));
            };
            for element in elements {
                let MacroValue::Str(s) = element else {
                    return Err(self.error_at(
                        delta,
                        "Exclude must be a non-empty string literal",
                    ));
                };
                if s.is_empty() {
                    return Err(self.error_at(
                        delta,
                        "Exclude must be a non-empty string literal",
                    ));
                }
                excludes.push(s.clone().into_bytes());
            }
        }

        for exclude in &excludes {
            for (trigger, _) in &tag_rule_pairs {
                if trigger.starts_with(exclude.as_slice()) {
                    return Err(self.error_at(
                        delta,
                        format!(
                            "Exclude string must not be a prefix of trigger string: {}",
                            String::from_utf8_lossy(exclude)
                        ),
                    ));
                }
            }
        }

        let tag_dispatch = TagDispatch {
            tag_rule_pairs,
            loop_after_dispatch,
            excludes,
        };
        Ok(self.builder.add_tag_dispatch(&tag_dispatch))
    }

    fn parse_token_set(&mut self) -> Result<i32, ParserError> {
        self.consume(1); // Consume Token identifier
        let start = self.pos;
        let args = self.parse_macro_arguments()?;
        let delta = start as isize - self.pos as isize;

        if !args.named.is_empty() {
            return Err(
                self.error_at(delta, "Token() does not accept named arguments")
            );
        }
        if args.positional.is_empty() {
            return Err(self.error_at(
                delta,
                "Token() requires at least one integer argument",
            ));
        }
        let mut token_ids = Vec::with_capacity(args.positional.len());
        for arg in &args.positional {
            let MacroValue::Int(value) = arg else {
                return Err(self.error_at(
                    delta,
                    "Token() arguments must be non-negative integers",
                ));
            };
            if *value < 0 {
                return Err(self.error_at(
                    delta,
                    "Token() arguments must be non-negative integers",
                ));
            }
            token_ids.push(*value as i32);
        }
        token_ids.sort_unstable();
        token_ids.dedup();
        Ok(self.builder.add_token_set(&token_ids))
    }

    fn parse_exclude_token(&mut self) -> Result<i32, ParserError> {
        self.consume(1); // Consume ExcludeToken identifier
        let start = self.pos;
        let args = self.parse_macro_arguments()?;
        let delta = start as isize - self.pos as isize;

        if !args.named.is_empty() {
            return Err(self.error_at(
                delta,
                "ExcludeToken() does not accept named arguments",
            ));
        }
        if args.positional.is_empty() {
            return Err(self.error_at(
                delta,
                "ExcludeToken() requires at least one integer argument",
            ));
        }
        let mut token_ids = Vec::with_capacity(args.positional.len());
        for arg in &args.positional {
            let MacroValue::Int(value) = arg else {
                return Err(self.error_at(
                    delta,
                    "ExcludeToken() arguments must be non-negative integers",
                ));
            };
            if *value < 0 {
                return Err(self.error_at(
                    delta,
                    "ExcludeToken() arguments must be non-negative integers",
                ));
            }
            token_ids.push(*value as i32);
        }
        token_ids.sort_unstable();
        token_ids.dedup();
        Ok(self.builder.add_exclude_token_set(&token_ids))
    }

    fn parse_token_tag_dispatch(&mut self) -> Result<i32, ParserError> {
        self.consume(1); // Consume TokenTagDispatch identifier
        let start = self.pos;
        let args = self.parse_macro_arguments()?;
        let delta = start as isize - self.pos as isize;

        for (name, _) in &args.named {
            if name != "loop_after_dispatch" && name != "excludes" {
                return Err(self.error_at(
                    delta,
                    format!(
                        "Unknown named argument for TokenTagDispatch: {name}"
                    ),
                ));
            }
        }

        let mut trigger_rule_pairs = Vec::new();
        for arg in &args.positional {
            let MacroValue::Tuple(elements) = arg else {
                return Err(self.error_at(
                    delta,
                    "Each TokenTagDispatch element must be a pair (token_id, rule)",
                ));
            };
            if elements.len() != 2 {
                return Err(self.error_at(
                    delta,
                    "Each TokenTagDispatch element must be a pair (token_id, rule)",
                ));
            }
            let MacroValue::Int(token_id) = &elements[0] else {
                return Err(self.error_at(
                    delta,
                    "Token trigger ID must be a non-negative integer",
                ));
            };
            if *token_id < 0 {
                return Err(self.error_at(
                    delta,
                    "Token trigger ID must be a non-negative integer",
                ));
            }
            let MacroValue::Identifier(rule_name) = &elements[1] else {
                return Err(self
                    .error_at(delta, "Rule reference must be an identifier"));
            };
            let rule_id = self.builder.get_rule_id(rule_name);
            if rule_id == NO_EXPR {
                return Err(self.error_at(
                    delta,
                    format!("Rule \"{rule_name}\" is not defined"),
                ));
            }
            trigger_rule_pairs.push((*token_id as i32, rule_id));
        }

        let mut loop_after_dispatch = true;
        if let Some(value) = args.named("loop_after_dispatch") {
            let MacroValue::Bool(b) = value else {
                return Err(self
                    .error_at(delta, "loop_after_dispatch must be a boolean"));
            };
            loop_after_dispatch = *b;
        }

        let mut excludes = Vec::new();
        if let Some(value) = args.named("excludes") {
            let MacroValue::Tuple(elements) = value else {
                return Err(self.error_at(delta, "excludes must be a tuple"));
            };
            for element in elements {
                let MacroValue::Int(token_id) = element else {
                    return Err(self.error_at(
                        delta,
                        "Exclude token ID must be a non-negative integer",
                    ));
                };
                if *token_id < 0 {
                    return Err(self.error_at(
                        delta,
                        "Exclude token ID must be a non-negative integer",
                    ));
                }
                excludes.push(*token_id as i32);
            }
        }

        for &exclude_id in &excludes {
            for (token_id, _) in &trigger_rule_pairs {
                if *token_id == exclude_id {
                    return Err(self.error_at(
                        delta,
                        format!(
                            "Token trigger ID {token_id} must not overlap with exclude token ID"
                        ),
                    ));
                }
            }
        }

        let ttd = TokenTagDispatch {
            trigger_rule_pairs,
            loop_after_dispatch,
            excludes,
        };
        Ok(self.builder.add_token_tag_dispatch(&ttd))
    }

    fn parse_element(&mut self) -> Result<i32, ParserError> {
        match self.cur().ty {
            TokenType::LParen => {
                self.nest_layer_guard += 1;
                if self.nest_layer_guard > MAX_NEST_LAYER {
                    return Err(self.error_at(
                        -1,
                        "Nest layer exceeded the maximum limit",
                    ));
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
            },
            TokenType::LBracket => self.parse_char_class(),
            TokenType::StringLiteral => self.parse_string(),
            TokenType::Identifier => match self.cur_str().as_str() {
                "TagDispatch" => self.parse_tag_dispatch(),
                "Token" => self.parse_token_set(),
                "ExcludeToken" => self.parse_exclude_token(),
                "TokenTagDispatch" => self.parse_token_tag_dispatch(),
                _ => self.parse_rule_ref(),
            },
            _ => Err(self.error(format!(
                "Expect element, but got {}",
                self.cur().lexeme
            ))),
        }
    }

    fn parse_integer(&mut self) -> Result<i64, ParserError> {
        if self.cur().ty != TokenType::IntegerLiteral {
            return Err(self.error(format!(
                "Expect integer, but got {}",
                self.cur().lexeme
            )));
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
                if self.cur().ty == TokenType::Identifier
                    && self.cur().lexeme == "-1"
                {
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
            },
            TokenType::RBrace => {
                self.consume(1);
                Ok((lower, lower))
            },
            _ => Err(self.error("Expect ',' or '}' in repetition range")),
        }
    }

    fn handle_star_quantifier(
        &mut self,
        grammar_expr_id: i32,
    ) -> i32 {
        // A character-class star has a dedicated expr type: [a-z]*
        let char_class = {
            let expr = self.builder.grammar_expr(grammar_expr_id);
            (expr.ty == GrammarExprType::CharacterClass)
                .then(|| expr.character_class())
        };
        if let Some((is_negative, ranges)) = char_class {
            return self.builder.add_character_class_star(&ranges, is_negative);
        }
        // Otherwise: a*  -->  rule ::= a rule | ""
        let new_rule_name = self.builder.get_new_rule_name(&self.cur_rule_name);
        let new_rule_id = self.builder.add_empty_rule(new_rule_name);
        let ref_to_new_rule = self.builder.add_rule_ref(new_rule_id);
        let empty = self.builder.add_empty_str();
        let seq =
            self.builder.add_sequence(&[grammar_expr_id, ref_to_new_rule]);
        let body = self.builder.add_choices(&[empty, seq]);
        self.builder.update_rule_body(new_rule_id, body);
        self.builder.add_rule_ref(new_rule_id)
    }

    fn handle_plus_quantifier(
        &mut self,
        grammar_expr_id: i32,
    ) -> i32 {
        // a+  -->  rule ::= a rule | a
        let new_rule_name = self.builder.get_new_rule_name(&self.cur_rule_name);
        let new_rule_id = self.builder.add_empty_rule(new_rule_name);
        let ref_to_new_rule = self.builder.add_rule_ref(new_rule_id);
        let seq =
            self.builder.add_sequence(&[grammar_expr_id, ref_to_new_rule]);
        let body = self.builder.add_choices(&[seq, grammar_expr_id]);
        self.builder.update_rule_body(new_rule_id, body);
        self.builder.add_rule_ref(new_rule_id)
    }

    fn handle_question_quantifier(
        &mut self,
        grammar_expr_id: i32,
    ) -> i32 {
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
            },
            TokenType::Plus => {
                self.consume(1);
                self.handle_plus_quantifier(grammar_expr_id)
            },
            TokenType::Question => {
                self.consume(1);
                self.handle_question_quantifier(grammar_expr_id)
            },
            TokenType::LBrace => {
                let (lower, upper) = self.parse_repetition_range()?;
                let name = self.cur_rule_name.clone();
                self.builder.add_repeat_from_expr(
                    &name,
                    grammar_expr_id,
                    lower as i32,
                    if upper == -1 {
                        -1
                    } else {
                        upper as i32
                    },
                )
            },
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
        self.expect(
            TokenType::LookaheadLParen,
            "Expect (= in lookahead assertion",
        )?;
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
                let name = self.tokens[i]
                    .value
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
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
            self.builder.update_lookahead_assertion_by_name(&name, lookahead);
        }
        let Parser {
            builder,
            root_rule_name,
            ..
        } = self;
        builder.into_grammar(&root_rule_name).map_err(|message| ParserError {
            line: 0,
            column: 0,
            message,
        })
    }
}
