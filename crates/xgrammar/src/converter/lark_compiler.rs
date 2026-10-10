//! Lowers a parsed Lark document into the grammar IR.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use serde_json::Value;

use super::{
    lark_definition::LarkDefinition, lark_document::LarkDocument, lark_error::LarkError, lark_lexer::is_c_space,
    lark_location::LarkLocation, lark_node::LarkNode, lark_node_kind::LarkNodeKind, lark_parser::LarkParser,
    lark_special_token_set::LarkSpecialTokenSet, lark_trigger::LarkTrigger, named_grammar::NamedGrammar,
    named_grammar_registry::NamedGrammarRegistry,
};
use crate::{
    fsm::{RegexFsmBuilder, rewrite_regex_dots},
    functor::{add_sub_grammar, dead_code_eliminator, grammar_normalizer},
    grammar::{CharacterClassElement, Grammar, GrammarBuilder, SuffixStopInfo, TagDispatch, TokenTagDispatch},
    support::hex_char_to_int,
    tokenizer::TokenizerInfo,
};

/// Compiles one Lark document (the top-level source, an inline `%lark` block, or a named Lark
/// source) into a normalized [`Grammar`] rooted at its `start` rule.
#[derive(Debug)]
pub struct LarkCompiler<'a, 'r> {
    source: &'a str,
    document: Rc<LarkDocument>,
    tokenizer_info: Option<&'a TokenizerInfo>,
    named_grammars: &'r mut NamedGrammarRegistry<'a>,
    builder: GrammarBuilder,
    definition_by_name: HashMap<String, usize>,
    rule_ids: HashMap<String, i32>,
    named_grammar_roots: HashMap<String, i32>,
    skip_rule_id: Option<i32>,
    allow_initial_skip: bool,
    dynamic_unused_rules: HashSet<String>,
}

/// The regular expressions of the supported `%import common.NAME` terminals.
fn common_regex(path: &str) -> Option<&'static str> {
    Some(match path {
        "common.DIGIT" => "[0-9]",
        "common.HEXDIGIT" => "[a-fA-F0-9]",
        "common.INT" => "[0-9]+",
        "common.SIGNED_INT" => r"(\+|-)?[0-9]+",
        "common.DECIMAL" => r"([0-9]+\.[0-9]*)|(\.[0-9]+)",
        "common._EXP" => r"[eE](\+|-)?[0-9]+",
        "common.FLOAT" => r"([0-9]+\.[0-9]*|\.[0-9]+)([eE](\+|-)?[0-9]+)?|[0-9]+[eE](\+|-)?[0-9]+",
        "common.SIGNED_FLOAT" => r"(\+|-)?(([0-9]+\.[0-9]*|\.[0-9]+)([eE](\+|-)?[0-9]+)?|[0-9]+[eE](\+|-)?[0-9]+)",
        "common.NUMBER" => r"([0-9]+)|([0-9]+\.[0-9]*|\.[0-9]+)([eE](\+|-)?[0-9]+)?|[0-9]+[eE](\+|-)?[0-9]+",
        "common.SIGNED_NUMBER" => {
            r"(\+|-)?(([0-9]+)|([0-9]+\.[0-9]*|\.[0-9]+)([eE](\+|-)?[0-9]+)?|[0-9]+[eE](\+|-)?[0-9]+)"
        },
        "common.ESCAPED_STRING" => r#"\"([^\"\\]|\\.)*\""#,
        "common.LCASE_LETTER" => "[a-z]",
        "common.UCASE_LETTER" => "[A-Z]",
        "common.LETTER" => "[A-Za-z]",
        "common.WORD" => "[A-Za-z]+",
        "common.CNAME" => "[_A-Za-z][_A-Za-z0-9]*",
        "common.WS_INLINE" => r"[ \t]+",
        "common.WS" => r"[ \t\f\r\n]+",
        "common.CR" => r"\r",
        "common.LF" => r"\n",
        "common.NEWLINE" => r"(\r?\n)+",
        "common.SH_COMMENT" => r"#[^\n]*",
        "common.CPP_COMMENT" => r"//[^\n]*",
        "common.C_COMMENT" => r"\/\*[^*]*\*+(?:[^/*][^*]*\*+)*\/",
        "common.SQL_COMMENT" => r"--[^\n]*",
        _ => return None,
    })
}

/// Trims C-locale whitespace from both ends.
fn trim_c_space(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_ascii() && is_c_space(c as u8))
}

/// The prefix of `text` before its first NUL (what a C string view of it contains).
fn c_str(text: &str) -> &str {
    text.split('\0').next().unwrap_or_default()
}

/// Decodes a regex made only of literal characters and escapes into the bytes it matches, or
/// `None` if it uses any other regex construct.
fn parse_fixed_regex_literal(pattern: &str) -> Option<Vec<u8>> {
    let bytes = pattern.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        i += 1;
        if c != b'\\' {
            if b".^$*+?()[]{}|".contains(&c) {
                return None;
            }
            result.push(c);
            continue;
        }
        let escaped = *bytes.get(i)?;
        i += 1;
        match escaped {
            b'n' => result.push(b'\n'),
            b'r' => result.push(b'\r'),
            b't' => result.push(b'\t'),
            b'f' => result.push(0x0C),
            b'v' => result.push(0x0B),
            b'0' => result.push(0),
            b'^' | b'$' | b'.' | b'*' | b'+' | b'?' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'|' | b'\\' | b'/'
            | b'-' => result.push(escaped),
            b'x' | b'u' => {
                let mut codepoint = 0u32;
                if escaped == b'u' && bytes.get(i) == Some(&b'{') {
                    i += 1;
                    let mut digit_count = 0;
                    while let Some(value) = bytes.get(i).and_then(|&b| hex_char_to_int(b)) {
                        if digit_count == 6 {
                            break;
                        }
                        codepoint = codepoint * 16 + value;
                        i += 1;
                        digit_count += 1;
                    }
                    if digit_count == 0 || bytes.get(i) != Some(&b'}') {
                        return None;
                    }
                    i += 1;
                } else {
                    let digit_count = if escaped == b'x' {
                        2
                    } else {
                        4
                    };
                    for _ in 0..digit_count {
                        codepoint = codepoint * 16 + hex_char_to_int(*bytes.get(i)?)?;
                        i += 1;
                    }
                }
                let character = char::from_u32(codepoint)?;
                result.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
            },
            _ => return None,
        }
    }
    Some(result)
}

/// Escapes a literal string so that a regex matches exactly it.
fn escape_regex_literal(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character {
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\0'..='\x1F' | '\x7F' => result.push_str(&format!("\\x{:02X}", character as u32)),
            _ => {
                if r"\.^$|()[]{}*+?".contains(character) {
                    result.push('\\');
                }
                result.push(character);
            },
        }
    }
    result
}

/// Unwraps single-element sequences.
fn unwrap_single(mut node: &LarkNode) -> &LarkNode {
    while node.kind == LarkNodeKind::Sequence && node.children.len() == 1 {
        node = &node.children[0];
    }
    node
}

/// The elements of a sequence, or the node itself.
fn flatten_sequence(node: &LarkNode) -> Vec<LarkNode> {
    if node.kind == LarkNodeKind::Sequence {
        return node.children.clone();
    }
    vec![node.clone()]
}

/// The trigger of a lazy head written as one regex `<any-text prefix><fixed literal>`.
fn extract_lazy_regex_trigger(node: &LarkNode) -> Option<LarkTrigger> {
    if node.kind != LarkNodeKind::Regex || node.flags.contains('i') || !node.flags.chars().all(|f| "su".contains(f)) {
        return None;
    }
    let prefixes: &[&str] = if node.flags.contains('s') {
        &[".*"]
    } else {
        &["(.|\\n)*", "(\\n|.)*", "(?:.|\\n)*", "(?:\\n|.)*", "[\\s\\S]*", "(?s:.*)"]
    };
    prefixes.iter().find_map(|prefix| {
        let rest = node.text.strip_prefix(prefix).filter(|rest| !rest.is_empty())?;
        parse_fixed_regex_literal(rest).filter(|trigger| !trigger.is_empty()).map(LarkTrigger::String)
    })
}

impl<'a, 'r> LarkCompiler<'a, 'r> {
    /// Compiles `document` (parsed from `source`) into a grammar.
    ///
    /// # Errors
    /// Returns a located [`LarkError`] for any invalid or unsupported construct.
    pub fn compile(
        source: &'a str,
        mut document: LarkDocument,
        tokenizer_info: Option<&'a TokenizerInfo>,
        named_grammars: &'r mut NamedGrammarRegistry<'a>,
    ) -> Result<Grammar, LarkError> {
        for import in &document.imports {
            let pattern = common_regex(&import.path).ok_or_else(|| {
                LarkError::at(source, import.location, format!("unknown common import '{}'", import.path))
            })?;
            let regex = LarkNode::with_text(LarkNodeKind::Regex, import.location, pattern);
            document.definitions.push(LarkDefinition::new(import.local_name.clone(), true, regex, import.location));
        }
        Self {
            source,
            document: Rc::new(document),
            tokenizer_info,
            named_grammars,
            builder: GrammarBuilder::new(),
            definition_by_name: HashMap::new(),
            rule_ids: HashMap::new(),
            named_grammar_roots: HashMap::new(),
            skip_rule_id: None,
            allow_initial_skip: false,
            dynamic_unused_rules: HashSet::new(),
        }
        .compile_document()
    }

    fn compile_document(mut self) -> Result<Grammar, LarkError> {
        self.parse_options()?;
        self.index_definitions()?;
        self.validate_terminal_cycles()?;

        let document = Rc::clone(&self.document);
        for definition in &document.definitions {
            let rule_id = self.builder.add_empty_rule(definition.name.clone());
            self.rule_ids.insert(definition.name.clone(), rule_id);
        }
        for definition in document.definitions.iter().filter(|definition| definition.is_terminal) {
            let body = self.compile_node(&definition.body, &definition.name, true, true)?;
            self.builder.update_rule_body(self.rule_ids[&definition.name], body);
        }

        self.compile_ignore()?;

        let start = &document.definitions[self.definition_by_name["start"]];
        let dynamic_start_body = if start.temperature.is_some() {
            None
        } else {
            self.compile_dynamic_start(start)?
        };

        for definition in document.definitions.iter().filter(|definition| !definition.is_terminal) {
            let rule_id = self.rule_ids[&definition.name];
            if self.dynamic_unused_rules.contains(&definition.name) {
                if let Some((_, location)) = definition.max_tokens {
                    return Err(
                        self.error(location, "max_tokens is not supported on rules consumed by dynamic dispatch")
                    );
                }
                if definition.max_chars.is_some() {
                    eprintln!(
                        "Ignoring max_chars on rule '{}' because it is consumed by dynamic dispatch.",
                        definition.name
                    );
                }
                if let Some((_, location)) = definition.capture_name {
                    return Err(self.error(location, "capture is not supported on rules consumed by dynamic dispatch"));
                }
                let empty = self.builder.add_empty_str();
                self.builder.update_rule_body(rule_id, empty);
                continue;
            }
            let body_expr_id = if definition.temperature.is_some() {
                let body = self.compile_temperature_rule(definition)?;
                if let Some(max_chars) = definition.max_chars {
                    self.builder.update_max_chars(rule_id, max_chars);
                }
                body
            } else if definition.name == "start" {
                let body = match dynamic_start_body {
                    Some(dynamic_body) => {
                        if let Some((_, location)) = definition.max_tokens {
                            return Err(
                                self.error(location, "max_tokens is not supported on a dynamic dispatch start rule")
                            );
                        }
                        if definition.max_chars.is_some() {
                            eprintln!("Ignoring max_chars on dynamic dispatch start rule '{}'.", definition.name);
                        }
                        dynamic_body
                    },
                    None => self.compile_rule_body(definition)?,
                };
                match self.skip_rule_id {
                    Some(skip_rule_id) if self.allow_initial_skip => {
                        let skip = self.builder.add_rule_ref(skip_rule_id);
                        self.builder.add_sequence(&[skip, body])
                    },
                    _ => body,
                }
            } else {
                self.compile_rule_body(definition)?
            };
            self.builder.update_rule_body(rule_id, body_expr_id);
            if let Some((capture_name, _)) = &definition.capture_name {
                self.builder.update_capture_name(rule_id, capture_name.clone());
            }
            self.builder.update_rule_temperature(rule_id, definition.temperature);
        }

        let mut root_rule_id = self.rule_ids["start"];
        if let (Some(_), Some(skip_rule_id)) = (start.temperature, self.skip_rule_id) {
            let mut elements = Vec::new();
            if self.allow_initial_skip {
                elements.push(self.builder.add_rule_ref(skip_rule_id));
            }
            elements.push(self.builder.add_rule_ref(root_rule_id));
            elements.push(self.builder.add_rule_ref(skip_rule_id));
            let sequence = self.builder.add_sequence(&elements);
            root_rule_id = self.builder.add_rule_with_hint("start_with_skip", sequence);
        }
        let grammar = self.builder.into_grammar_with_root_id(root_rule_id);
        Ok(dead_code_eliminator(&grammar_normalizer(&grammar)))
    }

    fn error(
        &self,
        location: LarkLocation,
        message: impl Into<String>,
    ) -> LarkError {
        LarkError::at(self.source, location, message)
    }

    fn definition_index(
        &self,
        name: &str,
    ) -> Option<usize> {
        self.definition_by_name.get(name).copied()
    }

    fn compile_temperature_rule(
        &mut self,
        definition: &LarkDefinition,
    ) -> Result<i32, LarkError> {
        let body = unwrap_single(&definition.body);
        if matches!(body.kind, LarkNodeKind::Json | LarkNodeKind::NestedLark | LarkNodeKind::GrammarRef) {
            return self.compile_node(body, &definition.name, false, false);
        }
        self.compile_node(&definition.body, &definition.name, true, false).map_err(|error| {
            self.error(
                definition.location,
                format!("{error}; temperature is only supported on terminals and subgrammars"),
            )
        })
    }

    /// Compiles an ordinary rule body, applying its token and character budgets.
    fn compile_rule_body(
        &mut self,
        definition: &LarkDefinition,
    ) -> Result<i32, LarkError> {
        let rule_id = self.rule_ids[&definition.name];
        if let Some((max_tokens, _)) = definition.max_tokens {
            self.builder.update_max_tokens(rule_id, max_tokens);
        }
        if let Some(max_chars) = definition.max_chars {
            self.builder.update_max_chars(rule_id, max_chars);
        }
        if definition.has_lazy_semantics() {
            return self.compile_lazy_rule(definition);
        }
        self.compile_node(&definition.body, &definition.name, false, true)
    }

    fn parse_options(&mut self) -> Result<(), LarkError> {
        let document = Rc::clone(&self.document);
        for (value, location) in &document.options {
            let Value::Object(object) = value else {
                return Err(self.error(*location, "%grammar_options value must be an object"));
            };
            for (key, option) in object {
                let flag = match key.as_str() {
                    "allow_initial_skip" | "no_forcing" | "allow_invalid_utf8" => option.as_bool(),
                    _ => return Err(self.error(*location, format!("unknown %grammar_options option '{key}'"))),
                };
                let Some(flag) = flag else {
                    return Err(self.error(*location, format!("{key} must be a boolean")));
                };
                if key == "allow_initial_skip" {
                    self.allow_initial_skip |= flag;
                } else if flag {
                    return Err(self.error(*location, format!("%grammar_options option '{key}' is not supported")));
                }
            }
        }
        Ok(())
    }

    fn index_definitions(&mut self) -> Result<(), LarkError> {
        let document = Rc::clone(&self.document);
        for (index, definition) in document.definitions.iter().enumerate() {
            if self.definition_by_name.insert(definition.name.clone(), index).is_some() {
                return Err(
                    self.error(definition.location, format!("duplicate rule or terminal '{}'", definition.name))
                );
            }
        }
        match self.definition_index("start").map(|index| &document.definitions[index]) {
            None => Err(self.error(LarkLocation::START, "no start rule found")),
            Some(start) if start.is_terminal => Err(self.error(start.location, "start must be a rule")),
            Some(_) => Ok(()),
        }
    }

    fn collect_referenced_names<'n>(
        node: &'n LarkNode,
        names: &mut Vec<&'n str>,
    ) {
        if node.kind == LarkNodeKind::Name {
            names.push(&node.text);
        }
        for child in &node.children {
            Self::collect_referenced_names(child, names);
        }
    }

    fn validate_terminal_cycles(&self) -> Result<(), LarkError> {
        let mut states = HashMap::new();
        for definition in &self.document.definitions {
            if definition.is_terminal && !states.contains_key(definition.name.as_str()) {
                self.visit_terminal(definition, &mut states)?;
            }
        }
        Ok(())
    }

    /// Depth-first visit of terminal references; `states` maps a name to 1 (visiting) or 2 (done).
    fn visit_terminal<'d>(
        &'d self,
        definition: &'d LarkDefinition,
        states: &mut HashMap<&'d str, i32>,
    ) -> Result<(), LarkError> {
        states.insert(&definition.name, 1);
        let mut names = Vec::new();
        Self::collect_referenced_names(&definition.body, &mut names);
        for name in names {
            let Some(index) = self.definition_index(name) else {
                return Err(self.error(definition.location, format!("unknown name '{name}'")));
            };
            let referenced = &self.document.definitions[index];
            if !referenced.is_terminal {
                return Err(self.error(
                    definition.location,
                    format!("terminal '{}' cannot reference rule '{name}'", definition.name),
                ));
            }
            match states.get(name) {
                Some(1) => {
                    return Err(self.error(definition.location, format!("circular reference in terminal '{name}'")));
                },
                Some(_) => {},
                None => self.visit_terminal(referenced, states)?,
            }
        }
        states.insert(&definition.name, 2);
        Ok(())
    }

    fn compile_ignore(&mut self) -> Result<(), LarkError> {
        let document = Rc::clone(&self.document);
        if document.ignores.is_empty() {
            return Ok(());
        }
        let mut ignore_choices = Vec::with_capacity(document.ignores.len());
        for ignore in &document.ignores {
            ignore_choices.push(self.compile_node(ignore, "lark_ignore", true, true)?);
        }
        let ignore_body = if ignore_choices.len() == 1 {
            ignore_choices[0]
        } else {
            self.builder.add_choices(&ignore_choices)
        };
        let ignore_item_rule = self.builder.add_rule_with_hint("lark_ignore_item", ignore_body);
        let ignore_repeat = self.builder.add_repeat(ignore_item_rule, 0, -1);
        self.skip_rule_id = Some(self.builder.add_rule_with_hint("lark_ignore", ignore_repeat));
        Ok(())
    }

    /// The characters of a case-insensitive (`"..."i`) literal, which must be ASCII.
    fn case_insensitive_chars(
        &self,
        node: &LarkNode,
    ) -> Result<Vec<char>, LarkError> {
        if node.flags != "i" {
            return Err(self.error(node.location, format!("unsupported string literal flags '{}'", node.flags)));
        }
        let characters: Vec<char> = c_str(&node.text).chars().collect();
        if !node.text.is_empty() && characters.is_empty() {
            return Err(self.error(node.location, "case-insensitive string is not valid UTF-8"));
        }
        if !characters.iter().all(char::is_ascii) {
            return Err(
                self.error(node.location, "case-insensitive string literals currently support ASCII characters only")
            );
        }
        Ok(characters)
    }

    fn compile_string_literal(
        &mut self,
        node: &LarkNode,
    ) -> Result<i32, LarkError> {
        if node.flags.is_empty() {
            return Ok(if node.text.is_empty() {
                self.builder.add_empty_str()
            } else {
                self.builder.add_byte_string(&node.text)
            });
        }
        let mut elements = Vec::new();
        for character in self.case_insensitive_chars(node)? {
            elements.push(if character.is_ascii_alphabetic() {
                let lower = i32::from(character.to_ascii_lowercase() as u8);
                let upper = i32::from(character.to_ascii_uppercase() as u8);
                self.builder.add_character_class(
                    &[CharacterClassElement::new(lower, lower), CharacterClassElement::new(upper, upper)],
                    false,
                )
            } else {
                self.builder.add_byte_string(character.encode_utf8(&mut [0; 4]))
            });
        }
        Ok(match elements.len() {
            0 => self.builder.add_empty_str(),
            1 => elements[0],
            _ => self.builder.add_sequence(&elements),
        })
    }

    /// Validates regex flags, returning `(case_insensitive, dot_all)`.
    fn parse_regex_flags(
        &self,
        node: &LarkNode,
    ) -> Result<(bool, bool), LarkError> {
        let mut flags = (false, false);
        for flag in node.flags.chars() {
            match flag {
                'i' => flags.0 = true,
                's' => flags.1 = true,
                // XGrammar regular expressions use Unicode codepoint semantics by default.
                'u' => {},
                _ => {
                    return Err(self.error(node.location, format!("regular-expression flag '{flag}' is not supported")));
                },
            }
        }
        Ok(flags)
    }

    fn string_literal_to_regex(
        &self,
        node: &LarkNode,
    ) -> Result<String, LarkError> {
        if node.flags.is_empty() {
            return Ok(escape_regex_literal(&node.text));
        }
        let mut result = String::new();
        for character in self.case_insensitive_chars(node)? {
            if character.is_ascii_alphabetic() {
                result.push('[');
                result.push(character.to_ascii_lowercase());
                result.push(character.to_ascii_uppercase());
                result.push(']');
            } else {
                result.push_str(&escape_regex_literal(character.encode_utf8(&mut [0; 4])));
            }
        }
        Ok(result)
    }

    /// The single codepoints of a character range's endpoints.
    fn range_endpoints(
        &self,
        node: &LarkNode,
    ) -> Result<(char, char), LarkError> {
        let single = |text: &str| {
            let mut characters = c_str(text).chars();
            characters.next().filter(|_| characters.next().is_none())
        };
        let (Some(begin), Some(end)) = (single(&node.text), single(&node.text2)) else {
            return Err(self.error(node.location, "character range endpoints must be one character"));
        };
        if begin > end {
            return Err(self.error(node.location, "character range start must not exceed end"));
        }
        Ok((begin, end))
    }

    /// Lowers a terminal expression to a single regular expression (used by suffix and stop).
    fn terminal_node_to_regex(
        &self,
        node: &LarkNode,
        visiting: &mut HashSet<String>,
    ) -> Result<String, LarkError> {
        let message = match node.kind {
            LarkNodeKind::Sequence => {
                let mut result = String::new();
                for child in &node.children {
                    let pattern = self.terminal_node_to_regex(child, visiting)?;
                    if !pattern.is_empty() {
                        result.push_str(&format!("(?:{pattern})"));
                    }
                }
                return Ok(result);
            },
            LarkNodeKind::Choice => {
                let mut alternatives = Vec::with_capacity(node.children.len());
                for child in &node.children {
                    alternatives.push(self.terminal_node_to_regex(child, visiting)?);
                }
                return Ok(format!("(?:{})", alternatives.join("|")));
            },
            LarkNodeKind::Repeat => {
                let child = self.terminal_node_to_regex(&node.children[0], visiting)?;
                if child.is_empty() {
                    return Ok(String::new());
                }
                let quantifier = match (node.min_repeat, node.max_repeat) {
                    (0, -1) => "*".to_owned(),
                    (1, -1) => "+".to_owned(),
                    (0, 1) => "?".to_owned(),
                    (min, max) if min == max => format!("{{{min}}}"),
                    (min, -1) => format!("{{{min},}}"),
                    (min, max) => format!("{{{min},{max}}}"),
                };
                return Ok(format!("(?:{child}){quantifier}"));
            },
            LarkNodeKind::String => return self.string_literal_to_regex(node),
            LarkNodeKind::Regex => {
                let (case_insensitive, dot_all) = self.parse_regex_flags(node)?;
                if case_insensitive {
                    return Err(self.error(
                        node.location,
                        "regular-expression flag 'i' is not supported with suffix or stop attributes",
                    ));
                }
                let pattern = rewrite_regex_dots(node.text.as_bytes(), dot_all);
                return Ok(format!("(?:{})", String::from_utf8_lossy(&pattern)));
            },
            LarkNodeKind::Range => {
                let (begin, end) = self.range_endpoints(node)?;
                let escape_class_character = |character: char| {
                    if "\\]-^".contains(character) {
                        format!("\\{character}")
                    } else {
                        character.to_string()
                    }
                };
                return Ok(format!("[{}-{}]", escape_class_character(begin), escape_class_character(end)));
            },
            LarkNodeKind::Name => {
                let Some(index) = self.definition_index(&node.text) else {
                    return Err(self.error(node.location, format!("unknown name '{}'", node.text)));
                };
                let definition = &self.document.definitions[index];
                if !definition.is_terminal {
                    return Err(self.error(node.location, format!("terminal cannot reference rule '{}'", node.text)));
                }
                if !visiting.insert(node.text.clone()) {
                    return Err(
                        self.error(node.location, format!("recursive terminal '{}' is not supported", node.text))
                    );
                }
                let result = self.terminal_node_to_regex(&definition.body, visiting)?;
                visiting.remove(&node.text);
                return Ok(result);
            },
            LarkNodeKind::SpecialToken => "special tokens cannot be used in terminals",
            LarkNodeKind::Json => "%json cannot be used in terminals",
            LarkNodeKind::NestedLark => "nested %lark cannot be used in terminals",
            LarkNodeKind::RegexExt => "structured %regex cannot be used with suffix or stop",
            LarkNodeKind::GrammarRef => "named grammars cannot be used in terminals",
            LarkNodeKind::Not => "regular-expression complement '~' is not supported",
        };
        Err(self.error(node.location, message))
    }

    fn parse_structured_regex_chunks(
        &self,
        node: &LarkNode,
    ) -> Result<Vec<Vec<u8>>, LarkError> {
        let error = |message: &str| self.error(node.location, message);
        let value = crate::support::parse_json(&node.text)
            .map_err(|parse_error| error(&format!("failed to parse %regex: {parse_error}")))?;
        let Value::Object(object) = value else {
            return Err(error("%regex value must be an object"));
        };
        let mut fields = Vec::new();
        for (key, field_value) in &object {
            if !matches!(key.as_str(), "substring_chunks" | "substring_chars" | "substring_words") {
                return Err(error(&format!("unknown field '{key}' in %regex")));
            }
            if !field_value.is_null() {
                fields.push((key.as_str(), field_value));
            }
        }
        let &[(field, field_value)] = fields.as_slice() else {
            return Err(error(if fields.is_empty() {
                "no fields set on %regex"
            } else {
                "only one field can be set on %regex"
            }));
        };
        match field {
            "substring_words" => Err(error(if field_value.is_string() {
                "substring_words is not supported yet"
            } else {
                "substring_words must be a string"
            })),
            "substring_chars" => {
                let Value::String(text) = field_value else {
                    return Err(error("substring_chars must be a string"));
                };
                Ok(text.chars().map(|character| character.to_string().into_bytes()).collect())
            },
            _ => {
                let chunks = field_value.as_array().and_then(|array| {
                    array.iter().map(|chunk| chunk.as_str().map(|chunk| chunk.as_bytes().to_vec())).collect()
                });
                chunks.ok_or_else(|| error("substring_chunks must be an array of strings"))
            },
        }
    }

    fn compile_structured_regex(
        &mut self,
        node: &LarkNode,
        rule_hint: &str,
    ) -> Result<i32, LarkError> {
        let chunks = self.parse_structured_regex_chunks(node)?;
        // A substring expr can only be the body of a rule, so wrap it and return a reference.
        let substring_expr_id = self.builder.add_substring(&chunks);
        let rule_id = self.builder.add_rule_with_hint(&format!("{rule_hint}_substring"), substring_expr_id);
        Ok(self.builder.add_rule_ref(rule_id))
    }

    /// Makes the named grammar `name` available in the registry, compiling it if it is a Lark
    /// source.
    fn resolve_named_grammar(
        &mut self,
        name: &str,
        location: LarkLocation,
    ) -> Result<(), LarkError> {
        let Some(&input) = self.named_grammars.inputs.get(name) else {
            return Err(self.error(location, format!("unknown named grammar '@{name}'")));
        };
        let NamedGrammar::Lark {
            source: named_source,
            ..
        } = input
        else {
            return Ok(());
        };
        if self.named_grammars.compiled.contains_key(name) {
            return Ok(());
        }
        if let Some(cycle_start) = self.named_grammars.active.iter().position(|active| active == name) {
            let mut cycle: Vec<String> =
                self.named_grammars.active[cycle_start..].iter().map(|active| format!("@{active}")).collect();
            cycle.push(format!("@{name}"));
            return Err(self.error(location, format!("circular named grammar reference: {}", cycle.join(" -> "))));
        }

        self.named_grammars.active.push(name.to_owned());
        let compiled = LarkParser::parse(named_source).and_then(|document| {
            LarkCompiler::compile(named_source, document, self.tokenizer_info, self.named_grammars)
        });
        self.named_grammars.active.pop();
        let compiled = compiled
            .map_err(|error| self.error(location, format!("failed to compile named grammar '@{name}': {error}")))?;
        self.named_grammars.compiled.insert(name.to_owned(), compiled);
        Ok(())
    }

    fn compile_node(
        &mut self,
        node: &LarkNode,
        rule_hint: &str,
        terminal_mode: bool,
        append_skip: bool,
    ) -> Result<i32, LarkError> {
        // Whether to append the `%ignore` skip after a lexeme-like result.
        let skip_lexeme = !terminal_mode && append_skip;
        let result = match node.kind {
            LarkNodeKind::Sequence | LarkNodeKind::Choice => {
                if node.children.is_empty() {
                    return Ok(self.builder.add_empty_str());
                }
                let mut elements = Vec::with_capacity(node.children.len());
                for child in &node.children {
                    elements.push(self.compile_node(child, rule_hint, terminal_mode, append_skip)?);
                }
                return Ok(match (elements.len(), node.kind) {
                    (1, _) => elements[0],
                    (_, LarkNodeKind::Sequence) => self.builder.add_sequence(&elements),
                    _ => self.builder.add_choices(&elements),
                });
            },
            LarkNodeKind::Repeat => {
                let repeat_hint = format!("{rule_hint}_repeat");
                let child = self.compile_node(&node.children[0], &repeat_hint, terminal_mode, append_skip)?;
                return Ok(self.builder.add_repeat_from_expr(&repeat_hint, child, node.min_repeat, node.max_repeat));
            },
            LarkNodeKind::Name => {
                let Some(index) = self.definition_index(&node.text) else {
                    return Err(self.error(node.location, format!("unknown name '{}'", node.text)));
                };
                let definition = &self.document.definitions[index];
                if terminal_mode && !definition.is_terminal {
                    return Err(self.error(node.location, format!("terminal cannot reference rule '{}'", node.text)));
                }
                // Lazy and temperature rules are compiled like terminals (lexemes), so they also
                // take a trailing skip.
                let is_lexeme =
                    definition.is_terminal || definition.has_lazy_semantics() || definition.temperature.is_some();
                let result = self.builder.add_rule_ref(self.rule_ids[&node.text]);
                return Ok(if skip_lexeme && is_lexeme {
                    self.append_skip(result)
                } else {
                    result
                });
            },
            LarkNodeKind::String => {
                let result = self.compile_string_literal(node)?;
                return Ok(if skip_lexeme && !node.text.is_empty() {
                    self.append_skip(result)
                } else {
                    result
                });
            },
            LarkNodeKind::Range => {
                let (begin, end) = self.range_endpoints(node)?;
                self.builder.add_character_class(&[CharacterClassElement::new(begin as i32, end as i32)], false)
            },
            LarkNodeKind::Regex => {
                let (case_insensitive, dot_all) = self.parse_regex_flags(node)?;
                let pattern = String::from_utf8_lossy(&rewrite_regex_dots(node.text.as_bytes(), dot_all)).into_owned();
                if case_insensitive {
                    // The FSM regex engine handles the (?i) prefix with ASCII case folding. Validate
                    // the pattern eagerly so that errors carry the source location.
                    let flagged_pattern = format!("(?i){pattern}");
                    RegexFsmBuilder::matches_empty(flagged_pattern.as_bytes()).map_err(|error| {
                        self.error(node.location, format!("failed to compile regular expression: {error}"))
                    })?;
                    let regex_expr = self.builder.add_regex(flagged_pattern.as_bytes(), false);
                    let regex_rule_id = self.builder.add_rule_with_hint(&format!("{rule_hint}_regex"), regex_expr);
                    self.builder.add_rule_ref(regex_rule_id)
                } else {
                    let grammar = Grammar::from_regex(&pattern).map_err(|error| {
                        self.error(node.location, format!("failed to compile regular expression: {error}"))
                    })?;
                    let root = add_sub_grammar(&mut self.builder, &grammar);
                    self.builder.add_rule_ref(root)
                }
            },
            LarkNodeKind::Json => {
                if terminal_mode {
                    return Err(self.error(node.location, "%json cannot be used in terminals"));
                }
                let grammar = Grammar::from_json_schema(&node.text, true, None, None, true, None).map_err(|error| {
                    self.error(node.location, format!("failed to compile inline JSON schema: {error}"))
                })?;
                let root = add_sub_grammar(&mut self.builder, &grammar);
                self.builder.add_rule_ref(root)
            },
            LarkNodeKind::NestedLark => {
                if terminal_mode {
                    return Err(self.error(node.location, "nested %lark cannot be used in terminals"));
                }
                let Some(nested) = &node.nested else {
                    return Err(self.error(node.location, "unsupported grammar node"));
                };
                let grammar = LarkCompiler::compile(
                    self.source,
                    LarkDocument::clone(nested),
                    self.tokenizer_info,
                    self.named_grammars,
                )
                .map_err(|error| {
                    self.error(node.location, format!("failed to compile nested Lark grammar: {error}"))
                })?;
                let root = add_sub_grammar(&mut self.builder, &grammar);
                self.builder.add_rule_ref(root)
            },
            LarkNodeKind::SpecialToken => {
                if terminal_mode {
                    return Err(self.error(node.location, "special tokens cannot be used in terminals"));
                }
                let token_set = self.resolve_special_token(&node.text, node.location)?;
                if token_set.excluded {
                    self.builder.add_exclude_token_set(&token_set.token_ids)
                } else {
                    self.builder.add_token_set(&token_set.token_ids)
                }
            },
            LarkNodeKind::RegexExt => self.compile_structured_regex(node, rule_hint)?,
            LarkNodeKind::GrammarRef => {
                if terminal_mode {
                    return Err(self.error(node.location, "named grammars cannot be used in terminals"));
                }
                let name = &node.text[1..];
                let root = match self.named_grammar_roots.get(name) {
                    Some(&root) => root,
                    None => {
                        self.resolve_named_grammar(name, node.location)?;
                        let Some(grammar) = self.named_grammars.get(name) else {
                            return Err(self.error(node.location, format!("unknown named grammar '@{name}'")));
                        };
                        let root = add_sub_grammar(&mut self.builder, grammar);
                        self.named_grammar_roots.insert(name.to_owned(), root);
                        root
                    },
                };
                self.builder.add_rule_ref(root)
            },
            LarkNodeKind::Not => {
                return Err(self.error(node.location, "regular-expression complement '~' is not supported"));
            },
        };
        Ok(if skip_lexeme {
            self.append_skip(result)
        } else {
            result
        })
    }

    fn append_skip(
        &mut self,
        expression: i32,
    ) -> i32 {
        let Some(skip_rule_id) = self.skip_rule_id else {
            return expression;
        };
        let skip = self.builder.add_rule_ref(skip_rule_id);
        self.builder.add_sequence(&[expression, skip])
    }

    fn resolve_special_token(
        &self,
        token: &str,
        location: LarkLocation,
    ) -> Result<LarkSpecialTokenSet, LarkError> {
        let mut result = LarkSpecialTokenSet::default();
        let Some(mut contents) = token.strip_prefix("<[").and_then(|rest| rest.strip_suffix("]>")) else {
            let Some(tokenizer_info) = self.tokenizer_info else {
                return Err(self.error(location, format!("named special token {token} requires tokenizer_info")));
            };
            result.token_ids = (0..)
                .zip(tokenizer_info.decoded_vocab())
                .filter(|(_, decoded)| decoded.as_slice() == token.as_bytes())
                .map(|(token_id, _)| token_id)
                .collect();
            if result.token_ids.is_empty() {
                return Err(self.error(location, format!("unknown special token {token}")));
            }
            return Ok(result);
        };
        if let Some(rest) = contents.strip_prefix('^') {
            result.excluded = true;
            contents = rest;
        }
        if contents == "*" {
            if result.excluded {
                return Err(self.error(location, "negated wildcard special token is not supported"));
            }
            let Some(tokenizer_info) = self.tokenizer_info else {
                return Err(self.error(location, "wildcard special token requires tokenizer_info"));
            };
            result.token_ids = (0..tokenizer_info.vocab_size()).collect();
            return Ok(result);
        }
        if contents.contains('*') {
            return Err(self.error(location, "wildcard cannot be mixed with token ranges"));
        }
        for range in contents.split(',').map(trim_c_space).filter(|range| !range.is_empty()) {
            let invalid = || self.error(location, format!("invalid numeric special-token range '{range}'"));
            let parse_token_id = |value: &str| trim_c_space(value).parse::<i64>().ok();
            let (first, last) = match range.split_once('-') {
                Some((_, last)) if last.contains('-') => return Err(invalid()),
                Some((first, last)) => (parse_token_id(first), parse_token_id(last)),
                None => (parse_token_id(range), parse_token_id(range)),
            };
            let (Some(first), Some(last)) = (first, last) else {
                return Err(invalid());
            };
            if first < 0 || last < first || last > i64::from(i32::MAX) {
                return Err(invalid());
            }
            if last - first > 1_000_000 {
                return Err(self.error(location, "special-token range is too large"));
            }
            result.token_ids.extend((first..=last).map(|token_id| token_id as i32));
        }
        if result.token_ids.is_empty() {
            return Err(self.error(location, "empty numeric special-token range"));
        }
        result.token_ids.sort_unstable();
        result.token_ids.dedup();
        Ok(result)
    }

    /// Whether `node` matches any text: an any-text regex, possibly behind terminal names.
    fn is_any_text(
        &self,
        node: &LarkNode,
        visiting: &mut HashSet<String>,
    ) -> bool {
        match node.kind {
            LarkNodeKind::Regex => {
                let pattern: String = node.text.chars().filter(|c| !matches!(c, ' ' | '\t' | '\r')).collect();
                if !node.flags.chars().all(|flag| "isu".contains(flag)) {
                    return false;
                }
                if node.flags.contains('s') {
                    return pattern == ".*";
                }
                ["(.|\\n)*", "(\\n|.)*", "(?s:.*)", "(?:.|\\n)*", "(?:\\n|.)*", "[\\s\\S]*"].contains(&pattern.as_str())
            },
            LarkNodeKind::Sequence if node.children.len() == 1 => self.is_any_text(&node.children[0], visiting),
            LarkNodeKind::Name => {
                let Some(index) = self.definition_index(&node.text) else {
                    return false;
                };
                if !visiting.insert(node.text.clone()) {
                    return false;
                }
                let result = self.is_any_text(&self.document.definitions[index].body, visiting);
                visiting.remove(&node.text);
                result
            },
            _ => false,
        }
    }

    fn extract_lazy_trigger(
        &self,
        definition: &LarkDefinition,
    ) -> Result<Option<LarkTrigger>, LarkError> {
        if let Some(marker) = definition.stop.as_ref().or(definition.suffix.as_ref()) {
            let is_fixed_string = marker.kind == LarkNodeKind::String && marker.flags.is_empty();
            if !is_fixed_string || !self.is_any_text(&definition.body, &mut HashSet::new()) {
                return Ok(None);
            }
            return Ok(Some(LarkTrigger::String(marker.text.as_bytes().to_vec())));
        }
        if !definition.lazy {
            return Ok(None);
        }
        let regex_trigger = extract_lazy_regex_trigger(unwrap_single(&definition.body));
        if regex_trigger.is_some() {
            return Ok(regex_trigger);
        }
        let body = &definition.body;
        if body.kind != LarkNodeKind::Sequence
            || body.children.len() != 2
            || !self.is_any_text(&body.children[0], &mut HashSet::new())
        {
            return Ok(None);
        }
        let trigger = &body.children[1];
        if trigger.kind == LarkNodeKind::String && !trigger.text.is_empty() && trigger.flags.is_empty() {
            return Ok(Some(LarkTrigger::String(trigger.text.as_bytes().to_vec())));
        }
        if trigger.kind == LarkNodeKind::SpecialToken {
            let token_set = self.resolve_special_token(&trigger.text, trigger.location)?;
            if token_set.excluded {
                return Err(self.error(trigger.location, "lazy special-token trigger cannot be negated"));
            }
            return Ok(Some(LarkTrigger::Token(token_set.token_ids)));
        }
        Ok(None)
    }

    fn compile_lazy_rule(
        &mut self,
        definition: &LarkDefinition,
    ) -> Result<i32, LarkError> {
        let rule_id = self.rule_ids[&definition.name];
        let marker = definition.suffix.as_ref().or(definition.stop.as_ref());
        let fixed_marker_length = marker.filter(|marker| marker.kind == LarkNodeKind::String).map(|m| m.text.len());
        let hidden_bytes = fixed_marker_length.map_or(1, |length| length as i32);
        let mut suffix_stop_info = SuffixStopInfo::default();
        if definition.suffix.is_some() {
            suffix_stop_info.hidden_suffix_bytes = hidden_bytes;
        } else if definition.stop.is_some() {
            suffix_stop_info.hidden_stop_bytes = hidden_bytes;
        }
        if let Some(stop_capture_name) = &definition.stop_capture_name {
            suffix_stop_info.stop_capture_name = stop_capture_name.clone();
        }
        let body = unwrap_single(&definition.body);
        if marker.is_none() && body.kind == LarkNodeKind::Regex && extract_lazy_regex_trigger(body).is_some() {
            return Err(self
                .error(definition.location, "lazy regex suffix is only supported on a head used by dynamic dispatch"));
        }
        let mut patterns = None;
        let needs_helpers =
            fixed_marker_length.is_none() || definition.max_tokens.is_some() || definition.max_chars.is_some();
        if let Some(marker) = marker.filter(|_| needs_helpers) {
            let body_pattern = self.terminal_node_to_regex(&definition.body, &mut HashSet::new())?;
            let marker_pattern = self.terminal_node_to_regex(marker, &mut HashSet::new())?;
            let body_helper_expr = self.builder.add_regex(body_pattern.as_bytes(), false);
            suffix_stop_info.body_rule_id =
                self.builder.add_rule_with_hint(&format!("{}_stop_body", definition.name), body_helper_expr);
            let marker_helper_expr = self.builder.add_regex(marker_pattern.as_bytes(), false);
            suffix_stop_info.marker_rule_id =
                self.builder.add_rule_with_hint(&format!("{}_stop_marker", definition.name), marker_helper_expr);
            patterns = Some((body_pattern, marker_pattern));
        }
        self.builder.update_suffix_stop_info(rule_id, suffix_stop_info);
        let Some(trigger) = self.extract_lazy_trigger(definition)? else {
            // General committed-shortest lazy rule: compiled like a terminal (no skip insertion);
            // the terminal-like requirement is validated after grammar optimization. suffix="s" and
            // stop="s" both desugar to the lazy rule over (body "s"); their only difference is
            // capture scope, represented by the metadata set above.
            self.builder.update_lazy(rule_id, true);
            let Some(marker) = marker else {
                return self.compile_node(&definition.body, &definition.name, true, true);
            };
            let (body_pattern, marker_pattern) = match patterns {
                Some(patterns) => patterns,
                None => (
                    self.terminal_node_to_regex(&definition.body, &mut HashSet::new())?,
                    self.terminal_node_to_regex(marker, &mut HashSet::new())?,
                ),
            };
            return Ok(self.builder.add_regex(format!("(?:{body_pattern})(?:{marker_pattern})").as_bytes(), false));
        };
        let empty_expr = self.builder.add_empty_str();
        let empty_rule = self.builder.add_rule_with_hint("lark_lazy_end", empty_expr);
        let result = match trigger {
            LarkTrigger::String(trigger) => self.builder.add_tag_dispatch(&TagDispatch {
                tag_rule_pairs: vec![(trigger, empty_rule)],
                loop_after_dispatch: false,
                excludes: Vec::new(),
            }),
            LarkTrigger::Token(token_ids) => self.builder.add_token_tag_dispatch(&TokenTagDispatch {
                trigger_rule_pairs: token_ids.into_iter().map(|token_id| (token_id, empty_rule)).collect(),
                loop_after_dispatch: false,
                excludes: Vec::new(),
            }),
        };
        Ok(self.append_skip(result))
    }

    /// Lowers `start: (tool_a | tool_b)* tail` with lazy tool heads and an any-text tail into a
    /// looping tag dispatch, or returns `None` if `start` does not have that shape.
    fn compile_dynamic_start(
        &mut self,
        start: &LarkDefinition,
    ) -> Result<Option<i32>, LarkError> {
        let document = Rc::clone(&self.document);
        let mut unused_rules = HashSet::new();
        let start_elements = flatten_sequence(&start.body);
        let [loop_element, tail_element] = &start_elements[..] else {
            return Ok(None);
        };
        let loop_node = unwrap_single(loop_element);
        if loop_node.kind != LarkNodeKind::Repeat || loop_node.min_repeat != 0 || loop_node.max_repeat != -1 {
            return Ok(None);
        }
        let loop_body = unwrap_single(&loop_node.children[0]);
        let mut tool_names = Vec::new();
        match loop_body.kind {
            LarkNodeKind::Choice => {
                for alternative in &loop_body.children {
                    let name = unwrap_single(alternative);
                    if name.kind != LarkNodeKind::Name {
                        return Ok(None);
                    }
                    tool_names.push(name.text.as_str());
                }
            },
            LarkNodeKind::Name => tool_names.push(loop_body.text.as_str()),
            _ => return Ok(None),
        }

        let tail_name = unwrap_single(tail_element);
        if tail_name.kind != LarkNodeKind::Name {
            return Ok(None);
        }
        match self.definition_index(&tail_name.text) {
            Some(index) if self.is_any_text(&document.definitions[index].body, &mut HashSet::new()) => {},
            _ => return Ok(None),
        }
        unused_rules.insert(tail_name.text.clone());

        let mut alternatives = Vec::new();
        for tool_name in tool_names {
            let tool = match self.definition_index(tool_name) {
                Some(index) if !document.definitions[index].is_terminal => &document.definitions[index],
                _ => return Ok(None),
            };
            unused_rules.insert(tool_name.to_owned());
            let tool_elements = flatten_sequence(&tool.body);
            if tool_elements.is_empty() {
                return Ok(None);
            }

            let mut trigger = None;
            let mut marker_event_rule_id = -1;
            let mut remainder_begin = 0;
            let first = unwrap_single(&tool_elements[0]);
            let head = (first.kind == LarkNodeKind::Name)
                .then(|| self.definition_index(&first.text))
                .flatten()
                .map(|index| &document.definitions[index]);
            if let Some(head) = head {
                trigger = self.extract_lazy_trigger(head)?;
                if trigger.is_some() {
                    unused_rules.insert(first.text.clone());
                    remainder_begin = 1;
                }
                // A stop marker or stop capture implies a fixed-string trigger.
                let has_stop = head.stop.is_some() || head.stop_capture_name.is_some();
                if let Some(LarkTrigger::String(marker)) = trigger.as_ref().filter(|_| has_stop) {
                    // The dispatch FSM consumes the trigger before entering the remainder. Insert a
                    // zero-width rule there so capture materialization can recover that preceding
                    // marker without giving up the deterministic dispatch path.
                    let event_expr = self.builder.add_empty_str();
                    marker_event_rule_id =
                        self.builder.add_rule_with_hint(&format!("{}_dynamic_marker", head.name), event_expr);
                    let marker_bytes: Vec<i32> = marker.iter().map(|&byte| i32::from(byte)).collect();
                    let marker_expr = self.builder.add_byte_string_bytes(&marker_bytes);
                    let marker_rule_id =
                        self.builder.add_rule_with_hint(&format!("{}_dynamic_marker_text", head.name), marker_expr);
                    let hidden_bytes = marker.len() as i32;
                    let suffix_stop_info = SuffixStopInfo {
                        body_rule_id: marker_event_rule_id,
                        marker_rule_id,
                        hidden_stop_bytes: if head.stop.is_some() {
                            hidden_bytes
                        } else {
                            0
                        },
                        hidden_suffix_bytes: if head.stop.is_some() {
                            0
                        } else {
                            hidden_bytes
                        },
                        stop_capture_name: head.stop_capture_name.clone().unwrap_or_default(),
                        ..SuffixStopInfo::default()
                    };
                    self.builder.update_suffix_stop_info(marker_event_rule_id, suffix_stop_info);
                }
            }
            if trigger.is_none() && tool_elements.len() >= 2 && self.is_any_text(&tool_elements[0], &mut HashSet::new())
            {
                let token_trigger = unwrap_single(&tool_elements[1]);
                if token_trigger.kind == LarkNodeKind::SpecialToken {
                    let token_set = self.resolve_special_token(&token_trigger.text, token_trigger.location)?;
                    if token_set.excluded {
                        return Err(
                            self.error(token_trigger.location, "dynamic special-token trigger cannot be negated")
                        );
                    }
                    trigger = Some(LarkTrigger::Token(token_set.token_ids));
                    remainder_begin = 2;
                }
            }
            let Some(trigger) = trigger else {
                return Ok(None);
            };
            let remainder = LarkNode::with_children(
                LarkNodeKind::Sequence,
                tool.location,
                tool_elements[remainder_begin..].to_vec(),
            );
            alternatives.push((trigger, remainder, marker_event_rule_id));
        }

        let Some((first_trigger, ..)) = alternatives.first() else {
            return Ok(None);
        };
        let is_string_level = matches!(first_trigger, LarkTrigger::String(_));
        if alternatives.iter().any(|(trigger, ..)| matches!(trigger, LarkTrigger::String(_)) != is_string_level) {
            return Err(self.error(start.location, "a dynamic Lark start rule cannot mix string and token triggers"));
        }

        // Group alternatives by trigger string or trigger token, in order of first appearance.
        let mut groups: Vec<(LarkTrigger, Vec<usize>)> = Vec::new();
        let mut group_by_trigger = HashMap::new();
        for (index, (trigger, ..)) in alternatives.iter().enumerate() {
            let keys = match trigger {
                LarkTrigger::String(_) => vec![trigger.clone()],
                LarkTrigger::Token(token_ids) => token_ids.iter().map(|&id| LarkTrigger::Token(vec![id])).collect(),
            };
            for key in keys {
                let group = *group_by_trigger.entry(key.clone()).or_insert_with(|| {
                    groups.push((key, Vec::new()));
                    groups.len() - 1
                });
                groups[group].1.push(index);
            }
        }

        let body_hint = if is_string_level {
            "lark_dynamic_body"
        } else {
            "lark_dynamic_token_body"
        };
        let mut tag_rule_pairs = Vec::with_capacity(groups.len());
        let mut trigger_rule_pairs = Vec::with_capacity(groups.len());
        for (trigger, indices) in groups {
            let mut remainder_choices = Vec::with_capacity(indices.len());
            for index in indices {
                let (_, remainder, marker_event_rule_id) = &alternatives[index];
                let mut remainder_expr = self.compile_node(remainder, body_hint, false, true)?;
                if *marker_event_rule_id >= 0 {
                    let marker_event = self.builder.add_rule_ref(*marker_event_rule_id);
                    remainder_expr = self.builder.add_sequence(&[marker_event, remainder_expr]);
                }
                remainder_choices.push(remainder_expr);
            }
            let body = if remainder_choices.len() == 1 {
                remainder_choices[0]
            } else {
                self.builder.add_choices(&remainder_choices)
            };
            let body_rule = self.builder.add_rule_with_hint(body_hint, body);
            match trigger {
                LarkTrigger::String(trigger) => tag_rule_pairs.push((trigger, body_rule)),
                LarkTrigger::Token(token_ids) => trigger_rule_pairs.push((token_ids[0], body_rule)),
            }
        }
        self.dynamic_unused_rules = unused_rules;
        Ok(Some(if is_string_level {
            self.builder.add_tag_dispatch(&TagDispatch {
                tag_rule_pairs,
                loop_after_dispatch: true,
                excludes: Vec::new(),
            })
        } else {
            self.builder.add_token_tag_dispatch(&TokenTagDispatch {
                trigger_rule_pairs,
                loop_after_dispatch: true,
                excludes: Vec::new(),
            })
        }))
    }
}
