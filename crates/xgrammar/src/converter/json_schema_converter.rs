//! Converts a JSON Schema into a grammar AST, built directly through [`GrammarBuilder`].
//!
//! [`JsonSchemaConverter`] owns the conversion state and the grammar-building primitives. The
//! generation algorithm lives in [`SchemaConverter`], whose hooks the XML tool-calling converters
//! override.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use super::{
    cohere_xml_tool_calling_converter::CohereXmlToolCallingConverter,
    indent_manager::IndentManager,
    json_format::JsonFormat,
    key_trie::KeyTrie,
    minimax_m3_xml_tool_calling_converter::MiniMaxM3XmlToolCallingConverter,
    range_regex::{generate_float_range_regex_with_options, generate_range_regex},
    regex_converter::regex_to_ebnf,
    schema_converter::SchemaConverter,
    schema_error::SchemaError,
    schema_parser::{SchemaParser, serialize_json},
    schema_spec::{IntegerSpec, NumberSpec, SchemaSpecPtr, StringSpec},
    utf8_range_collapser::Utf8RangeCollapser,
    xml_tool_calling_converter::XmlToolCallingConverter,
};
use crate::{
    fsm::{FsmWithStartEnd, TrieFsmBuilder},
    functor::{GrammarFsmBuilder, add_sub_grammar, grammar_normalizer},
    grammar::{CharacterClassElement, Grammar, GrammarBuilder, GrammarExprType, TagDispatch},
    support::parse_json,
};

pub const BASIC_ANY: &str = "basic_any";
pub const BASIC_INTEGER: &str = "basic_integer";
pub const BASIC_NUMBER: &str = "basic_number";
pub const BASIC_STRING: &str = "basic_string";
pub const BASIC_BOOLEAN: &str = "basic_boolean";
pub const BASIC_NULL: &str = "basic_null";
pub const BASIC_ARRAY: &str = "basic_array";
pub const BASIC_OBJECT: &str = "basic_object";
pub const BASIC_ESCAPE: &str = "basic_escape";
pub const BASIC_STRING_SUB: &str = "basic_string_sub";

/// The content of a JSON string, escapes included.
pub const JSON_STRING_BODY_REGEX: &str = r#"([^"\\\x00-\x1f]|\\(["\\/bfnrt]|u[0-9a-fA-F]{4}))*"#;

/// The cache context of the plain JSON encoding.
pub const JSON_CACHE_CONTEXT: i32 = 1;

const WHITESPACE_CHARACTERS: [CharacterClassElement; 4] = [
    CharacterClassElement::new(b' ' as i32, b' ' as i32),
    CharacterClassElement::new(b'\n' as i32, b'\n' as i32),
    CharacterClassElement::new(b'\r' as i32, b'\r' as i32),
    CharacterClassElement::new(b'\t' as i32, b'\t' as i32),
];

/// Characters that may follow the end of a JSON string or key.
const CLOSING_CONTEXT: [CharacterClassElement; 4] = [
    CharacterClassElement::new(b',' as i32, b',' as i32),
    CharacterClassElement::new(b'}' as i32, b'}' as i32),
    CharacterClassElement::new(b']' as i32, b']' as i32),
    CharacterClassElement::new(b':' as i32, b':' as i32),
];

/// Characters that cannot appear unescaped in a JSON string.
const JSON_STRING_SPECIAL_CHARACTERS: [CharacterClassElement; 5] = [
    CharacterClassElement::new(0, 0x1f),
    CharacterClassElement::new(b'"' as i32, b'"' as i32),
    CharacterClassElement::new(b'\\' as i32, b'\\' as i32),
    CharacterClassElement::new(b'\r' as i32, b'\r' as i32),
    CharacterClassElement::new(b'\n' as i32, b'\n' as i32),
];

const ANY_CODEPOINT: [CharacterClassElement; 1] = [CharacterClassElement::new(0, 0x10_FFFF)];

impl Grammar {
    /// Constructs a BNF grammar from a JSON schema string.
    ///
    /// The schema string should be in the format of the schema of a JSON file. The schema is
    /// parsed and a BNF grammar is generated.
    ///
    /// `indent` is the number of spaces for indentation. If `None`, the output will be in one
    /// line. Default: 2.
    ///
    /// `separators` are the two separators used in the schema: comma and colon. Examples:
    /// `(",", ":")`, `(", ", ": ")`. If `None`, the default separators are used: `(",", ": ")`
    /// when `indent` is set, and `(", ", ": ")` otherwise. This follows the convention in Python
    /// `json.dumps()`.
    ///
    /// `strict_mode`, when true, does not allow properties and items that are not specified in
    /// the schema. This is equivalent to setting `unevaluatedProperties` and
    /// `unevaluatedItems` to false. This helps LLMs generate accurate output in grammar-guided
    /// generation with JSON schema.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the schema is invalid or unsatisfiable.
    pub fn from_json_schema(
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
    ) -> Result<Grammar, SchemaError> {
        Self::from_json_schema_with_any_order(
            schema,
            any_whitespace,
            indent,
            separators,
            strict_mode,
            max_whitespace_cnt,
            false,
        )
    }

    /// Constructs a BNF grammar from a JSON schema string, optionally allowing object
    /// properties in any order.
    ///
    /// When `any_order` is true, object properties may appear in any order; only key validity
    /// and each key's value schema are enforced.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the schema is invalid or unsatisfiable.
    pub fn from_json_schema_with_any_order(
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        any_order: bool,
    ) -> Result<Grammar, SchemaError> {
        Self::from_json_schema_with_options(
            schema,
            any_whitespace,
            indent,
            separators,
            strict_mode,
            max_whitespace_cnt,
            false,
            any_order,
        )
    }

    /// Constructs a BNF grammar from a JSON schema string with full formatting options.
    ///
    /// If `print_converted_ebnf` is true, the converted grammar is printed (for debugging).
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the schema is invalid or unsatisfiable.
    #[allow(clippy::too_many_arguments)]
    pub fn from_json_schema_with_options(
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        print_converted_ebnf: bool,
        any_order: bool,
    ) -> Result<Grammar, SchemaError> {
        let grammar = grammar_normalizer(&json_schema_to_grammar(
            schema,
            any_whitespace,
            indent,
            separators,
            strict_mode,
            max_whitespace_cnt,
            any_order,
            JsonFormat::Json,
            &[],
        )?);
        if print_converted_ebnf {
            println!("Converted EBNF: {grammar}");
        }
        Ok(grammar)
    }
}

/// Converts a JSON Schema string to its EBNF grammar text (the normalized grammar, printed).
///
/// `any_whitespace` allows flexible whitespace; `indent`/`separators` control formatting;
/// `strict_mode` forbids unspecified additional items/properties; `max_whitespace_cnt` bounds
/// whitespace runs; `json_format` selects the encoding of the root object; `any_order` lets
/// object properties appear in any order.
///
/// # Errors
/// Returns a [`SchemaError`] if the schema is invalid or unsatisfiable.
#[allow(clippy::too_many_arguments)]
pub fn json_schema_to_ebnf(
    schema: &str,
    any_whitespace: bool,
    indent: Option<i32>,
    separators: Option<(&str, &str)>,
    strict_mode: bool,
    max_whitespace_cnt: Option<i32>,
    json_format: JsonFormat,
    any_order: bool,
) -> Result<String, SchemaError> {
    let grammar = json_schema_to_grammar(
        schema,
        any_whitespace,
        indent,
        separators,
        strict_mode,
        max_whitespace_cnt,
        any_order,
        json_format,
        &[],
    )?;
    Ok(grammar_normalizer(&grammar).to_string())
}

/// Converts a JSON Schema string to an unnormalized grammar AST. `excludes` lists substrings that
/// may not appear in unconstrained string values and property names.
///
/// # Errors
/// Returns a [`SchemaError`] if the schema is invalid, unsatisfiable, or unsupported by
/// `json_format`.
#[allow(clippy::too_many_arguments)]
pub fn json_schema_to_grammar(
    schema: &str,
    any_whitespace: bool,
    indent: Option<i32>,
    separators: Option<(&str, &str)>,
    strict_mode: bool,
    max_whitespace_cnt: Option<i32>,
    any_order: bool,
    json_format: JsonFormat,
    excludes: &[String],
) -> Result<Grammar, SchemaError> {
    let root = parse_json(schema).map_err(|e| SchemaError::invalid(format!("Failed to parse JSON: {e}")))?;
    if max_whitespace_cnt.is_some_and(|count| count <= 0) {
        return Err(SchemaError::invalid("max_whitespace_cnt must be positive."));
    }
    let mut parser = SchemaParser::new(root.clone(), strict_mode);
    let spec = parser.parse(&root, None)?;
    let excludes = excludes.to_vec();
    if !excludes.is_empty() && matches!(json_format, JsonFormat::MiniMaxM3Xml | JsonFormat::CohereXml) {
        let name = if json_format == JsonFormat::CohereXml {
            "cohere_xml"
        } else {
            "minimax_m3_xml"
        };
        return Err(SchemaError::invalid(format!("JSONSchemaFormat.excludes is not supported for {name}")));
    }
    let base =
        JsonSchemaConverter::new(parser, indent, separators, any_whitespace, max_whitespace_cnt, any_order, excludes)?;
    match json_format {
        JsonFormat::Json => base.into_grammar(&spec),
        JsonFormat::MiniMaxM3Xml => MiniMaxM3XmlToolCallingConverter::new(base).into_grammar(&spec),
        JsonFormat::CohereXml => CohereXmlToolCallingConverter::new(base)?.into_grammar(&spec),
        _ => XmlToolCallingConverter::new(base, json_format)?.into_grammar(&spec),
    }
}

/// The conversion state shared by every JSON-schema converter: the grammar builder, formatting
/// options, the rule caches, and the `$ref` resolver. Its methods build grammar expressions and
/// reuse identical leaf nodes.
pub struct JsonSchemaConverter {
    pub builder: GrammarBuilder,
    pub indent_manager: IndentManager,
    pub colon_expr_id: i32,
    pub any_whitespace: bool,
    pub max_whitespace_cnt: Option<i32>,
    /// Object properties may appear in any order (applies to nested objects too).
    pub any_order: bool,
    /// Substrings forbidden in unconstrained strings and property names.
    pub excludes: Vec<String>,
    /// Generated rules keyed by schema cache key and encoding context.
    pub rule_cache: HashMap<(String, i32), i32>,
    /// `$ref` rules keyed by the converter's reference cache key, for recursive references.
    pub uri_to_rule_id: HashMap<String, i32>,
    parser: SchemaParser,
    empty_expr_id: Option<i32>,
    unsatisfiable_expr_id: Option<i32>,
    whitespace_expr_id: Option<i32>,
    byte_string_expr_ids: HashMap<Vec<u8>, i32>,
    rule_ref_expr_ids: HashMap<i32, i32>,
}

impl SchemaConverter for JsonSchemaConverter {
    fn base(&self) -> &JsonSchemaConverter {
        self
    }

    fn base_mut(&mut self) -> &mut JsonSchemaConverter {
        self
    }
}

impl JsonSchemaConverter {
    /// Creates the converter state.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the separators cannot be encoded.
    pub fn new(
        parser: SchemaParser,
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        any_whitespace: bool,
        max_whitespace_cnt: Option<i32>,
        any_order: bool,
        excludes: Vec<String>,
    ) -> Result<Self, SchemaError> {
        let item_separator = match separators {
            Some((item, _)) => item,
            None if any_whitespace || indent.is_some() => ",",
            None => ", ",
        };
        let colon_separator = match separators {
            Some((_, colon)) => colon,
            None if any_whitespace => ":",
            None => ": ",
        };
        let mut converter = Self {
            builder: GrammarBuilder::new(),
            indent_manager: IndentManager::new(indent, item_separator, any_whitespace, max_whitespace_cnt),
            colon_expr_id: -1,
            any_whitespace,
            max_whitespace_cnt,
            any_order,
            excludes,
            rule_cache: HashMap::new(),
            uri_to_rule_id: HashMap::new(),
            parser,
            empty_expr_id: None,
            unsatisfiable_expr_id: None,
            whitespace_expr_id: None,
            byte_string_expr_ids: HashMap::new(),
            rule_ref_expr_ids: HashMap::new(),
        };
        let colon = if any_whitespace {
            let whitespace = converter.whitespace_pattern();
            format!("{whitespace} \"{colon_separator}\" {whitespace}")
        } else {
            format!("\"{colon_separator}\"")
        };
        converter.colon_expr_id = converter.formatting_expression(&colon)?;
        Ok(converter)
    }

    /// Resolves a `$ref` URI to its parsed schema.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the referenced schema is missing or invalid.
    pub fn resolve_ref(
        &mut self,
        uri: &str,
    ) -> Result<SchemaSpecPtr, SchemaError> {
        self.parser.resolve_ref(uri)
    }

    /// Fills the bodies of the helper rules `basic_escape` and `basic_string_sub`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the exclusions cannot be applied to string bodies.
    pub fn add_helper_rules(&mut self) -> Result<(), SchemaError> {
        if self.max_whitespace_cnt.is_some() {
            // Keep the historical helper-rule numbering after grammar optimization.
            let empty = self.empty();
            self.builder.add_rule_with_hint(BASIC_STRING_SUB, empty);
        }
        let escaped_character = self.builder.add_character_class(&ascii_class(b"\"\\/bfnrt"), false);
        let hexadecimal_character = self.builder.add_character_class(
            &[
                CharacterClassElement::new(0x41, 0x46),
                CharacterClassElement::new(0x61, 0x66),
                CharacterClassElement::new(0x30, 0x39),
            ],
            false,
        );
        let u = self.byte_string(b"u");
        let unicode_escape = self.sequence(&[
            u,
            hexadecimal_character,
            hexadecimal_character,
            hexadecimal_character,
            hexadecimal_character,
        ]);
        let escape_body = self.choice(&[escaped_character, unicode_escape]);
        self.builder.update_rule_body_by_name(BASIC_ESCAPE, escape_body);

        let normal_character = self.builder.add_character_class(&JSON_STRING_SPECIAL_CHARACTERS, true);
        let string_sub_ref = self.rule_ref_by_name(BASIC_STRING_SUB);
        let quote = self.byte_string(b"\"");
        let with_normal = self.sequence(&[normal_character, string_sub_ref]);
        let backslash = self.byte_string(b"\\");
        let escape_ref = self.rule_ref_by_name(BASIC_ESCAPE);
        let with_escape = self.sequence(&[backslash, escape_ref, string_sub_ref]);
        let string_sub_body = self.choice(&[quote, with_normal, with_escape]);
        self.builder.update_rule_body_by_name(BASIC_STRING_SUB, string_sub_body);
        if !self.excludes.is_empty() {
            let body = self.excluding_string(JSON_STRING_BODY_REGEX, BASIC_STRING_SUB, true, &[])?;
            self.builder.update_rule_body_by_name(BASIC_STRING_SUB, body);
        }
        let lookahead = self.closing_lookahead();
        self.builder.update_lookahead_assertion_by_name(BASIC_STRING_SUB, lookahead);
        Ok(())
    }

    /// The lookahead `[ \n\r\t]* [,}\]:]` that ends a JSON string or key.
    pub fn closing_lookahead(&mut self) -> i32 {
        let whitespace = self.whitespace_expression();
        let closing_context = self.builder.add_character_class(&CLOSING_CONTEXT, false);
        self.sequence(&[whitespace, closing_context])
    }

    /// The empty-string expression (shared).
    pub fn empty(&mut self) -> i32 {
        if let Some(id) = self.empty_expr_id {
            return id;
        }
        let id = self.builder.add_empty_str();
        self.empty_expr_id = Some(id);
        id
    }

    /// An expression matching nothing, for alternatives ruled out by the exclusions (shared).
    pub fn unsatisfiable(&mut self) -> i32 {
        if let Some(id) = self.unsatisfiable_expr_id {
            return id;
        }
        let id = self.builder.add_character_class(&ANY_CODEPOINT, true);
        self.unsatisfiable_expr_id = Some(id);
        id
    }

    /// A byte-string expression (shared per value; the empty string maps to [`Self::empty`]).
    pub fn byte_string(
        &mut self,
        value: &[u8],
    ) -> i32 {
        if let Some(&id) = self.byte_string_expr_ids.get(value) {
            return id;
        }
        let id = if value.is_empty() {
            self.empty()
        } else {
            let bytes: Vec<i32> = value.iter().map(|&byte| i32::from(byte)).collect();
            self.builder.add_byte_string_bytes(&bytes)
        };
        self.byte_string_expr_ids.insert(value.to_vec(), id);
        id
    }

    /// A tag-dispatch expression without tags: any text that contains none of `excludes`.
    pub fn tag_dispatch(
        &mut self,
        loop_after_dispatch: bool,
        excludes: &[String],
    ) -> i32 {
        self.builder.add_tag_dispatch(&TagDispatch {
            tag_rule_pairs: Vec::new(),
            loop_after_dispatch,
            excludes: excludes.iter().map(|exclude| exclude.as_bytes().to_vec()).collect(),
        })
    }

    /// A reference to the rule `rule_id` (shared per rule).
    pub fn rule_ref(
        &mut self,
        rule_id: i32,
    ) -> i32 {
        if let Some(&id) = self.rule_ref_expr_ids.get(&rule_id) {
            return id;
        }
        let id = self.builder.add_rule_ref(rule_id);
        self.rule_ref_expr_ids.insert(rule_id, id);
        id
    }

    /// A reference to one of the converter's own rules, by name.
    pub fn rule_ref_by_name(
        &mut self,
        rule_name: &str,
    ) -> i32 {
        let rule_id = self.builder.get_rule_id(rule_name);
        debug_assert!(rule_id >= 0, "rule {rule_name} is not allocated");
        self.rule_ref(rule_id)
    }

    /// A sequence; a single element is returned as is and no element is the empty string.
    pub fn sequence(
        &mut self,
        elements: &[i32],
    ) -> i32 {
        match elements {
            [] => self.empty(),
            [element] => *element,
            _ => self.builder.add_sequence(elements),
        }
    }

    /// An alternation; a single choice is returned as is and no choice is the empty string.
    pub fn choice(
        &mut self,
        choices: &[i32],
    ) -> i32 {
        match choices {
            [] => self.empty(),
            [choice] => *choice,
            _ => self.builder.add_choices(choices),
        }
    }

    /// `expr_id` repeated `[min_count, max_count]` times (`-1` is unbounded).
    pub fn repeat(
        &mut self,
        rule_name_hint: &str,
        expr_id: i32,
        min_count: i32,
        max_count: i32,
    ) -> i32 {
        match (min_count, max_count) {
            (0, 0) => return self.empty(),
            (1, 1) => return expr_id,
            (0, 1) => {
                let empty = self.empty();
                return self.choice(&[empty, expr_id]);
            },
            (0, -1) => {
                let (ty, data) = {
                    let expr = self.builder.grammar_expr(expr_id);
                    (expr.ty, expr.data.to_vec())
                };
                if ty == GrammarExprType::CharacterClass {
                    return self.builder.add_grammar_expr(GrammarExprType::CharacterClassStar, &data);
                }
            },
            _ => {},
        }
        self.builder.add_repeat_from_expr(rule_name_hint, expr_id, min_count, max_count)
    }

    /// Copies `grammar` into the builder and returns a reference to its root rule.
    pub fn add_sub_grammar(
        &mut self,
        grammar: &Grammar,
    ) -> i32 {
        let rule_id = add_sub_grammar(&mut self.builder, grammar);
        self.rule_ref(rule_id)
    }

    /// The EBNF spelling of one whitespace run.
    pub fn whitespace_pattern(&self) -> String {
        IndentManager::whitespace_pattern(self.max_whitespace_cnt)
    }

    /// One whitespace run. Unbounded runs share one expression; bounded runs stay distinct rules.
    pub fn whitespace_expression(&mut self) -> i32 {
        let Some(max_whitespace_cnt) = self.max_whitespace_cnt else {
            if let Some(id) = self.whitespace_expr_id {
                return id;
            }
            let id = self.builder.add_character_class_star(&WHITESPACE_CHARACTERS, false);
            self.whitespace_expr_id = Some(id);
            return id;
        };
        let whitespace = self.builder.add_character_class(&WHITESPACE_CHARACTERS, false);
        self.repeat("whitespace", whitespace, 0, max_whitespace_cnt)
    }

    /// Converts a separator fragment from [`IndentManager`] (a whitespace run, a JSON string
    /// literal, or a literal padded by whitespace runs) into an expression.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a separator is not a valid JSON string literal.
    pub fn formatting_expression(
        &mut self,
        expression: &str,
    ) -> Result<i32, SchemaError> {
        let whitespace = self.whitespace_pattern();
        if expression == whitespace {
            return Ok(self.whitespace_expression());
        }
        let prefix = format!("{whitespace} ");
        let suffix = format!(" {whitespace}");
        if expression.len() >= prefix.len() + suffix.len()
            && expression.starts_with(&prefix)
            && expression.ends_with(&suffix)
        {
            let leading = self.whitespace_expression();
            let inner = self.formatting_expression(&expression[prefix.len()..expression.len() - suffix.len()])?;
            let trailing = self.whitespace_expression();
            return Ok(self.sequence(&[leading, inner, trailing]));
        }
        let literal: String = serde_json::from_str(expression)
            .map_err(|_| SchemaError::invalid(format!("Unsupported indentation expression: {expression}")))?;
        Ok(self.byte_string(literal.as_bytes()))
    }

    /// The grammar expression matching `regex`. Prefers a `Regex` node compiled into one
    /// automaton; `json_string` excludes the characters that must be escaped in a JSON string from
    /// every character match. Falls back to the CFG expansion when forced, when the automaton
    /// engine cannot build the pattern, or when the exclusion leaves the language empty.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the regex is invalid.
    pub fn regex_expression(
        &mut self,
        regex: &str,
        json_string: bool,
        force_cfg_expansion: bool,
    ) -> Result<i32, SchemaError> {
        let can_use_fsm =
            !force_cfg_expansion && (!json_string || regex.bytes().all(|byte| (0x20..=0x7e).contains(&byte)));
        if can_use_fsm && regex_language_is_nonempty(regex, json_string) {
            return Ok(self.builder.add_regex(regex.as_bytes(), json_string));
        }
        let ebnf = regex_to_ebnf(regex, true).map_err(|e| SchemaError::invalid(e.to_string()))?;
        let grammar = Grammar::from_ebnf(&ebnf, "root").map_err(|e| SchemaError::invalid(e.to_string()))?;
        Ok(self.add_sub_grammar(&grammar))
    }

    /// Logs that a string's length constraints are dropped because exclusions are set.
    pub fn warn_dropped_length_constraints(
        spec: &StringSpec,
        rule_name: &str,
    ) {
        eprintln!(
            "Ignoring the length constraints of string {rule_name} (minLength={}, maxLength={}): they are not \
             applied together with JSONSchemaFormat.excludes",
            spec.min_length, spec.max_length
        );
    }

    /// Whether `text` contains none of the exclusions.
    pub fn is_allowed_string(
        &self,
        text: &str,
    ) -> bool {
        !self.excludes.iter().any(|excluded| text.contains(excluded.as_str()))
    }

    /// Whether the serialized JSON literal contains none of the exclusions. Strings are checked
    /// as their escaped JSON content, or as the raw text when `raw_string` is set; arrays and
    /// objects are checked element-wise and key-wise.
    pub fn is_allowed_json_literal(
        &self,
        json_value: &str,
        raw_string: bool,
    ) -> bool {
        self.excludes.is_empty() || self.is_allowed_literal(&parse_literal(json_value), raw_string)
    }

    fn is_allowed_literal(
        &self,
        value: &Value,
        raw_string: bool,
    ) -> bool {
        match value {
            Value::String(text) if raw_string => self.is_allowed_string(text),
            Value::String(text) => self.is_allowed_string(&json_string_content(text)),
            Value::Array(items) => items.iter().all(|item| self.is_allowed_literal(item, false)),
            Value::Object(members) => members.iter().all(|(key, item)| {
                self.is_allowed_string(&json_string_content(key)) && self.is_allowed_literal(item, false)
            }),
            _ => true,
        }
    }

    /// The fixed spelling of a JSON object key, or an unsatisfiable expression when the key
    /// contains an exclusion.
    pub fn json_property_key(
        &mut self,
        key: &str,
    ) -> i32 {
        if !self.is_allowed_string(&json_string_content(key)) {
            return self.unsatisfiable();
        }
        let encoded = serialize_json(&Value::String(key.to_owned()));
        self.byte_string(encoded.as_bytes())
    }

    /// A JSON object member: `key`, the colon separator, and the value rule.
    pub fn json_member(
        &mut self,
        key: i32,
        value_rule_id: i32,
    ) -> i32 {
        let colon = self.colon_expr_id;
        let value = self.rule_ref(value_rule_id);
        self.sequence(&[key, colon, value])
    }

    /// `pattern` repeated so that the total count stays within `[min_properties, max_properties]`
    /// after `already_repeated_times` occurrences.
    pub fn property_with_number_constraints(
        &mut self,
        pattern: i32,
        min_properties: i32,
        max_properties: i32,
        already_repeated_times: i32,
        rule_name: &str,
    ) -> i32 {
        if max_properties != -1 && max_properties == already_repeated_times {
            return self.empty();
        }
        let lower = (min_properties - already_repeated_times).max(0);
        let upper = if max_properties == -1 {
            -1
        } else {
            (max_properties - already_repeated_times).max(-1)
        };
        self.repeat(&format!("{rule_name}_properties"), pattern, lower, upper)
    }

    /// A JSON key (from its opening quote) that is none of `names`. Only the literal unescaped
    /// spelling of each name is excluded.
    pub fn key_excluding_names(
        &mut self,
        names: &[&str],
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let quote = self.byte_string(b"\"");
        if !self.excludes.is_empty() {
            let keys: Vec<String> = names.iter().map(|name| json_string_content(name)).collect();
            let body = self.excluding_string(JSON_STRING_BODY_REGEX, &format!("{rule_name}_addl_key"), true, &keys)?;
            return Ok(self.sequence(&[quote, body]));
        }
        let trie = KeyTrie::new(names.iter().map(|name| name.bytes()));
        let key_rule_id = self.builder.add_empty_rule_with_hint(&format!("{rule_name}_addl_key"));
        let trie_body = self.key_trie_body(&trie);
        let body = self.sequence(&[quote, trie_body]);
        self.builder.update_rule_body(key_rule_id, body);
        let lookahead = self.closing_lookahead();
        self.builder.update_lookahead_assertion(key_rule_id, lookahead);
        Ok(self.rule_ref(key_rule_id))
    }

    fn key_trie_body(
        &mut self,
        node: &KeyTrie<u8>,
    ) -> i32 {
        let mut choices = Vec::new();
        if !node.is_terminal {
            choices.push(self.byte_string(b"\""));
        }
        let mut excluded = JSON_STRING_SPECIAL_CHARACTERS.to_vec();
        excluded.extend(node.children.keys().map(|&byte| CharacterClassElement::new(i32::from(byte), i32::from(byte))));
        let other_character = self.builder.add_character_class(&excluded, true);
        let string_sub = self.rule_ref_by_name(BASIC_STRING_SUB);
        choices.push(self.sequence(&[other_character, string_sub]));
        let backslash = self.byte_string(b"\\");
        let escape = self.rule_ref_by_name(BASIC_ESCAPE);
        choices.push(self.sequence(&[backslash, escape, string_sub]));
        for (&byte, child) in &node.children {
            let character = self.byte_string(&[byte]);
            let rest = self.key_trie_body(child);
            choices.push(self.sequence(&[character, rest]));
        }
        self.choice(&choices)
    }

    /// Rules matching `regex` (one of the converter's own ASCII string bodies) minus every string
    /// containing an exclusion, and minus `excluded_keys` as whole strings. With
    /// `close_json_string` the closing quote follows the filtered body, so it is never part of an
    /// exclusion.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the exclusion automaton cannot be built.
    pub fn excluding_string(
        &mut self,
        regex: &str,
        rule_name: &str,
        close_json_string: bool,
        excluded_keys: &[String],
    ) -> Result<i32, SchemaError> {
        let fsm_error =
            |message: String| SchemaError::invalid(format!("Cannot apply JSONSchemaFormat.excludes: {message}"));
        let parsed = GrammarFsmBuilder::regex(regex.as_bytes(), false).map_err(fsm_error)?;
        let exclusion = GrammarFsmBuilder::tag_dispatch(&TagDispatch {
            tag_rule_pairs: Vec::new(),
            loop_after_dispatch: false,
            excludes: self.excludes.iter().map(|excluded| excluded.as_bytes().to_vec()).collect(),
        })
        .ok_or_else(|| SchemaError::invalid("Invalid JSONSchemaFormat.excludes"))?;
        let mut fsm = FsmWithStartEnd::intersect(&parsed, &exclusion).map_err(fsm_error)?;
        if !excluded_keys.is_empty() {
            let keys: Vec<&[u8]> = excluded_keys.iter().map(String::as_bytes).collect();
            let other_keys = TrieFsmBuilder::build(&keys, &[], None, true, false)
                .ok_or_else(|| fsm_error("cannot build the key trie".to_owned()))?
                .not()
                .map_err(fsm_error)?;
            fsm = FsmWithStartEnd::intersect(&fsm, &other_keys).map_err(fsm_error)?;
        }
        // Do not emit paths that can never complete after exclusions: a matcher could accept a
        // forbidden alternative's prefix and reach an all-rejected mask later.
        let mut collapser = Utf8RangeCollapser::new(&fsm);
        if !collapser.is_productive(fsm.start()) {
            return Ok(self.unsatisfiable());
        }

        let mut rules = vec![-1; fsm.num_states() as usize];
        let mut pending = vec![fsm.start()];
        // Share complete fallback branches across key prefixes to reuse their token masks. The
        // key is the target state followed by every codepoint interval.
        let mut shared_key_continuations: BTreeMap<Vec<i32>, i32> = BTreeMap::new();
        rules[fsm.start() as usize] = self.builder.add_empty_rule_with_hint(&format!("{rule_name}_exclude"));
        let mut index = 0;
        while index < pending.len() {
            let state = pending[index];
            index += 1;
            let mut transitions: BTreeMap<i32, Vec<CharacterClassElement>> = BTreeMap::new();
            for [target, min, max] in collapser.state_ranges(state) {
                transitions.entry(target).or_default().push(CharacterClassElement::new(min, max));
            }
            let mut choices = Vec::new();
            // Keep the closing quote on the accepting body states: a nullable body rule would
            // finish at every character. The quote is JSON syntax, not filtered string content.
            if fsm.is_end_state(state) {
                choices.push(if close_json_string {
                    self.byte_string(b"\"")
                } else {
                    self.empty()
                });
            }
            for (target, codepoints) in transitions {
                if rules[target as usize] == -1 {
                    rules[target as usize] = self.builder.add_empty_rule_with_hint(&format!("{rule_name}_exclude"));
                    pending.push(target);
                }
                let single_ascii =
                    codepoints.len() == 1 && codepoints[0].lower == codepoints[0].upper && codepoints[0].upper <= 0x7f;
                // Keep self-loops local for the compiler's speculative string-mask fast path.
                if !excluded_keys.is_empty() && !single_ascii && target != state {
                    let mut key = Vec::with_capacity(1 + codepoints.len() * 2);
                    key.push(target);
                    key.extend(codepoints.iter().flat_map(|range| [range.lower, range.upper]));
                    let continuation = match shared_key_continuations.get(&key) {
                        Some(&rule_id) => rule_id,
                        None => {
                            let class = self.builder.add_character_class(&codepoints, false);
                            let next = self.rule_ref(rules[target as usize]);
                            let body = self.sequence(&[class, next]);
                            let rule_id =
                                self.builder.add_rule_with_hint(&format!("{rule_name}_exclude_continuation"), body);
                            shared_key_continuations.insert(key, rule_id);
                            rule_id
                        },
                    };
                    choices.push(self.rule_ref(continuation));
                } else {
                    let class = self.builder.add_character_class(&codepoints, false);
                    let next = self.rule_ref(rules[target as usize]);
                    choices.push(self.sequence(&[class, next]));
                }
            }
            if choices.is_empty() {
                choices.push(self.unsatisfiable());
            }
            let body = self.choice(&choices);
            self.builder.update_rule_body(rules[state as usize], body);
        }
        Ok(self.rule_ref(rules[fsm.start() as usize]))
    }

    /// `integer`: a range, a `multipleOf` constraint, or any JSON integer.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the range regex cannot be expanded.
    pub fn generate_integer(
        &mut self,
        spec: &IntegerSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let (start, end) = spec.effective_range();
        if let Some(multiple_of) = spec.multiple_of {
            // The parser keeps multipleOf only for fully bounded ranges (enumerated here) and fully
            // unbounded ones (a modulo automaton).
            if let (Some(start), Some(end)) = (start, end) {
                let mut multiples = Vec::new();
                for value in start..=end {
                    if value % multiple_of == 0 {
                        multiples.push(self.byte_string(value.to_string().as_bytes()));
                    }
                }
                return Ok(self.choice(&multiples));
            }
            return Ok(self.integer_multiple_of(multiple_of, rule_name));
        }
        if start.is_some() || end.is_some() {
            return self.regex_expression(&generate_range_regex(start, end), false, true);
        }
        let empty = self.empty();
        let minus = self.byte_string(b"-");
        let optional_minus = self.choice(&[empty, minus]);
        let zero = self.byte_string(b"0");
        let leading_digit = self.builder.add_character_class(&[CharacterClassElement::new(0x31, 0x39)], false);
        let digits = self.builder.add_character_class_star(&[CharacterClassElement::new(0x30, 0x39)], false);
        let nonzero = self.sequence(&[optional_minus, leading_digit, digits]);
        Ok(self.choice(&[zero, nonzero]))
    }

    /// The decimal integers divisible by `multiple_of`, as an automaton over the remainder.
    fn integer_multiple_of(
        &mut self,
        multiple_of: i64,
        rule_name: &str,
    ) -> i32 {
        let states: Vec<i32> = (0..multiple_of)
            .map(|state| {
                self.builder.add_empty_rule_with_hint(&format!("{rule_name}_multiple_of_{multiple_of}_mod_{state}"))
            })
            .collect();
        for state in 0..multiple_of {
            let mut transitions = Vec::with_capacity(11);
            if state == 0 {
                transitions.push(self.empty());
            }
            for digit in 0..=9 {
                let next = states[((state * 10 + digit) % multiple_of) as usize];
                let digit = self.byte_string(digit.to_string().as_bytes());
                let next = self.rule_ref(next);
                transitions.push(self.sequence(&[digit, next]));
            }
            let body = self.choice(&transitions);
            self.builder.update_rule_body(states[state as usize], body);
        }
        let mut non_zero_starts = Vec::with_capacity(9);
        for digit in 1..=9 {
            let next = states[(digit % multiple_of) as usize];
            let digit = self.byte_string(digit.to_string().as_bytes());
            let next = self.rule_ref(next);
            non_zero_starts.push(self.sequence(&[digit, next]));
        }
        let zero = self.byte_string(b"0");
        let empty = self.empty();
        let minus = self.byte_string(b"-");
        let optional_minus = self.choice(&[empty, minus]);
        let non_zero = self.choice(&non_zero_starts);
        let signed = self.sequence(&[optional_minus, non_zero]);
        self.choice(&[zero, signed])
    }

    /// `number`: a range or any JSON number.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the range regex cannot be expanded.
    pub fn generate_number(
        &mut self,
        spec: &NumberSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let mut start = spec.minimum;
        let mut end = spec.maximum;
        let mut exclusive_start = false;
        let mut exclusive_end = false;
        // The larger lower bound wins; on a tie the exclusive one is stricter.
        if let Some(exclusive) = spec.exclusive_minimum {
            if start.is_none_or(|start| exclusive >= start) {
                start = Some(exclusive);
                exclusive_start = true;
            }
        }
        if let Some(exclusive) = spec.exclusive_maximum {
            if end.is_none_or(|end| exclusive <= end) {
                end = Some(exclusive);
                exclusive_end = true;
            }
        }
        if start.is_some() || end.is_some() {
            let regex = generate_float_range_regex_with_options(start, end, exclusive_start, exclusive_end);
            return self.regex_expression(&regex, false, true);
        }
        let empty = self.empty();
        let minus = self.byte_string(b"-");
        let optional_minus = self.choice(&[empty, minus]);
        let zero = self.byte_string(b"0");
        let leading_digit = self.builder.add_character_class(&[CharacterClassElement::new(0x31, 0x39)], false);
        let digits = self.builder.add_character_class_star(&[CharacterClassElement::new(0x30, 0x39)], false);
        let nonzero = self.sequence(&[leading_digit, digits]);
        let integer_part = self.choice(&[zero, nonzero]);
        let digit = self.builder.add_character_class(&[CharacterClassElement::new(0x30, 0x39)], false);
        let one_or_more_digits = self.repeat(&format!("{rule_name}_digits"), digit, 1, -1);
        let dot = self.byte_string(b".");
        let fraction_digits = self.sequence(&[dot, one_or_more_digits]);
        let fraction = self.choice(&[empty, fraction_digits]);
        let exponent_mark = self.builder.add_character_class(&ascii_class(b"eE"), false);
        let sign = self.builder.add_character_class(&ascii_class(b"+-"), false);
        let optional_sign = self.choice(&[empty, sign]);
        let exponent_digits = self.sequence(&[exponent_mark, optional_sign, one_or_more_digits]);
        let exponent = self.choice(&[empty, exponent_digits]);
        // "-"? ("0" | ...) admits -0; ("0" | "-"? ...) would not.
        Ok(self.sequence(&[optional_minus, integer_part, fraction, exponent]))
    }

    /// `boolean`.
    pub fn generate_boolean(&mut self) -> i32 {
        let true_literal = self.byte_string(b"true");
        let false_literal = self.byte_string(b"false");
        self.choice(&[true_literal, false_literal])
    }

    /// `null`.
    pub fn generate_null(&mut self) -> i32 {
        self.byte_string(b"null")
    }

    /// Any codepoint repeated `[min_length, max_length]` times, the body of a raw string with a
    /// length bound.
    pub fn raw_characters(
        &mut self,
        spec: &StringSpec,
        rule_name: &str,
    ) -> i32 {
        let character = self.builder.add_character_class(&ANY_CODEPOINT, false);
        self.repeat(&format!("{rule_name}_characters"), character, spec.min_length, spec.max_length)
    }
}

/// The JSON-schema `format` → regex table. A recognized format is compiled to its regex, which
/// takes precedence over `minLength`/`maxLength`.
pub fn json_format_to_regex_pattern(format: &str) -> Option<String> {
    let atext = "[\\w!#$%&'*+/=?^`{|}~-]";
    let dot_string = format!("({atext}+(\\.{atext}+)*)");
    let quoted_string = "\\\\\"(\\\\[\\x20-\\x7E]|[\\x20\\x21\\x23-\\x5B\\x5D-\\x7E])*\\\\\"";
    let domain = "([A-Za-z0-9]([\\-A-Za-z0-9]*[A-Za-z0-9])?)((\\.[A-Za-z0-9][\\-A-Za-z0-9]*[A-Za-z0-9])*)";

    let pat = match format {
        "email" => format!("^({dot_string}|{quoted_string})@{domain}$"),
        "date" => "^(\\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[1-2]\\d|3[01]))$".to_owned(),
        "time" => {
            "^([01]\\d|2[0-3]):[0-5]\\d:([0-5]\\d|60)(\\.\\d+)?(Z|[+-]([01]\\d|2[0-3]):[0-5]\\d)$"
                .to_owned()
        }
        "date-time" => "^(\\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[1-2]\\d|3[01]))T([01]\\d|2[0-3]):[0-5]\\d:([0-5]\\d|60)(\\.\\d+)?(Z|[+-]([01]\\d|2[0-3]):[0-5]\\d)$".to_owned(),
        "duration" => "^P((\\d+D|\\d+M(\\d+D)?|\\d+Y(\\d+M(\\d+D)?)?)(T(\\d+S|\\d+M(\\d+S)?|\\d+H(\\d+M(\\d+S)?)?))?|T(\\d+S|\\d+M(\\d+S)?|\\d+H(\\d+M(\\d+S)?)?)|\\d+W)$".to_owned(),
        "ipv4" => {
            let decbyte = "(25[0-5]|2[0-4]\\d|[0-1]?\\d?\\d)";
            format!("^({decbyte}\\.){{3}}{decbyte}$")
        }
        "ipv6" => "(([0-9a-fA-F]{1,4}:){7,7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:)|::(ffff(:0{1,4}){0,1}:){0,1}((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9]))".to_owned(),
        "hostname" => "^([a-z0-9]([a-z0-9-]*[a-z0-9])?)(\\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$".to_owned(),
        "uuid" => "^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$".to_owned(),
        "uri" => {
            let schema_pat = "[a-zA-Z][a-zA-Z+\\.-]*";
            let pchar = "([\\w\\.~!$&'()*+,;=:@-]|%[0-9A-Fa-f][0-9A-Fa-f])";
            let query_fragment_char = "([\\w\\.~!$&'()*+,;=:@/\\?-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let query = format!("(\\?{query_fragment_char})?");
            let fragment = format!("(#{query_fragment_char})?");
            let path_abempty = format!("(/{pchar}*)*");
            let path_absolute_rootless_empty = format!("/?({pchar}+(/{pchar}*)*)?");
            let userinfo = "([\\w\\.~!$&'()*+,;=:-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let host = "([\\w\\.~!$&'()*+,;=-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let authority = format!("({userinfo}@)?{host}(:\\d*)?");
            let hier_part = format!("(//{authority}{path_abempty}|{path_absolute_rootless_empty})");
            format!("^{schema_pat}:{hier_part}{query}{fragment}$")
        }
        "uri-reference" => {
            let pchar = "([\\w\\.~!$&'()*+,;=:@-]|%[0-9A-Fa-f][0-9A-Fa-f])";
            let query_fragment_char = "([\\w\\.~!$&'()*+,;=:@/\\?-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let query = format!("(\\?{query_fragment_char})?");
            let fragment = format!("(#{query_fragment_char})?");
            let path_abempty = format!("(/{pchar}*)*");
            let path_absolute = format!("/({pchar}+(/{pchar}*)*)?");
            let segment_nz_nc = "([\\w\\.~!$&'()*+,;=@-]|%[0-9A-Fa-f][0-9A-Fa-f])+";
            let path_noscheme = format!("{segment_nz_nc}(/{pchar}*)*");
            let userinfo = "([\\w\\.~!$&'()*+,;=:-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let host = "([\\w\\.~!$&'()*+,;=-]|%[0-9A-Fa-f][0-9A-Fa-f])*";
            let authority = format!("({userinfo}@)?{host}(:\\d*)?");
            let relative_part =
                format!("(//{authority}{path_abempty}|{path_absolute}|{path_noscheme})?");
            format!("^{relative_part}{query}{fragment}$")
        }
        "uri-template" => {
            let literals =
                "([\\x21\\x23-\\x24\\x26\\x28-\\x3B\\x3D\\x3F-\\x5B\\x5D\\x5F\\x61-\\x7A\\x7E]|%[0-9A-Fa-f][0-9A-Fa-f])";
            let op = "[+#\\./;\\?&=,!@|]";
            let varchar = "(\\w|%[0-9A-Fa-f][0-9A-Fa-f])";
            let varname = format!("{varchar}(\\.?{varchar})*");
            let varspec = format!("{varname}(:[1-9]\\d?\\d?\\d?|\\*)?");
            let variable_list = format!("{varspec}(,{varspec})*");
            let expression = format!("\\{{({op})?{variable_list}\\}}");
            format!("^({literals}|{expression})*$")
        }
        "json-pointer" => {
            "^(/([\\x00-\\x2E]|[\\x30-\\x7D]|[\\x7F-\\U0010FFFF]|~[01])*)*$".to_owned()
        }
        "relative-json-pointer" => {
            "^(0|[1-9][0-9]*)(#|(/([\\x00-\\x2E]|[\\x30-\\x7D]|[\\x7F-\\U0010FFFF]|~[01])*)*)$"
                .to_owned()
        }
        _ => return None,
    };
    Some(pat)
}

/// Parses a JSON literal serialized from a schema (`const`/`enum` values).
pub fn parse_literal(json_value: &str) -> Value {
    serde_json::from_str(json_value).expect("schema literals are serialized by serde_json")
}

/// The escaped content of `text` as a JSON string literal, without the surrounding quotes.
pub fn json_string_content(text: &str) -> String {
    let encoded = serialize_json(&Value::String(text.to_owned()));
    encoded[1..encoded.len() - 1].to_owned()
}

/// Single-character ranges for each byte of `characters`.
pub fn ascii_class(characters: &[u8]) -> Vec<CharacterClassElement> {
    characters.iter().map(|&byte| CharacterClassElement::new(i32::from(byte), i32::from(byte))).collect()
}

/// Whether the automaton of `regex` (excluding the JSON-string forbidden characters with
/// `json_string`) accepts any string.
fn regex_language_is_nonempty(
    regex: &str,
    json_string: bool,
) -> bool {
    GrammarFsmBuilder::regex(regex.as_bytes(), json_string)
        .is_ok_and(|fsm| fsm.reachable_states().into_iter().any(|state| fsm.is_end_state(state)))
}
