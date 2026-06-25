//! Converts a parsed [`SchemaSpec`] tree into a JSON-shaped EBNF grammar — a port of
//! `JSONSchemaConverter` and `JSONSchemaToEBNF` in `cpp/json_schema_converter.cc`.

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
};

use serde_json::Value;

use super::{
    ebnf_script_creator::EbnfScriptCreator,
    indent_manager::IndentManager,
    range_regex::{generate_float_range_regex, generate_range_regex},
    regex_converter::regex_to_ebnf,
    schema_error::SchemaError,
    schema_parser::SchemaParser,
    schema_spec::{
        AllOfSpec, AnyOfSpec, ArraySpec, ConstSpec, EnumSpec, IntegerSpec,
        NumberSpec, ObjectSpec, Property, RefSpec, SchemaSpec, SchemaSpecPtr,
        SchemaSpecVariant, StringSpec, TypeArraySpec,
    },
    xml_tool_calling_converter::{XmlJsonFormat, xml_wrapper},
};
use crate::grammar::Grammar;

impl Grammar {
    /// Builds a grammar from a JSON Schema string (the C++ `Grammar::FromJSONSchema`).
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
        let ebnf = json_schema_to_ebnf(
            schema,
            any_whitespace,
            indent,
            separators,
            strict_mode,
            max_whitespace_cnt,
        )?;
        Ok(Grammar::from_ebnf(&ebnf, "root")
            .expect("json schema converter produced valid EBNF"))
    }
}

const BASIC_ANY: &str = "basic_any";
const BASIC_INTEGER: &str = "basic_integer";
const BASIC_NUMBER: &str = "basic_number";
const BASIC_STRING: &str = "basic_string";
const BASIC_BOOLEAN: &str = "basic_boolean";
const BASIC_NULL: &str = "basic_null";
const BASIC_ARRAY: &str = "basic_array";
const BASIC_OBJECT: &str = "basic_object";
const BASIC_ESCAPE: &str = "basic_escape";
const BASIC_STRING_SUB: &str = "basic_string_sub";

const XML_STRING: &str = "xml_string";
const XML_ANY: &str = "xml_any";
const XML_OBJECT: &str = "xml_object";
const XML_VARIABLE_NAME: &str = "xml_variable_name";

/// Converts a JSON Schema string to an EBNF grammar.
///
/// `any_whitespace` allows flexible whitespace; `indent`/`separators` control formatting;
/// `strict_mode` forbids unspecified additional items/properties; `max_whitespace_cnt`
/// bounds whitespace runs.
///
/// # Errors
/// Returns a [`SchemaError`] if the schema is invalid or unsatisfiable.
pub fn json_schema_to_ebnf(
    schema: &str,
    any_whitespace: bool,
    indent: Option<i32>,
    separators: Option<(&str, &str)>,
    strict_mode: bool,
    max_whitespace_cnt: Option<i32>,
) -> Result<String, SchemaError> {
    let root: Value = serde_json::from_str(schema).map_err(|e| {
        SchemaError::invalid(format!("Failed to parse JSON: {e}"))
    })?;
    let mut parser = SchemaParser::new(root.clone(), strict_mode);
    let spec = parser.parse(&root, None)?;
    let mut converter = JsonSchemaConverter::new(
        indent,
        separators,
        any_whitespace,
        max_whitespace_cnt,
        parser,
    );
    Ok(converter.convert(&spec))
}

/// Converts a JSON Schema to EBNF in an XML tool-calling wire format.
///
/// # Errors
/// Returns [`SchemaError`] if the schema is invalid or unsatisfiable.
pub fn json_schema_to_ebnf_xml(
    schema: &str,
    format: XmlJsonFormat,
) -> Result<String, SchemaError> {
    let root: Value = serde_json::from_str(schema).map_err(|e| {
        SchemaError::invalid(format!("Failed to parse JSON: {e}"))
    })?;
    let mut parser = SchemaParser::new(root.clone(), true);
    let spec = parser.parse(&root, None)?;
    let mut converter = JsonSchemaConverter::new_xml(format, parser);
    Ok(converter.convert(&spec))
}

struct JsonSchemaConverter {
    indent_manager: IndentManager,
    any_whitespace: bool,
    max_whitespace_cnt: Option<i32>,
    colon_pattern: String,
    ebnf: EbnfScriptCreator,
    parser: SchemaParser,
    rule_cache: HashMap<String, String>,
    uri_to_rule_name: HashMap<String, String>,
    xml_format: Option<XmlJsonFormat>,
    nested_object_level: u8,
    inner_rule_cache: HashMap<String, String>,
}

impl JsonSchemaConverter {
    fn new(
        indent: Option<i32>,
        separators: Option<(&str, &str)>,
        any_whitespace: bool,
        max_whitespace_cnt: Option<i32>,
        parser: SchemaParser,
    ) -> Self {
        let item_sep = match separators {
            Some((s, _)) => s.to_owned(),
            None if any_whitespace || indent.is_some() => ",".to_owned(),
            None => ", ".to_owned(),
        };
        let colon_sep = match separators {
            Some((_, c)) => c.to_owned(),
            None if any_whitespace => ":".to_owned(),
            None => ": ".to_owned(),
        };
        let colon_pattern = if any_whitespace {
            let ws = whitespace_pattern(max_whitespace_cnt);
            format!("{ws} \"{colon_sep}\" {ws}")
        } else {
            format!("\"{colon_sep}\"")
        };
        Self {
            indent_manager: IndentManager::new(
                indent,
                &item_sep,
                any_whitespace,
                max_whitespace_cnt,
            ),
            any_whitespace,
            max_whitespace_cnt,
            colon_pattern,
            ebnf: EbnfScriptCreator::new(),
            parser,
            rule_cache: HashMap::new(),
            uri_to_rule_name: HashMap::new(),
            xml_format: None,
            nested_object_level: 0,
            inner_rule_cache: HashMap::new(),
        }
    }

    fn new_xml(
        format: XmlJsonFormat,
        parser: SchemaParser,
    ) -> Self {
        let mut converter = Self::new(None, None, true, None, parser);
        converter.xml_format = Some(format);
        converter
    }

    fn is_xml_outer(&self) -> bool {
        self.xml_format.is_some() && self.nested_object_level <= 1
    }

    fn xml_wrapper(&self) -> super::xml_tool_calling_converter::XmlWrapper {
        xml_wrapper(self.xml_format.expect("xml format set"))
    }

    fn convert(
        &mut self,
        spec: &SchemaSpecPtr,
    ) -> String {
        self.nested_object_level = 0;
        self.add_basic_rules();

        let root_rule_name = self.ebnf.allocate_rule_name("root");
        self.uri_to_rule_name.insert("#".to_owned(), root_rule_name.clone());

        if let Some(cached) = self.get_cache(&spec.cache_key) {
            self.ebnf.add_rule_with_allocated_name(&root_rule_name, &cached);
        } else {
            if !spec.cache_key.is_empty() {
                self.add_cache(&spec.cache_key, &root_rule_name);
            }
            let body = self.generate_from_spec(spec, &root_rule_name);
            self.ebnf.add_rule_with_allocated_name(&root_rule_name, &body);
        }
        self.ebnf.script()
    }

    fn add_basic_rules(&mut self) {
        if self.xml_format.is_some() && self.nested_object_level == 0 {
            self.nested_object_level = 2;
            self.add_standard_basic_rules();
            self.nested_object_level = 1;
            self.add_xml_outer_rules();
            self.nested_object_level = 0;
            return;
        }
        self.add_standard_basic_rules();
    }

    fn add_xml_outer_rules(&mut self) {
        let wrapper = self.xml_wrapper();
        self.ebnf.add_rule(
            XML_STRING,
            &format!(
                "TagDispatch(loop_after_dispatch=false,excludes=(\"{}\"))",
                wrapper.param_suffix
            ),
        );
        self.add_cache("{\"type\":\"string\"}", XML_STRING);

        let any_spec = SchemaSpec::make(SchemaSpecVariant::Any, "{}");
        let any_body = self.generate_any();
        self.ebnf.add_rule(XML_ANY, &any_body);
        self.add_cache("{}", XML_ANY);

        let obj_spec = ObjectSpec {
            allow_additional_properties: true,
            additional_properties_schema: Some(any_spec),
            ..ObjectSpec::default()
        };
        self.nested_object_level = 0;
        self.nested_object_level += 1;
        let obj_body = self.generate_object(
            &obj_spec,
            XML_OBJECT,
            self.nested_object_level > 1,
        );
        self.nested_object_level -= 1;
        self.ebnf.add_rule(XML_OBJECT, &obj_body);
        self.add_cache("{\"type\":\"object\"}", XML_OBJECT);

        self.ebnf.add_rule(XML_VARIABLE_NAME, "[a-zA-Z_][a-zA-Z0-9_]*");
    }

    fn add_standard_basic_rules(&mut self) {
        self.add_helper_rules();

        let saved = self.indent_manager.clone();
        self.indent_manager = if self.any_whitespace {
            IndentManager::new(None, ",", true, None)
        } else {
            IndentManager::new(None, ", ", false, None)
        };

        let any_spec = SchemaSpec::make(SchemaSpecVariant::Any, "{}");
        let any_body = self.generate_any();
        self.ebnf.add_rule(BASIC_ANY, &any_body);
        self.add_cache("{}", BASIC_ANY);

        let int_body = self.generate_integer(&IntegerSpec::default());
        self.ebnf.add_rule(BASIC_INTEGER, &int_body);
        self.add_cache("{\"type\":\"integer\"}", BASIC_INTEGER);

        let num_body = self.generate_number(&NumberSpec::default());
        self.ebnf.add_rule(BASIC_NUMBER, &num_body);
        self.add_cache("{\"type\":\"number\"}", BASIC_NUMBER);

        self.ebnf.add_rule(BASIC_STRING, &format!("[\"] {BASIC_STRING_SUB}"));
        self.add_cache("{\"type\":\"string\"}", BASIC_STRING);

        let bool_body = Self::generate_boolean();
        self.ebnf.add_rule(BASIC_BOOLEAN, &bool_body);
        self.add_cache("{\"type\":\"boolean\"}", BASIC_BOOLEAN);

        let null_body = Self::generate_null();
        self.ebnf.add_rule(BASIC_NULL, &null_body);
        self.add_cache("{\"type\":\"null\"}", BASIC_NULL);

        let array_spec = ArraySpec {
            allow_additional_items: true,
            additional_items: Some(any_spec.clone()),
            ..ArraySpec::default()
        };
        let array_body = self.generate_array(&array_spec, BASIC_ARRAY);
        self.ebnf.add_rule(BASIC_ARRAY, &array_body);
        self.add_cache("{\"type\":\"array\"}", BASIC_ARRAY);

        let obj_spec = ObjectSpec {
            allow_additional_properties: true,
            additional_properties_schema: Some(any_spec),
            ..ObjectSpec::default()
        };
        let obj_body = self.generate_object(&obj_spec, BASIC_OBJECT, true);
        self.ebnf.add_rule(BASIC_OBJECT, &obj_body);
        self.add_cache("{\"type\":\"object\"}", BASIC_OBJECT);

        self.indent_manager = saved;
    }

    fn add_helper_rules(&mut self) {
        self.ebnf.add_rule(
            BASIC_ESCAPE,
            "[\"\\\\/bfnrt] | \"u\" [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9] [A-Fa-f0-9]",
        );
        let ws = self.get_whitespace_pattern();
        self.ebnf.add_rule(
            BASIC_STRING_SUB,
            &format!(
                "(\"\\\"\" | [^\\0-\\x1f\\\"\\\\\\r\\n] {BASIC_STRING_SUB} | \"\\\\\" \
                 {BASIC_ESCAPE} {BASIC_STRING_SUB}) (= {ws} [,}}\\]:])"
            ),
        );
    }

    fn get_whitespace_pattern(&self) -> String {
        whitespace_pattern(self.max_whitespace_cnt)
    }

    fn next_separator(
        &mut self,
        is_end: bool,
    ) -> String {
        if self.is_xml_outer() {
            return self.get_whitespace_pattern();
        }
        self.indent_manager.next_separator(is_end)
    }

    fn add_cache(
        &mut self,
        key: &str,
        value: &str,
    ) {
        if key.is_empty() {
            return;
        }
        if self.xml_format.is_some() && self.nested_object_level > 1 {
            self.inner_rule_cache.insert(key.to_owned(), value.to_owned());
        } else {
            self.rule_cache.insert(key.to_owned(), value.to_owned());
        }
    }

    fn get_cache(
        &self,
        key: &str,
    ) -> Option<String> {
        if key.is_empty() {
            return None;
        }
        if self.xml_format.is_some() && self.nested_object_level > 1 {
            self.inner_rule_cache.get(key).cloned()
        } else {
            self.rule_cache.get(key).cloned()
        }
    }

    fn create_rule(
        &mut self,
        spec: &SchemaSpecPtr,
        rule_name_hint: &str,
    ) -> String {
        if let Some(cached) = self.get_cache(&spec.cache_key) {
            return cached;
        }
        let rule_name = self.ebnf.allocate_rule_name(rule_name_hint);
        let body = self.generate_from_spec(spec, &rule_name);
        self.ebnf.add_rule_with_allocated_name(&rule_name, &body);
        rule_name
    }

    fn generate_from_spec(
        &mut self,
        spec: &SchemaSpecPtr,
        rule_name_hint: &str,
    ) -> String {
        match &spec.spec {
            SchemaSpecVariant::Integer(s) => self.generate_integer(s),
            SchemaSpecVariant::Number(s) => self.generate_number(s),
            SchemaSpecVariant::String(s) => self.generate_string(s),
            SchemaSpecVariant::Boolean => Self::generate_boolean(),
            SchemaSpecVariant::Null => Self::generate_null(),
            SchemaSpecVariant::Array(s) => {
                if self.xml_format.is_some() {
                    self.nested_object_level += 1;
                    let result = self.generate_array(s, rule_name_hint);
                    self.nested_object_level -= 1;
                    result
                } else {
                    self.generate_array(s, rule_name_hint)
                }
            },
            SchemaSpecVariant::Object(s) => {
                if self.xml_format.is_some() {
                    self.nested_object_level += 1;
                    let need_braces = self.nested_object_level > 1;
                    let result =
                        self.generate_object(s, rule_name_hint, need_braces);
                    self.nested_object_level -= 1;
                    result
                } else {
                    self.generate_object(s, rule_name_hint, true)
                }
            },
            SchemaSpecVariant::Any => self.generate_any(),
            SchemaSpecVariant::Const(s) => self.generate_const(s),
            SchemaSpecVariant::Enum(s) => self.generate_enum(s),
            SchemaSpecVariant::Ref(s) => self.generate_ref(s),
            SchemaSpecVariant::AnyOf(s) => {
                self.generate_any_of(s, rule_name_hint)
            },
            SchemaSpecVariant::AllOf(s) => {
                self.generate_all_of(s, rule_name_hint)
            },
            SchemaSpecVariant::TypeArray(s) => {
                self.generate_type_array(s, rule_name_hint)
            },
        }
    }

    fn generate_integer(
        &self,
        spec: &IntegerSpec,
    ) -> String {
        let mut start = spec.minimum;
        if let Some(e) = spec.exclusive_minimum {
            start = Some(e + 1);
        }
        let mut end = spec.maximum;
        if let Some(e) = spec.exclusive_maximum {
            end = Some(e - 1);
        }
        if start.is_some() || end.is_some() {
            let regex = generate_range_regex(start, end);
            return regex_to_ebnf(&regex, false).expect("range regex is valid");
        }
        "(\"0\" | \"-\"? [1-9] [0-9]*)".to_owned()
    }

    fn generate_number(
        &self,
        spec: &NumberSpec,
    ) -> String {
        let mut start = spec.minimum;
        if let Some(e) = spec.exclusive_minimum {
            start = Some(e);
        }
        let mut end = spec.maximum;
        if let Some(e) = spec.exclusive_maximum {
            end = Some(e);
        }
        if start.is_some() || end.is_some() {
            let regex = generate_float_range_regex(start, end);
            return regex_to_ebnf(&regex, false)
                .expect("float range regex is valid");
        }
        "\"-\"? (\"0\" | [1-9] [0-9]*) (\".\" [0-9]+)? ([eE] [+-]? [0-9]+)?"
            .to_owned()
    }

    fn generate_string(
        &self,
        spec: &StringSpec,
    ) -> String {
        if self.is_xml_outer() {
            if spec.pattern.is_none()
                && spec.format.is_none()
                && spec.min_length == 0
                && spec.max_length == -1
            {
                return XML_STRING.to_owned();
            }
            if let Some(format) = &spec.format {
                if let Some(regex) = json_format_to_regex_pattern(format) {
                    let converted = regex_to_ebnf(&regex, false)
                        .expect("format regex is valid");
                    return converted;
                }
            }
            if let Some(pattern) = &spec.pattern {
                let converted = regex_to_ebnf(pattern, false)
                    .expect("pattern regex is valid");
                return converted;
            }
            if spec.min_length != 0 || spec.max_length != -1 {
                let repetition = if spec.max_length == -1 {
                    format!("{{{},}}", spec.min_length)
                } else {
                    format!("{{{},{}}}", spec.min_length, spec.max_length)
                };
                return format!("[^]{repetition}");
            }
        }
        if let Some(format) = &spec.format {
            if let Some(regex) = json_format_to_regex_pattern(format) {
                let converted = regex_to_ebnf(&regex, false)
                    .expect("format regex is valid");
                return format!("\"\\\"\" {converted} \"\\\"\"");
            }
        }
        if let Some(pattern) = &spec.pattern {
            let converted =
                regex_to_ebnf(pattern, false).expect("pattern regex is valid");
            return format!("\"\\\"\" {converted} \"\\\"\"");
        }
        if spec.min_length != 0 || spec.max_length != -1 {
            let repetition = if spec.max_length == -1 {
                format!("{{{},}}", spec.min_length)
            } else {
                format!("{{{},{}}}", spec.min_length, spec.max_length)
            };
            return format!("\"\\\"\" [^\"\\\\\\r\\n]{repetition} \"\\\"\"");
        }
        format!("[\"] {BASIC_STRING_SUB}")
    }

    fn generate_boolean() -> String {
        "\"true\" | \"false\"".to_owned()
    }

    fn generate_null() -> String {
        "\"null\"".to_owned()
    }

    fn generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> String {
        self.indent_manager.start_indent();
        let start_sep = self.indent_manager.start_separator();
        let mid_sep = self.indent_manager.middle_separator();
        let end_sep = self.indent_manager.end_separator();
        let empty_sep = self.indent_manager.empty_separator();

        let mut item_rule_names = Vec::new();
        for (i, item) in spec.prefix_items.iter().enumerate() {
            item_rule_names
                .push(self.create_rule(item, &format!("{rule_name}_item_{i}")));
        }

        let mut additional_rule_name = String::new();
        if spec.allow_additional_items {
            if let Some(additional) = &spec.additional_items {
                additional_rule_name = self.create_rule(
                    additional,
                    &format!("{rule_name}_additional"),
                );
            }
        }

        self.indent_manager.end_indent();

        let left = EbnfScriptCreator::str_literal("[");
        let right = EbnfScriptCreator::str_literal("]");

        if spec.prefix_items.is_empty() {
            let empty_part = EbnfScriptCreator::concat(&[
                left.clone(),
                empty_sep,
                right.clone(),
            ]);
            if !spec.allow_additional_items
                || (spec.min_items == 0 && spec.max_items == 0)
            {
                empty_part
            } else if spec.min_items == 0 {
                let repeat = EbnfScriptCreator::repeat(
                    &EbnfScriptCreator::concat(&[
                        mid_sep,
                        additional_rule_name.clone(),
                    ]),
                    0,
                    if spec.max_items == -1 {
                        -1
                    } else {
                        (spec.max_items - 1) as i32
                    },
                );
                let non_empty = EbnfScriptCreator::concat(&[
                    left,
                    start_sep,
                    additional_rule_name,
                    repeat,
                    end_sep,
                    right,
                ]);
                EbnfScriptCreator::or(&[non_empty, empty_part])
            } else {
                let repeat = EbnfScriptCreator::repeat(
                    &EbnfScriptCreator::concat(&[
                        mid_sep,
                        additional_rule_name.clone(),
                    ]),
                    (spec.min_items - 1) as i32,
                    if spec.max_items == -1 {
                        -1
                    } else {
                        (spec.max_items - 1) as i32
                    },
                );
                EbnfScriptCreator::concat(&[
                    left,
                    start_sep,
                    additional_rule_name,
                    repeat,
                    end_sep,
                    right,
                ])
            }
        } else {
            let mut prefix_part = Vec::new();
            for (i, name) in item_rule_names.iter().enumerate() {
                if i > 0 {
                    prefix_part.push(mid_sep.clone());
                }
                prefix_part.push(name.clone());
            }
            let prefix_part_str = EbnfScriptCreator::concat(&prefix_part);
            if !spec.allow_additional_items {
                EbnfScriptCreator::concat(&[
                    left,
                    start_sep,
                    prefix_part_str,
                    end_sep,
                    right,
                ])
            } else {
                let min_items =
                    (spec.min_items - item_rule_names.len() as i64).max(0);
                let repeat = EbnfScriptCreator::repeat(
                    &EbnfScriptCreator::concat(&[
                        mid_sep,
                        additional_rule_name,
                    ]),
                    min_items as i32,
                    if spec.max_items == -1 {
                        -1
                    } else {
                        (spec.max_items - item_rule_names.len() as i64) as i32
                    },
                );
                EbnfScriptCreator::concat(&[
                    left,
                    start_sep,
                    prefix_part_str,
                    repeat,
                    end_sep,
                    right,
                ])
            }
        }
    }

    fn outer_key_pattern(&self) -> String {
        if self.is_xml_outer() {
            XML_VARIABLE_NAME.to_owned()
        } else {
            BASIC_STRING.to_owned()
        }
    }

    fn format_property_key(
        &self,
        key: &str,
    ) -> String {
        if self.is_xml_outer() {
            let w = self.xml_wrapper();
            return format!("\"{}{}{}\"", w.key_prefix, key, w.key_suffix);
        }
        format!(
            "\"{}\"",
            json_str_to_printable_str(
                &Value::String(key.to_owned()).to_string()
            )
        )
    }

    fn format_property(
        &self,
        key: &str,
        value_rule: &str,
    ) -> String {
        if self.is_xml_outer() {
            let w = self.xml_wrapper();
            let ws = self.get_whitespace_pattern();
            if w.value_prefix.is_empty() {
                return format!(
                    "\"{}{}{}\" {ws} {value_rule} {ws} \"{}\"",
                    w.key_prefix, key, w.key_suffix, w.param_suffix
                );
            }
            return format!(
                "\"{}{}{}\" {ws} \"{}\" {ws} {value_rule} {ws} \"{}\"",
                w.key_prefix, key, w.key_suffix, w.value_prefix, w.param_suffix
            );
        }
        format!(
            "{} {} {}",
            self.format_property_key(key),
            self.colon_pattern,
            value_rule
        )
    }

    fn format_other_property(
        &self,
        key_pattern: &str,
        value_rule: &str,
    ) -> String {
        if self.is_xml_outer() {
            let w = self.xml_wrapper();
            let ws = self.get_whitespace_pattern();
            if w.value_prefix.is_empty() {
                return format!(
                    "\"{}\" {key_pattern} \"{}\" {ws} {value_rule} {ws} \"{}\"",
                    w.key_prefix, w.key_suffix, w.param_suffix
                );
            }
            return format!(
                "\"{}\" {key_pattern} \"{}\" {ws} \"{}\" {ws} {value_rule} {ws} \"{}\"",
                w.key_prefix, w.key_suffix, w.value_prefix, w.param_suffix
            );
        }
        format!("{} {} {}", key_pattern, self.colon_pattern, value_rule)
    }

    fn get_property_with_number_constraints(
        pattern: &str,
        min_properties: i32,
        max_properties: i32,
        already_repeated_times: i32,
    ) -> String {
        if max_properties != -1 && max_properties == already_repeated_times {
            return "\"\"".to_owned();
        }
        let lower = (min_properties - already_repeated_times).max(0);
        let upper = if max_properties == -1 {
            -1
        } else {
            (max_properties - already_repeated_times).max(-1)
        };
        if lower == 0 && upper == -1 {
            format!("({pattern})*")
        } else if lower == 0 && upper == 1 {
            format!("({pattern})?")
        } else if lower == 1 && upper == 1 {
            pattern.to_owned()
        } else {
            format!(
                "({pattern}){{{lower},{}}} ",
                if upper == -1 {
                    String::new()
                } else {
                    upper.to_string()
                }
            )
        }
    }

    fn get_key_pattern_excluding(
        &mut self,
        properties: &[Property],
        rule_name: &str,
    ) -> String {
        if self.is_xml_outer() {
            return XML_VARIABLE_NAME.to_owned();
        }
        if properties.is_empty() {
            return BASIC_STRING.to_owned();
        }
        let mut root = TrieNode::default();
        for prop in properties {
            let mut cur = &mut root;
            for c in prop.name.bytes() {
                cur = cur.children.entry(c).or_default();
            }
            cur.is_terminal = true;
        }
        let inner = build_trie_body(&root);
        let ws = self.get_whitespace_pattern();
        let body = format!("[\"] ({inner}) (= {ws} [,}}\\]:])");
        self.ebnf.add_rule(&format!("{rule_name}_addl_key"), &body)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_partial_rule_for_properties(
        &mut self,
        properties: &[Property],
        required: &std::collections::HashSet<String>,
        additional: Option<&SchemaSpecPtr>,
        rule_name: &str,
        additional_suffix: &str,
        min_properties: i32,
        max_properties: i32,
        additional_prop_pattern_override: &str,
    ) -> String {
        if max_properties == 0 {
            return String::new();
        }

        let first_sep = self.next_separator(false);
        let mid_sep = self.next_separator(false);
        let last_sep = self.next_separator(true);

        let n = properties.len();
        let mut prop_patterns = Vec::with_capacity(n);
        for (idx, prop) in properties.iter().enumerate() {
            let value_rule = self
                .create_rule(&prop.schema, &format!("{rule_name}_prop_{idx}"));
            prop_patterns.push(self.format_property(&prop.name, &value_rule));
        }

        let mut res = String::new();

        if min_properties == 0 && max_properties == -1 {
            let mut rule_names = vec![String::new(); n];
            let mut is_required = vec![false; n];
            let allow_additional = additional.is_some();

            let mut additional_prop_pattern = String::new();
            if allow_additional {
                if !additional_prop_pattern_override.is_empty() {
                    additional_prop_pattern =
                        additional_prop_pattern_override.to_owned();
                } else {
                    let add_value_rule = self.create_rule(
                        additional.unwrap(),
                        &format!("{rule_name}_{additional_suffix}"),
                    );
                    let key = BASIC_STRING.to_owned();
                    additional_prop_pattern =
                        self.format_other_property(&key, &add_value_rule);
                }
                let last_rule_body =
                    format!("({mid_sep} {additional_prop_pattern})*");
                let last_rule_name = self.ebnf.add_rule(
                    &format!("{rule_name}_part_{}", n - 1),
                    &last_rule_body,
                );
                rule_names[n - 1] = last_rule_name;
            } else {
                rule_names[n - 1] = "\"\"".to_owned();
            }

            for i in (0..n.saturating_sub(1)).rev() {
                let prop_pattern = &prop_patterns[i + 1];
                let last_rule_name = &rule_names[i + 1];
                let mut cur_rule_body =
                    format!("{mid_sep} {prop_pattern} {last_rule_name}");
                if !required.contains(&properties[i + 1].name) {
                    cur_rule_body =
                        format!("{last_rule_name} | {cur_rule_body}");
                } else {
                    is_required[i + 1] = true;
                }
                let cur_rule_name = self
                    .ebnf
                    .add_rule(&format!("{rule_name}_part_{i}"), &cur_rule_body);
                rule_names[i] = cur_rule_name;
            }
            if required.contains(&properties[0].name) {
                is_required[0] = true;
            }

            for i in 0..n {
                if i != 0 {
                    res.push_str(" | ");
                }
                let _ = write!(res, "({} {})", prop_patterns[i], rule_names[i]);
                if is_required[i] {
                    break;
                }
            }

            if allow_additional && required.is_empty() {
                let _ = write!(
                    res,
                    " | {additional_prop_pattern} {}",
                    rule_names[n - 1]
                );
            }

            res = format!("{first_sep} ({res}) {last_sep}");
        } else if max_properties == -1 {
            res = self.partial_properties_min_only(
                properties,
                required,
                additional,
                rule_name,
                additional_suffix,
                min_properties,
                additional_prop_pattern_override,
                &prop_patterns,
                &mid_sep,
                &first_sep,
                &last_sep,
            );
        } else {
            res = self.partial_properties_min_max(
                properties,
                required,
                additional,
                rule_name,
                additional_suffix,
                min_properties,
                max_properties,
                additional_prop_pattern_override,
                &prop_patterns,
                &mid_sep,
                &first_sep,
                &last_sep,
            );
        }

        res
    }

    #[allow(clippy::too_many_arguments)]
    fn partial_properties_min_only(
        &mut self,
        properties: &[Property],
        required: &std::collections::HashSet<String>,
        additional: Option<&SchemaSpecPtr>,
        rule_name: &str,
        additional_suffix: &str,
        min_properties: i32,
        additional_prop_pattern_override: &str,
        prop_patterns: &[String],
        mid_sep: &str,
        first_sep: &str,
        last_sep: &str,
    ) -> String {
        let n = properties.len() as i32;
        let mut rule_names: Vec<Vec<String>> = vec![Vec::new(); n as usize];
        let mut key_matched_min = vec![0i32; n as usize];
        let mut is_required = vec![false; n as usize];
        let allow_additional = additional.is_some();

        let mut additional_prop_pattern = String::new();
        if allow_additional {
            if !additional_prop_pattern_override.is_empty() {
                additional_prop_pattern =
                    additional_prop_pattern_override.to_owned();
            } else {
                let add_value_rule = self.create_rule(
                    additional.unwrap(),
                    &format!("{rule_name}_{additional_suffix}"),
                );
                let key = BASIC_STRING.to_owned();
                additional_prop_pattern =
                    self.format_other_property(&key, &add_value_rule);
            }
        }

        let mut get_first_required = required.contains(&properties[0].name);
        key_matched_min[0] = 1;
        for i in 1..n as usize {
            if required.contains(&properties[i].name) {
                is_required[i] = true;
                key_matched_min[i] = key_matched_min[i - 1] + 1;
            } else {
                key_matched_min[i] = key_matched_min[i - 1];
            }
            if !get_first_required {
                key_matched_min[i] = 1;
            }
            if is_required[i] {
                get_first_required = true;
            }
        }
        if required.contains(&properties[0].name) {
            is_required[0] = true;
        }
        let last = (n - 1) as usize;
        if allow_additional {
            key_matched_min[last] = key_matched_min[last].max(1);
        } else {
            key_matched_min[last] = key_matched_min[last].max(min_properties);
        }
        for i in (0..(n - 1) as usize).rev() {
            key_matched_min[i] =
                key_matched_min[i].max(key_matched_min[i + 1] - 1);
        }

        if allow_additional {
            let mut matched = key_matched_min[last];
            while matched <= n {
                let body = Self::get_property_with_number_constraints(
                    &format!("{mid_sep} {additional_prop_pattern}"),
                    min_properties,
                    -1,
                    matched,
                );
                let name = self.ebnf.add_rule(
                    &format!("{rule_name}_part_{}_{}", n - 1, matched),
                    &body,
                );
                rule_names[last].push(name);
                matched += 1;
            }
        } else {
            let mut matched = key_matched_min[last];
            while matched <= n {
                rule_names[last].push("\"\"".to_owned());
                matched += 1;
            }
        }

        for i in (0..(n - 1) as usize).rev() {
            let prop_pattern = &prop_patterns[i + 1];
            let mut matched = key_matched_min[i];
            while matched <= (i + 1) as i32 {
                let body = if is_required[i + 1]
                    || matched == key_matched_min[i + 1] - 1
                {
                    format!(
                        "{mid_sep} {prop_pattern} {}",
                        rule_names[i + 1]
                            [(matched + 1 - key_matched_min[i + 1]) as usize]
                    )
                } else {
                    format!(
                        "{} | {mid_sep} {prop_pattern} {}",
                        rule_names[i + 1]
                            [(matched - key_matched_min[i + 1]) as usize],
                        rule_names[i + 1]
                            [(matched - key_matched_min[i + 1] + 1) as usize]
                    )
                };
                let name = self.ebnf.add_rule(
                    &format!("{rule_name}_part_{i}_{matched}"),
                    &body,
                );
                rule_names[i].push(name);
                matched += 1;
            }
        }

        let mut res = String::new();
        let mut is_first = true;
        for i in 0..n as usize {
            if key_matched_min[i] > 1 {
                break;
            }
            if !is_first {
                res.push_str(" | ");
            } else {
                is_first = false;
            }
            let _ = write!(
                res,
                "({} {})",
                prop_patterns[i],
                rule_names[i][(1 - key_matched_min[i]) as usize]
            );
            if is_required[i] {
                break;
            }
        }

        if allow_additional && required.is_empty() {
            if !is_first {
                res.push_str(" | ");
            }
            let _ = write!(
                res,
                "({additional_prop_pattern} {})",
                Self::get_property_with_number_constraints(
                    &format!("{mid_sep} {additional_prop_pattern}"),
                    min_properties,
                    -1,
                    1,
                )
            );
        }

        format!("{first_sep} ({res}) {last_sep}")
    }

    #[allow(clippy::too_many_arguments)]
    fn partial_properties_min_max(
        &mut self,
        properties: &[Property],
        required: &std::collections::HashSet<String>,
        additional: Option<&SchemaSpecPtr>,
        rule_name: &str,
        additional_suffix: &str,
        min_properties: i32,
        max_properties: i32,
        additional_prop_pattern_override: &str,
        prop_patterns: &[String],
        mid_sep: &str,
        first_sep: &str,
        last_sep: &str,
    ) -> String {
        let n = properties.len() as i32;
        let mut rule_names: Vec<Vec<String>> = vec![Vec::new(); n as usize];
        let mut key_matched_min = vec![0i32; n as usize];
        let mut key_matched_max = vec![n; n as usize];
        let mut is_required = vec![false; n as usize];
        let allow_additional = additional.is_some();

        let mut additional_prop_pattern = String::new();
        if allow_additional {
            if !additional_prop_pattern_override.is_empty() {
                additional_prop_pattern =
                    additional_prop_pattern_override.to_owned();
            } else {
                let add_value_rule = self.create_rule(
                    additional.unwrap(),
                    &format!("{rule_name}_{additional_suffix}"),
                );
                let key = BASIC_STRING.to_owned();
                additional_prop_pattern =
                    self.format_other_property(&key, &add_value_rule);
            }
        }

        let mut get_first_required = required.contains(&properties[0].name);
        key_matched_min[0] = 1;
        key_matched_max[0] = 1;
        for i in 1..n as usize {
            if required.contains(&properties[i].name) {
                is_required[i] = true;
                key_matched_min[i] = key_matched_min[i - 1] + 1;
            } else {
                key_matched_min[i] = key_matched_min[i - 1];
            }
            if !get_first_required {
                key_matched_min[i] = 1;
            }
            key_matched_max[i] = key_matched_max[i - 1] + 1;
            if is_required[i] {
                get_first_required = true;
            }
        }
        if required.contains(&properties[0].name) {
            is_required[0] = true;
        }
        let last = (n - 1) as usize;
        if allow_additional {
            key_matched_min[last] = key_matched_min[last].max(1);
            key_matched_max[last] = key_matched_max[last].min(max_properties);
        } else {
            key_matched_min[last] = key_matched_min[last].max(min_properties);
            key_matched_max[last] = key_matched_max[last].min(max_properties);
        }
        for i in (0..(n - 1) as usize).rev() {
            key_matched_min[i] =
                key_matched_min[i].max(key_matched_min[i + 1] - 1);
            if is_required[i + 1] {
                key_matched_max[i] =
                    key_matched_max[i].min(key_matched_max[i + 1] - 1);
            } else {
                key_matched_max[i] =
                    key_matched_max[i].min(key_matched_max[i + 1]);
            }
        }

        if allow_additional {
            let mut matched = key_matched_min[last];
            while matched <= key_matched_max[last] {
                let body = Self::get_property_with_number_constraints(
                    &format!("{mid_sep} {additional_prop_pattern}"),
                    min_properties,
                    max_properties,
                    matched,
                );
                let name = self.ebnf.add_rule(
                    &format!("{rule_name}_part_{}_{}", n - 1, matched),
                    &body,
                );
                rule_names[last].push(name);
                matched += 1;
            }
        } else {
            let mut matched = key_matched_min[last];
            while matched <= key_matched_max[last] {
                rule_names[last].push("\"\"".to_owned());
                matched += 1;
            }
        }

        for i in (0..(n - 1) as usize).rev() {
            let prop_pattern = &prop_patterns[i + 1];
            let mut matched = key_matched_min[i];
            while matched <= key_matched_max[i] {
                let body = if matched == key_matched_max[i + 1] {
                    rule_names[i + 1]
                        [(matched - key_matched_min[i + 1]) as usize]
                        .clone()
                } else if is_required[i + 1]
                    || matched == key_matched_min[i + 1] - 1
                {
                    format!(
                        "{mid_sep} {prop_pattern} {}",
                        rule_names[i + 1]
                            [(matched + 1 - key_matched_min[i + 1]) as usize]
                    )
                } else {
                    format!(
                        "{} | {mid_sep} {prop_pattern} {}",
                        rule_names[i + 1]
                            [(matched - key_matched_min[i + 1]) as usize],
                        rule_names[i + 1]
                            [(matched - key_matched_min[i + 1] + 1) as usize]
                    )
                };
                let name = self.ebnf.add_rule(
                    &format!("{rule_name}_part_{i}_{matched}"),
                    &body,
                );
                rule_names[i].push(name);
                matched += 1;
            }
        }

        let mut res = String::new();
        let mut is_first = true;
        for i in 0..n as usize {
            if key_matched_max[i] < key_matched_min[i] {
                continue;
            }
            if key_matched_min[i] > 1 {
                break;
            }
            if !is_first {
                res.push_str(" | ");
            } else {
                is_first = false;
            }
            let _ = write!(
                res,
                "({} {})",
                prop_patterns[i],
                rule_names[i][(1 - key_matched_min[i]) as usize]
            );
            if is_required[i] {
                break;
            }
        }

        if allow_additional && required.is_empty() {
            if !is_first {
                res.push_str(" | ");
            }
            let _ = write!(
                res,
                "({additional_prop_pattern} {})",
                Self::get_property_with_number_constraints(
                    &format!("{mid_sep} {additional_prop_pattern}"),
                    min_properties,
                    max_properties,
                    1,
                )
            );
        }

        format!("{first_sep} ({res}) {last_sep}")
    }

    fn generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
        need_braces: bool,
    ) -> String {
        let mut result = String::new();
        if need_braces {
            result.push_str("\"{\"");
        }

        let mut could_be_empty = false;

        let mut additional_suffix = String::new();
        let mut additional_property: Option<SchemaSpecPtr> = None;
        if spec.allow_additional_properties
            && spec.additional_properties_schema.is_some()
        {
            additional_suffix = "addl".to_owned();
            additional_property = spec.additional_properties_schema.clone();
        } else if spec.allow_unevaluated_properties
            && spec.unevaluated_properties_schema.is_some()
        {
            additional_suffix = "uneval".to_owned();
            additional_property = spec.unevaluated_properties_schema.clone();
        } else if spec.allow_additional_properties
            || spec.allow_unevaluated_properties
        {
            additional_suffix = "addl".to_owned();
            additional_property =
                Some(SchemaSpec::make(SchemaSpecVariant::Any, ""));
        }

        self.indent_manager.start_indent();

        if !spec.properties.is_empty()
            && (!spec.pattern_properties.is_empty()
                || spec.property_names.is_some())
        {
            let mut effective_additional = additional_property.clone();
            let mut effective_suffix = additional_suffix.clone();
            let mut pp_override = String::new();

            if !spec.pattern_properties.is_empty() {
                let mut pp_body = String::new();
                for (i, pp) in spec.pattern_properties.iter().enumerate() {
                    let value = self.create_rule(
                        &pp.schema,
                        &format!("{rule_name}_pp_{i}"),
                    );
                    let converted = regex_to_ebnf(&pp.pattern, false)
                        .expect("pattern regex is valid");
                    let pp_single = format!(
                        "\"\\\"\"{converted}\"\\\"\" {} {value}",
                        self.colon_pattern
                    );
                    if i != 0 {
                        pp_body.push_str(" | ");
                    }
                    pp_body.push_str(&pp_single);
                }
                if let Some(add) = &effective_additional {
                    let add_value_rule = self.create_rule(
                        add,
                        &format!("{rule_name}_{effective_suffix}"),
                    );
                    let add_prop = self.format_other_property(
                        &self.outer_key_pattern(),
                        &add_value_rule,
                    );
                    let _ = write!(pp_body, " | {add_prop}");
                }
                pp_override = format!("({pp_body})");
                if effective_additional.is_none() {
                    effective_additional =
                        Some(SchemaSpec::make(SchemaSpecVariant::Any, ""));
                }
                effective_suffix = "pp".to_owned();
            } else if spec.property_names.is_some()
                && effective_additional.is_some()
            {
                let key_pattern = self.create_rule(
                    spec.property_names.as_ref().unwrap(),
                    &format!("{rule_name}_name"),
                );
                let val_rule = self.create_rule(
                    effective_additional.as_ref().unwrap(),
                    &format!("{rule_name}_{effective_suffix}"),
                );
                pp_override =
                    format!("{key_pattern} {} {val_rule}", self.colon_pattern);
                effective_suffix = "pn".to_owned();
            }

            let partial = self.get_partial_rule_for_properties(
                &spec.properties,
                &spec.required,
                effective_additional.as_ref(),
                rule_name,
                &effective_suffix,
                spec.min_properties,
                spec.max_properties,
                &pp_override,
            );
            result.push(' ');
            result.push_str(&partial);
            could_be_empty =
                spec.required.is_empty() && spec.min_properties == 0;
        } else if !spec.pattern_properties.is_empty()
            || spec.property_names.is_some()
        {
            let beg_seq = self.next_separator(false);
            if spec.max_properties != 0 {
                let mut property_rule_body = String::from("(");
                if !spec.pattern_properties.is_empty() {
                    for (i, pp) in spec.pattern_properties.iter().enumerate() {
                        let value = self.create_rule(
                            &pp.schema,
                            &format!("{rule_name}_prop_{i}"),
                        );
                        let converted = regex_to_ebnf(&pp.pattern, false)
                            .expect("pattern regex is valid");
                        let property_pattern = format!(
                            "\"\\\"\"{converted}\"\\\"\" {} {value}",
                            self.colon_pattern
                        );
                        if i != 0 {
                            property_rule_body.push_str(" | ");
                        }
                        let _ = write!(
                            property_rule_body,
                            "({beg_seq} {property_pattern})"
                        );
                    }
                    property_rule_body.push(')');
                } else {
                    let key_pattern = self.create_rule(
                        spec.property_names.as_ref().unwrap(),
                        &format!("{rule_name}_name"),
                    );
                    let _ = write!(
                        property_rule_body,
                        "{beg_seq} {key_pattern} {} {BASIC_ANY})",
                        self.colon_pattern
                    );
                }
                let prop_rule_name =
                    self.ebnf.allocate_rule_name(&format!("{rule_name}_prop"));
                self.ebnf.add_rule_with_allocated_name(
                    &prop_rule_name,
                    &property_rule_body,
                );

                let next1 = self.next_separator(false);
                let constraints = Self::get_property_with_number_constraints(
                    &format!("{next1} {prop_rule_name}"),
                    spec.min_properties,
                    spec.max_properties,
                    1,
                );
                let next_end = self.next_separator(true);
                let _ =
                    write!(result, " {prop_rule_name} {constraints}{next_end}");
                could_be_empty = spec.min_properties == 0;
            }
        } else if !spec.properties.is_empty() {
            let partial = self.get_partial_rule_for_properties(
                &spec.properties,
                &spec.required,
                additional_property.as_ref(),
                rule_name,
                &additional_suffix,
                spec.min_properties,
                spec.max_properties,
                "",
            );
            result.push(' ');
            result.push_str(&partial);
            could_be_empty =
                spec.required.is_empty() && spec.min_properties == 0;
        } else if let Some(additional) = &additional_property {
            if spec.max_properties != 0 {
                let add_value_rule = self.create_rule(
                    additional,
                    &format!("{rule_name}_{additional_suffix}"),
                );
                let other_property_pattern = self.format_other_property(
                    &self.outer_key_pattern(),
                    &add_value_rule,
                );
                let sep1 = self.next_separator(false);
                let _ = write!(result, " {sep1} {other_property_pattern} ");
                let sep2 = self.next_separator(false);
                let constraints = Self::get_property_with_number_constraints(
                    &format!("{sep2} {other_property_pattern}"),
                    spec.min_properties,
                    spec.max_properties,
                    1,
                );
                let sep_end = self.next_separator(true);
                let _ = write!(result, "{constraints} {sep_end}");
            }
            could_be_empty = spec.min_properties == 0;
        } else {
            could_be_empty = true;
        }

        self.indent_manager.end_indent();

        if need_braces {
            result.push_str(" \"}\"");
        }
        if could_be_empty {
            let ws = self.get_whitespace_pattern();
            let rest = if need_braces {
                format!(
                    "\"{{\" {}\"}}\"",
                    if self.any_whitespace {
                        format!("{ws} ")
                    } else {
                        String::new()
                    }
                )
            } else if self.any_whitespace {
                ws
            } else {
                String::new()
            };
            if result == "\"{\"  \"}\"" || result.is_empty() {
                result = rest;
            } else {
                result = format!("({result}) | {rest}");
            }
        }

        if result.is_empty() {
            return "\"\"".to_owned();
        }
        result
    }

    fn generate_any(&self) -> String {
        if self.xml_format.is_some() {
            return match self.nested_object_level {
                0 => XML_OBJECT.to_owned(),
                1 => format!("{XML_STRING} | {BASIC_ARRAY} | {BASIC_OBJECT}"),
                _ => format!(
                    "{BASIC_NUMBER} | {BASIC_STRING} | {BASIC_BOOLEAN} | {BASIC_NULL} | \
                     {BASIC_ARRAY} | {BASIC_OBJECT}"
                ),
            };
        }
        format!(
            "{BASIC_NUMBER} | {BASIC_STRING} | {BASIC_BOOLEAN} | {BASIC_NULL} | {BASIC_ARRAY} | \
             {BASIC_OBJECT}"
        )
    }

    fn generate_const(
        &self,
        spec: &ConstSpec,
    ) -> String {
        if self.is_xml_outer() {
            let val = &spec.json_value;
            if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
                return format!("\"{}\"", &val[1..val.len() - 1]);
            }
            return format!("\"{val}\"");
        }
        format!("\"{}\"", json_str_to_printable_str(&spec.json_value))
    }

    fn generate_enum(
        &self,
        spec: &EnumSpec,
    ) -> String {
        if self.is_xml_outer() {
            let mut result = String::new();
            for (i, value) in spec.json_values.iter().enumerate() {
                if i != 0 {
                    result.push_str(" | ");
                }
                if value.len() >= 2
                    && value.starts_with('"')
                    && value.ends_with('"')
                {
                    let _ =
                        write!(result, "(\"{}\")", &value[1..value.len() - 1]);
                } else {
                    let _ = write!(result, "(\"{value}\")");
                }
            }
            return result;
        }
        let mut result = String::new();
        for (i, value) in spec.json_values.iter().enumerate() {
            if i != 0 {
                result.push_str(" | ");
            }
            let _ =
                write!(result, "(\"{}\")", json_str_to_printable_str(value));
        }
        result
    }

    fn generate_ref(
        &mut self,
        spec: &RefSpec,
    ) -> String {
        if let Some(name) = self.uri_to_rule_name.get(&spec.uri) {
            return name.clone();
        }

        let mut rule_name_hint = "ref".to_owned();
        if spec.uri.starts_with("#/") {
            let mut prefix = String::new();
            for part in spec.uri[2..].split('/') {
                if !part.is_empty() {
                    if !prefix.is_empty() {
                        prefix.push('_');
                    }
                    for c in part.chars() {
                        if c.is_ascii_alphabetic()
                            || c == '_'
                            || c == '-'
                            || c == '.'
                        {
                            prefix.push(c);
                        }
                    }
                }
            }
            if !prefix.is_empty() {
                rule_name_hint = prefix;
            }
        }

        let allocated = self.ebnf.allocate_rule_name(&rule_name_hint);
        self.uri_to_rule_name.insert(spec.uri.clone(), allocated.clone());

        let resolved = self
            .parser
            .resolve_ref(&spec.uri, &allocated)
            .expect("ref resolution succeeds for a valid schema");
        let body = self.generate_from_spec(&resolved, &allocated);
        self.ebnf.add_rule_with_allocated_name(&allocated, &body);

        if !resolved.cache_key.is_empty() {
            self.add_cache(&resolved.cache_key, &allocated);
        }
        allocated
    }

    fn generate_any_of(
        &mut self,
        spec: &AnyOfSpec,
        rule_name: &str,
    ) -> String {
        let mut result = String::new();
        for (i, option) in spec.options.iter().enumerate() {
            if i != 0 {
                result.push_str(" | ");
            }
            result.push_str(
                &self.create_rule(option, &format!("{rule_name}_case_{i}")),
            );
        }
        result
    }

    fn generate_all_of(
        &mut self,
        spec: &AllOfSpec,
        rule_name: &str,
    ) -> String {
        if spec.schemas.len() == 1 {
            return self.generate_from_spec(
                &spec.schemas[0],
                &format!("{rule_name}_case_0"),
            );
        }
        let any = SchemaSpec::make(SchemaSpecVariant::Any, "");
        self.generate_from_spec(&any, rule_name)
    }

    fn generate_type_array(
        &mut self,
        spec: &TypeArraySpec,
        rule_name: &str,
    ) -> String {
        let mut result = String::new();
        for (i, ty) in spec.type_schemas.iter().enumerate() {
            if i != 0 {
                result.push_str(" | ");
            }
            result.push_str(
                &self.create_rule(ty, &format!("{rule_name}_type_{i}")),
            );
        }
        result
    }
}

fn whitespace_pattern(max_whitespace_cnt: Option<i32>) -> String {
    match max_whitespace_cnt {
        None => "[ \\n\\t]*".to_owned(),
        Some(n) => format!("[ \\n\\t]{{0,{n}}}"),
    }
}

fn json_str_to_printable_str(json_str: &str) -> String {
    json_str.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Default)]
struct TrieNode {
    is_terminal: bool,
    children: BTreeMap<u8, TrieNode>,
}

fn build_trie_body(node: &TrieNode) -> String {
    let mut parts: Vec<String> = Vec::new();

    if !node.is_terminal {
        parts.push("\"\\\"\"".to_owned());
    }

    let mut neg = String::from("[^");
    for &c in node.children.keys() {
        if c == b']' || c == b'\\' || c == b'^' || c == b'-' {
            neg.push('\\');
        }
        neg.push(c as char);
    }
    neg.push_str("\\0-\\x1f\\\"\\\\\\r\\n]");
    parts.push(format!("{neg} {BASIC_STRING_SUB}"));

    parts.push(format!("\"\\\\\" {BASIC_ESCAPE} {BASIC_STRING_SUB}"));

    for (&c, child) in &node.children {
        let child_body = build_trie_body(child);
        let char_lit = if c == b'"' {
            "\"\\\"\"".to_owned()
        } else if c == b'\\' {
            "\"\\\\\"".to_owned()
        } else {
            format!("\"{}\"", c as char)
        };
        parts.push(format!("{char_lit} {child_body}"));
    }

    format!("({})", parts.join(" | "))
}

/// The JSON-schema `format` → regex table.
fn json_format_to_regex_pattern(format: &str) -> Option<String> {
    let atext = "[\\w!#$%&'*+/=?^`{|}~-]";
    let dot_string = format!("({atext}+(\\.{atext}+)*)");
    let quoted_string =
        "\\\\\"(\\\\[\\x20-\\x7E]|[\\x20\\x21\\x23-\\x5B\\x5D-\\x7E])*\\\\\"";
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
