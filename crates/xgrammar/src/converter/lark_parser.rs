//! Recursive-descent parser for the Lark grammar syntax.

use std::rc::Rc;

use serde_json::Value;

use super::{
    lark_definition::LarkDefinition, lark_document::LarkDocument, lark_error::LarkError, lark_import::LarkImport,
    lark_lexer::LarkLexer, lark_location::LarkLocation, lark_node::LarkNode, lark_node_kind::LarkNodeKind,
    lark_token::LarkToken, lark_token_type::LarkTokenType,
};
use crate::support::parse_json;

/// Groups recurse `parse_choice -> parse_sequence -> parse_expr -> parse_atom` once per level, so
/// the nesting depth is bounded instead of overflowing the stack.
const MAX_GROUP_DEPTH: i32 = 200;

/// Parses Lark tokens into a [`LarkDocument`].
#[derive(Debug)]
pub struct LarkParser<'a> {
    source: &'a str,
    tokens: Vec<LarkToken>,
    position: usize,
    group_depth: i32,
}

impl<'a> LarkParser<'a> {
    /// Tokenizes and parses a whole Lark source.
    ///
    /// # Errors
    /// Returns a located [`LarkError`] on a lexing or syntax error.
    pub fn parse(source: &'a str) -> Result<LarkDocument, LarkError> {
        let tokens = LarkLexer::new(source).tokenize()?;
        Self {
            source,
            tokens,
            position: 0,
            group_depth: 0,
        }
        .parse_document(false)
    }

    fn error(
        &self,
        location: LarkLocation,
        message: impl Into<String>,
    ) -> LarkError {
        LarkError::at(self.source, location, message)
    }

    fn peek_at(
        &self,
        offset: usize,
    ) -> &LarkToken {
        &self.tokens[(self.position + offset).min(self.tokens.len() - 1)]
    }

    fn peek(&self) -> &LarkToken {
        self.peek_at(0)
    }

    fn peek_type(&self) -> LarkTokenType {
        self.peek().ty
    }

    fn match_token(
        &mut self,
        ty: LarkTokenType,
    ) -> bool {
        if self.peek_type() != ty {
            return false;
        }
        self.position += 1;
        true
    }

    fn consume(
        &mut self,
        ty: LarkTokenType,
        message: &str,
    ) -> Result<LarkToken, LarkError> {
        if self.peek_type() != ty {
            return Err(self.error(self.peek().location, message));
        }
        self.position += 1;
        Ok(self.tokens[self.position - 1].clone())
    }

    /// Fails with `message` at the next token if it has type `ty`.
    fn reject(
        &self,
        ty: LarkTokenType,
        message: &str,
    ) -> Result<(), LarkError> {
        if self.peek_type() == ty {
            return Err(self.error(self.peek().location, message));
        }
        Ok(())
    }

    fn consume_newlines(&mut self) {
        while self.match_token(LarkTokenType::Newline) {}
    }

    fn parse_document(
        &mut self,
        stop_at_rbrace: bool,
    ) -> Result<LarkDocument, LarkError> {
        let mut document = LarkDocument::default();
        let at_end = |ty: LarkTokenType| ty == LarkTokenType::End || (stop_at_rbrace && ty == LarkTokenType::RBrace);
        self.consume_newlines();
        while !at_end(self.peek_type()) {
            match self.peek_type() {
                LarkTokenType::Import => self.parse_import(&mut document)?,
                LarkTokenType::Ignore => {
                    self.position += 1;
                    let ignore = self.parse_choice()?;
                    document.ignores.push(ignore);
                },
                LarkTokenType::GrammarOptions => {
                    let token = self.consume(LarkTokenType::GrammarOptions, "expected %grammar_options")?;
                    let value = parse_json(&token.text).map_err(|error| {
                        self.error(token.location, format!("invalid %grammar_options value: {error}"))
                    })?;
                    document.options.push((value, token.location));
                },
                LarkTokenType::UnsupportedDirective => {
                    return Err(
                        self.error(self.peek().location, format!("directive {} is not supported", self.peek().text))
                    );
                },
                _ => {
                    let definition = self.parse_definition()?;
                    document.definitions.push(definition);
                },
            }
            if self.peek_type() != LarkTokenType::Newline && !at_end(self.peek_type()) {
                return Err(self.error(self.peek().location, "expected end of grammar item"));
            }
            self.consume_newlines();
        }
        Ok(document)
    }

    fn is_terminal_name(raw_name: &str) -> bool {
        let name = raw_name.strip_prefix(['!', '?']).unwrap_or(raw_name);
        let name = name.strip_prefix('_').unwrap_or(name);
        name.bytes().next().is_some_and(|c| c.is_ascii_uppercase())
    }

    fn normalize_rule_name(name: &str) -> String {
        name.strip_prefix(['!', '?']).unwrap_or(name).to_owned()
    }

    fn validate_capture_name(
        &self,
        capture_name: &str,
        location: LarkLocation,
    ) -> Result<(), LarkError> {
        if capture_name.is_empty() {
            return Err(self.error(location, "capture name must not be empty"));
        }
        if !capture_name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.')) {
            return Err(self.error(location, "capture name must only contain letters, digits, '_', '-' and '.'"));
        }
        Ok(())
    }

    fn parse_stop_like_value(
        &mut self,
        attribute_name: &str,
    ) -> Result<LarkNode, LarkError> {
        let token = self.peek().clone();
        match token.ty {
            LarkTokenType::String => {
                self.position += 1;
                self.parse_string_node(&token)
            },
            LarkTokenType::Regex => {
                self.position += 1;
                Ok(Self::regex_node(token))
            },
            LarkTokenType::Name => {
                self.position += 1;
                if !Self::is_terminal_name(&token.text) {
                    return Err(self.error(token.location, format!("{attribute_name} terminal name must be uppercase")));
                }
                Ok(LarkNode::with_text(LarkNodeKind::Name, token.location, Self::normalize_rule_name(&token.text)))
            },
            _ => Err(self.error(
                token.location,
                format!(
                    "expected string literal, regular expression, or uppercase terminal name after {attribute_name}="
                ),
            )),
        }
    }

    fn regex_node(token: LarkToken) -> LarkNode {
        LarkNode {
            flags: token.flags,
            ..LarkNode::with_text(LarkNodeKind::Regex, token.location, token.text)
        }
    }

    fn parse_import(
        &mut self,
        document: &mut LarkDocument,
    ) -> Result<(), LarkError> {
        let location = self.consume(LarkTokenType::Import, "expected %import")?.location;
        let mut path = self.consume(LarkTokenType::Name, "expected import path")?.text;
        while self.match_token(LarkTokenType::Dot) {
            path.push('.');
            path.push_str(&self.consume(LarkTokenType::Name, "expected name after '.'")?.text);
        }

        if self.match_token(LarkTokenType::LParen) {
            loop {
                let name = self.consume(LarkTokenType::Name, "expected imported terminal name")?;
                document.imports.push(LarkImport {
                    path: format!("{path}.{}", name.text),
                    local_name: name.text,
                    location,
                });
                if !self.match_token(LarkTokenType::Comma) {
                    break;
                }
            }
            self.consume(LarkTokenType::RParen, "expected ')' after import list")?;
            return Ok(());
        }

        let mut local_name = path[path.rfind('.').map_or(0, |dot| dot + 1)..].to_owned();
        if self.match_token(LarkTokenType::Arrow) {
            local_name = self.consume(LarkTokenType::Name, "expected import alias")?.text;
        }
        document.imports.push(LarkImport {
            path,
            local_name,
            location,
        });
        Ok(())
    }

    fn parse_definition(&mut self) -> Result<LarkDefinition, LarkError> {
        let name_token = self.consume(LarkTokenType::Name, "expected rule or terminal name")?;
        let mut definition = LarkDefinition::new(
            Self::normalize_rule_name(&name_token.text),
            Self::is_terminal_name(&name_token.text),
            LarkNode::new(LarkNodeKind::Sequence, name_token.location),
            name_token.location,
        );

        if self.peek_type() == LarkTokenType::LBracket {
            if definition.is_terminal {
                return Err(self.error(self.peek().location, "attributes are only supported on rules"));
            }
            self.parse_attributes(&mut definition)?;
        }
        self.reject(LarkTokenType::Dot, "rule and terminal priorities are not supported")?;
        self.reject(LarkTokenType::DoubleColon, "parametric grammar is not supported")?;
        self.reject(LarkTokenType::LBrace, "Lark templates are not supported")?;
        self.consume(LarkTokenType::Colon, "expected ':' after rule name")?;
        definition.body = self.parse_choice()?;
        Ok(definition)
    }

    /// Parses a string-literal attribute value that must be a valid capture name.
    fn parse_capture_name_value(
        &mut self,
        attribute_name: &str,
    ) -> Result<(String, LarkLocation), LarkError> {
        let token = self.consume(LarkTokenType::String, &format!("expected string literal after {attribute_name}="))?;
        let node = self.parse_string_node(&token)?;
        if !node.flags.is_empty() {
            return Err(
                self.error(node.location, format!("case-insensitive flags are not supported on {attribute_name}"))
            );
        }
        Ok((node.text, node.location))
    }

    /// Parses a `suffix=` or `stop=` value; `other` is the attribute it cannot be combined with.
    fn parse_marker_attribute(
        &mut self,
        key: &LarkToken,
        current: Option<&LarkNode>,
        other: Option<&LarkNode>,
        other_name: &str,
    ) -> Result<LarkNode, LarkError> {
        let name = key.text.as_str();
        self.consume(LarkTokenType::Equals, &format!("expected '=' after {name} attribute"))?;
        let marker = self.parse_stop_like_value(name)?;
        if marker.kind == LarkNodeKind::String && marker.text.is_empty() {
            return Err(self.error(marker.location, format!("{name} must not be empty")));
        }
        if current.is_some() {
            return Err(self.error(key.location, format!("{name} attribute is specified more than once")));
        }
        if other.is_some() {
            return Err(self.error(key.location, format!("{name} cannot be combined with {other_name}")));
        }
        Ok(marker)
    }

    fn parse_attributes(
        &mut self,
        definition: &mut LarkDefinition,
    ) -> Result<(), LarkError> {
        self.consume(LarkTokenType::LBracket, "expected '['")?;
        let mut stop_capture_location = None;
        while self.peek_type() != LarkTokenType::RBracket {
            let key = self.consume(LarkTokenType::Name, "expected rule attribute")?;
            let duplicate = |parser: &Self| {
                parser.error(key.location, format!("{} attribute is specified more than once", key.text))
            };
            match key.text.as_str() {
                "lazy" if self.peek_type() != LarkTokenType::Equals => definition.lazy = true,
                "max_tokens" => {
                    self.consume(LarkTokenType::Equals, "expected '=' after max_tokens attribute")?;
                    let value_location = self.peek().location;
                    let value = self.parse_integer()?;
                    if value <= 0 {
                        return Err(self.error(value_location, "max_tokens must be positive"));
                    }
                    if value > 1_000_000 {
                        return Err(self.error(value_location, "max_tokens is too large"));
                    }
                    if definition.max_tokens.is_some() {
                        return Err(duplicate(self));
                    }
                    definition.max_tokens = Some((value, key.location));
                },
                "max_chars" => {
                    self.consume(LarkTokenType::Equals, "expected '=' after max_chars attribute")?;
                    let value = self.parse_integer()?;
                    if definition.max_chars.is_some() {
                        return Err(duplicate(self));
                    }
                    definition.max_chars = Some(value);
                },
                "capture" => {
                    let (capture_name, capture_location) = if self.match_token(LarkTokenType::Equals) {
                        self.parse_capture_name_value("capture")?
                    } else {
                        (definition.name.clone(), key.location)
                    };
                    self.validate_capture_name(&capture_name, capture_location)?;
                    if definition.capture_name.is_some() {
                        return Err(duplicate(self));
                    }
                    definition.capture_name = Some((capture_name, capture_location));
                },
                "suffix" => {
                    let (current, other) = (definition.suffix.as_ref(), definition.stop.as_ref());
                    let suffix = self.parse_marker_attribute(&key, current, other, "stop")?;
                    definition.suffix = Some(suffix);
                },
                "stop" => {
                    let (current, other) = (definition.stop.as_ref(), definition.suffix.as_ref());
                    let stop = self.parse_marker_attribute(&key, current, other, "suffix")?;
                    definition.stop = Some(stop);
                },
                "stop_capture" => {
                    self.consume(LarkTokenType::Equals, "expected '=' after stop_capture attribute")?;
                    let (name, location) = self.parse_capture_name_value("stop_capture")?;
                    self.validate_capture_name(&name, location)?;
                    if definition.stop_capture_name.is_some() {
                        return Err(duplicate(self));
                    }
                    definition.stop_capture_name = Some(name);
                    stop_capture_location = Some(location);
                },
                "temperature" => {
                    self.consume(LarkTokenType::Equals, "expected '=' after temperature attribute")?;
                    let value = self.consume(LarkTokenType::Number, "expected number after temperature=")?;
                    if definition.temperature.is_some() {
                        return Err(duplicate(self));
                    }
                    match value.text.parse::<f32>() {
                        Ok(temperature) if temperature.is_finite() && temperature >= 0.0 => {
                            definition.temperature = Some(temperature);
                        },
                        _ => {
                            return Err(self.error(value.location, "temperature must be a finite non-negative number"));
                        },
                    }
                },
                _ => {
                    return Err(self.error(
                        key.location,
                        format!("rule attribute '{}' is not supported by XGrammar Lark", key.text),
                    ));
                },
            }
            if !self.match_token(LarkTokenType::Comma) {
                break;
            }
        }
        self.consume(LarkTokenType::RBracket, "expected ']' after rule attributes")?;
        match stop_capture_location {
            Some(location) if definition.suffix.is_none() && definition.stop.is_none() => {
                Err(self.error(location, "stop_capture requires stop or suffix"))
            },
            _ => Ok(()),
        }
    }

    fn parse_choice(&mut self) -> Result<LarkNode, LarkError> {
        let location = self.peek().location;
        let mut alternatives = vec![self.parse_sequence()?];
        while self.match_alternative_separator() {
            alternatives.push(self.parse_sequence()?);
        }
        if alternatives.len() == 1 {
            return Ok(alternatives.swap_remove(0));
        }
        Ok(LarkNode::with_children(LarkNodeKind::Choice, location, alternatives))
    }

    fn match_alternative_separator(&mut self) -> bool {
        if self.match_token(LarkTokenType::Pipe) {
            return true;
        }
        let saved_position = self.position;
        self.consume_newlines();
        if self.match_token(LarkTokenType::Pipe) {
            return true;
        }
        if matches!(self.peek_type(), LarkTokenType::RParen | LarkTokenType::RBracket | LarkTokenType::RBrace) {
            return false;
        }
        self.position = saved_position;
        false
    }

    fn parse_sequence(&mut self) -> Result<LarkNode, LarkError> {
        let location = self.peek().location;
        let mut elements = Vec::new();
        while !matches!(
            self.peek_type(),
            LarkTokenType::Newline
                | LarkTokenType::Pipe
                | LarkTokenType::RParen
                | LarkTokenType::RBracket
                | LarkTokenType::RBrace
                | LarkTokenType::End
                | LarkTokenType::Arrow
                | LarkTokenType::And
                | LarkTokenType::If
        ) {
            elements.push(self.parse_expr()?);
        }
        if self.match_token(LarkTokenType::Arrow) {
            self.consume(LarkTokenType::Name, "expected alias name after '->'")?;
        }
        self.reject(LarkTokenType::And, "terminal intersection '&' is not supported")?;
        self.reject(LarkTokenType::If, "parametric %if conditions are not supported")?;
        Ok(LarkNode::with_children(LarkNodeKind::Sequence, location, elements))
    }

    fn parse_integer(&mut self) -> Result<i32, LarkError> {
        let token = self.consume(LarkTokenType::Number, "expected integer")?;
        token
            .text
            .parse::<i64>()
            .ok()
            .and_then(|value| i32::try_from(value).ok())
            .filter(|&value| value >= 0)
            .ok_or_else(|| self.error(token.location, "invalid repetition count"))
    }

    fn parse_expr(&mut self) -> Result<LarkNode, LarkError> {
        let location = self.peek().location;
        let negated = self.match_token(LarkTokenType::Tilde);
        let mut atom = self.parse_atom()?;
        if negated {
            atom = LarkNode::with_children(LarkNodeKind::Not, location, vec![atom]);
        }

        let (min_repeat, max_repeat) = if self.match_token(LarkTokenType::Question) {
            (0, 1)
        } else if self.match_token(LarkTokenType::Star) {
            (0, -1)
        } else if self.match_token(LarkTokenType::Plus) {
            (1, -1)
        } else if self.match_token(LarkTokenType::Tilde) {
            let min_repeat = self.parse_integer()?;
            let max_repeat = if self.match_token(LarkTokenType::DotDot) {
                self.parse_integer()?
            } else {
                min_repeat
            };
            (min_repeat, max_repeat)
        } else if self.match_token(LarkTokenType::LBrace) {
            let min_repeat = if self.peek_type() == LarkTokenType::Comma {
                0
            } else {
                self.parse_integer()?
            };
            let max_repeat = if !self.match_token(LarkTokenType::Comma) {
                min_repeat
            } else if self.peek_type() == LarkTokenType::RBrace {
                -1
            } else {
                self.parse_integer()?
            };
            self.consume(LarkTokenType::RBrace, "expected '}' after repetition range")?;
            (min_repeat, max_repeat)
        } else {
            return Ok(atom);
        };

        if max_repeat != -1 && max_repeat < min_repeat {
            return Err(self.error(location, "repetition end must be greater than or equal to start"));
        }
        Ok(LarkNode::repeat(location, atom, min_repeat, max_repeat))
    }

    fn parse_group_body(
        &mut self,
        open: &LarkToken,
    ) -> Result<LarkNode, LarkError> {
        if self.group_depth >= MAX_GROUP_DEPTH {
            return Err(self.error(open.location, format!("groups are nested deeper than {MAX_GROUP_DEPTH} levels")));
        }
        self.group_depth += 1;
        let result = self.parse_choice();
        self.group_depth -= 1;
        result
    }

    fn parse_atom(&mut self) -> Result<LarkNode, LarkError> {
        let token = self.peek().clone();
        self.position += 1;
        let location = token.location;
        match token.ty {
            LarkTokenType::LParen => {
                let result = self.parse_group_body(&token)?;
                self.consume(LarkTokenType::RParen, "expected ')' after group")?;
                Ok(result)
            },
            LarkTokenType::LBracket => {
                let inner = self.parse_group_body(&token)?;
                self.consume(LarkTokenType::RBracket, "expected ']' after optional group")?;
                Ok(LarkNode::repeat(location, inner, 0, 1))
            },
            LarkTokenType::String => {
                let result = self.parse_string_node(&token)?;
                if !self.match_token(LarkTokenType::DotDot) {
                    return Ok(result);
                }
                let end = self.consume(LarkTokenType::String, "expected string after '..'")?;
                let end_node = self.parse_string_node(&end)?;
                if !result.flags.is_empty() {
                    return Err(self.error(location, "flags are not allowed on character ranges"));
                }
                if !end_node.flags.is_empty() {
                    return Err(self.error(end.location, "flags are not allowed on character ranges"));
                }
                Ok(LarkNode {
                    text2: end_node.text,
                    ..LarkNode::with_text(LarkNodeKind::Range, location, result.text)
                })
            },
            LarkTokenType::Regex => Ok(Self::regex_node(token)),
            LarkTokenType::Name => {
                self.reject(LarkTokenType::DoubleColon, "parametric grammar is not supported")?;
                if self.peek_type() == LarkTokenType::LBrace
                    && !matches!(self.peek_at(1).ty, LarkTokenType::Comma | LarkTokenType::Number)
                {
                    return Err(self.error(self.peek().location, "Lark templates are not supported"));
                }
                Ok(LarkNode::with_text(LarkNodeKind::Name, location, Self::normalize_rule_name(&token.text)))
            },
            LarkTokenType::Json => Ok(LarkNode::with_text(LarkNodeKind::Json, location, token.text)),
            LarkTokenType::RegexExt => Ok(LarkNode::with_text(LarkNodeKind::RegexExt, location, token.text)),
            LarkTokenType::SpecialToken => Ok(LarkNode::with_text(LarkNodeKind::SpecialToken, location, token.text)),
            LarkTokenType::GrammarRef => Ok(LarkNode::with_text(LarkNodeKind::GrammarRef, location, token.text)),
            LarkTokenType::Lark => {
                self.consume(LarkTokenType::LBrace, "expected '{' after %lark")?;
                let nested = self.parse_document(true)?;
                self.consume(LarkTokenType::RBrace, "expected '}' after nested Lark grammar")?;
                Ok(LarkNode {
                    nested: Some(Rc::new(nested)),
                    ..LarkNode::new(LarkNodeKind::NestedLark, location)
                })
            },
            LarkTokenType::UnsupportedDirective => {
                Err(self.error(location, format!("directive {} is not supported", token.text)))
            },
            _ => Err(self.error(location, "expected grammar expression")),
        }
    }

    fn parse_string_node(
        &self,
        token: &LarkToken,
    ) -> Result<LarkNode, LarkError> {
        let (json_string, flags) = match token.text.strip_suffix('i') {
            Some(json_string) => (json_string, "i"),
            None => (token.text.as_str(), ""),
        };
        match parse_json(json_string) {
            Ok(Value::String(text)) => Ok(LarkNode {
                flags: flags.to_owned(),
                ..LarkNode::with_text(LarkNodeKind::String, token.location, text)
            }),
            Ok(_) => Err(self.error(token.location, "invalid string literal: ")),
            Err(error) => Err(self.error(token.location, format!("invalid string literal: {error}"))),
        }
    }
}
