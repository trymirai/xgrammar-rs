//! The Cohere XML tool-calling converter.

use std::collections::HashMap;

use super::{
    json_format::JsonFormat,
    json_schema_converter::{
        BASIC_BOOLEAN, BASIC_NULL, BASIC_NUMBER, JsonSchemaConverter, json_format_to_regex_pattern, parse_literal,
    },
    key_trie::KeyTrie,
    schema_converter::SchemaConverter,
    schema_error::SchemaError,
    schema_spec::{
        ArraySpec, ConstSpec, EnumSpec, ObjectSpec, Property, SchemaSpec, SchemaSpecPtr, SchemaSpecVariant, StringSpec,
    },
    xml_converter::XmlConverter,
    xml_tool_calling_converter::{XML_OBJECT, XML_STRING, XmlToolCallingConverter},
};
use crate::grammar::CharacterClassElement;

const COHERE_KEY: &str = "cohere_key";
const COHERE_ANY_SCALAR: &str = "cohere_any_scalar";
const COHERE_ANY_LIST: &str = "cohere_any_list";

/// The XML-sensitive key characters and the entities that spell them.
const KEY_ENTITIES: [(char, &str); 4] = [('&', "&amp;"), ('<', "&lt;"), ('>', "&gt;"), ('"', "&quot;")];

/// Converts a JSON Schema into recursive Cohere value tags,
/// `<cofl:value name="key" type="raw|json|dict|list">value</cofl:value>`: object members are
/// named value tags and array items unnamed ones.
pub struct CohereXmlToolCallingConverter {
    xml: XmlToolCallingConverter,
    /// The number of Cohere objects being generated.
    object_depth: usize,
    /// The number of Cohere arrays being generated.
    array_level: usize,
    /// Parameter suffix rules keyed by `$ref` URI; see [`Self::format_param_suffix`].
    param_ref_rules: HashMap<String, i32>,
}

impl CohereXmlToolCallingConverter {
    /// Creates the converter.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the XML state cannot be created.
    pub fn new(base: JsonSchemaConverter) -> Result<Self, SchemaError> {
        Ok(Self {
            xml: XmlToolCallingConverter::new(base, JsonFormat::CohereXml)?,
            object_depth: 0,
            array_level: 0,
            param_ref_rules: HashMap::new(),
        })
    }

    fn at_root(&self) -> bool {
        self.xml.nested_object_level == 0 && self.object_depth == 0 && self.array_level == 0
    }

    /// Whether values are Cohere tags here (dict and list contents stay tagged at any depth).
    fn in_value_context(&self) -> bool {
        self.xml.nested_object_level <= 1 || self.object_depth > 0 || self.array_level > 0
    }

    /// `<cofl:value` plus the optional ` name="..."` attribute of one parameter.
    fn param_prefix(
        &mut self,
        name: Option<&str>,
        key_pattern: Option<i32>,
    ) -> Result<i32, SchemaError> {
        let key_prefix = self.xml.wrapper.key_prefix;
        let base = &mut self.xml.base;
        let mut elements = vec![base.byte_string(key_prefix.as_bytes())];
        if let Some(name) = name {
            let attribute = format!(" name=\"{}\"", serialize_key(name)?);
            elements.push(base.byte_string(attribute.as_bytes()));
        } else if let Some(key_pattern) = key_pattern {
            elements.push(base.byte_string(b" name=\""));
            elements.push(key_pattern);
            elements.push(base.byte_string(b"\""));
        }
        Ok(base.sequence(&elements))
    }

    /// The type attribute, value, and closing tag of one parameter.
    fn suffix_with_type(
        &mut self,
        type_expr: i32,
        value_rule_id: i32,
    ) -> i32 {
        let wrapper = self.xml.wrapper;
        let base = &mut self.xml.base;
        let mut elements = vec![base.byte_string(b" type=\""), type_expr];
        elements.push(base.byte_string(format!("\"{}", wrapper.key_suffix).as_bytes()));
        if !wrapper.value_prefix.is_empty() {
            elements.push(base.byte_string(wrapper.value_prefix.as_bytes()));
        }
        if value_rule_id == base.builder.get_rule_id(XML_STRING) {
            elements.push(base.rule_ref(value_rule_id));
        } else {
            elements.push(base.whitespace_expression());
            elements.push(base.rule_ref(value_rule_id));
            elements.push(base.whitespace_expression());
        }
        elements.push(base.byte_string(wrapper.parameter_suffix.as_bytes()));
        base.sequence(&elements)
    }

    /// The suffix of an unconstrained parameter. Each type wrapper references its matching
    /// component of the Any union, instead of a type/body cross product.
    fn any_suffix(&mut self) -> i32 {
        let mut choices = Vec::with_capacity(4);
        for (type_name, rule_name) in
            [("raw", XML_STRING), ("json", COHERE_ANY_SCALAR), ("dict", XML_OBJECT), ("list", COHERE_ANY_LIST)]
        {
            let type_expr = self.xml.base.byte_string(type_name.as_bytes());
            let rule_id = self.xml.base.builder.get_rule_id(rule_name);
            choices.push(self.suffix_with_type(type_expr, rule_id));
        }
        self.xml.base.choice(&choices)
    }

    /// A whole parameter correlated with `schema`.
    fn format_param(
        &mut self,
        name: Option<&str>,
        key_pattern: Option<i32>,
        schema: &SchemaSpecPtr,
        value_rule_id: i32,
    ) -> Result<i32, SchemaError> {
        let prefix = self.param_prefix(name, key_pattern)?;
        let suffix = self.format_param_suffix(schema, value_rule_id)?;
        Ok(self.xml.base.sequence(&[prefix, suffix]))
    }

    /// The type attribute, value, and closing tag of one parameter, correlated with `schema`.
    /// The suffix does not depend on the name, so `$ref` schemas get one memoized rule per URI,
    /// registered before the reference is resolved so that recursive references point back at it.
    fn format_param_suffix(
        &mut self,
        schema: &SchemaSpecPtr,
        value_rule_id: i32,
    ) -> Result<i32, SchemaError> {
        let value_rule_name = self.xml.base.builder.get_rule(value_rule_id).name.clone();
        match &schema.spec {
            SchemaSpecVariant::Ref(reference) => {
                if let Some(&rule_id) = self.param_ref_rules.get(&reference.uri) {
                    return Ok(self.xml.base.rule_ref(rule_id));
                }
                let param_rule_id =
                    self.xml.base.builder.add_empty_rule_with_hint(&format!("{value_rule_name}_cohere_param"));
                self.param_ref_rules.insert(reference.uri.clone(), param_rule_id);
                let resolved = self.xml.base.resolve_ref(&reference.uri)?;
                let body = self.format_param_suffix(&resolved, value_rule_id)?;
                self.xml.base.builder.update_rule_body(param_rule_id, body);
                return Ok(self.xml.base.rule_ref(param_rule_id));
            },
            // Multi-branch allOf falls back to Any in the base converter; keep it canonical.
            SchemaSpecVariant::Any => return Ok(self.any_suffix()),
            SchemaSpecVariant::AllOf(spec) if spec.schemas.len() != 1 => return Ok(self.any_suffix()),
            _ => {},
        }
        let Some(options) = composite_options(schema) else {
            let type_name = match &schema.spec {
                SchemaSpecVariant::String(_) => "raw",
                SchemaSpecVariant::Object(_) => "dict",
                SchemaSpecVariant::Array(_) => "list",
                SchemaSpecVariant::Const(spec) => type_for_literal(&spec.json_value),
                SchemaSpecVariant::Enum(spec) => common_type(&spec.json_values).unwrap_or("json"),
                _ => "json",
            };
            let type_expr = self.xml.base.byte_string(type_name.as_bytes());
            return Ok(self.suffix_with_type(type_expr, value_rule_id));
        };
        let mut choices = Vec::with_capacity(options.len());
        for (index, option) in options.iter().enumerate() {
            let option_rule_id = self.create_rule(option, &format!("{value_rule_name}_cohere_case_{index}"))?;
            choices.push(self.format_param_suffix(option, option_rule_id)?);
        }
        Ok(self.xml.base.choice(&choices))
    }

    /// A key that is none of the trie's keys, from the given trie node.
    fn key_excluding_body(
        &mut self,
        node: &KeyTrie<char>,
        depth: usize,
    ) -> i32 {
        let base = &mut self.xml.base;
        let mut choices = Vec::new();
        if depth > 0 && !node.is_terminal {
            choices.push(base.empty());
        }
        let empty = base.empty();
        let key = base.rule_ref_by_name(COHERE_KEY);
        let optional_key_suffix = base.choice(&[empty, key]);
        let ordinary = base.builder.add_character_class(&ordinary_key_ranges_excluding(node.children.keys()), false);
        choices.push(base.sequence(&[ordinary, optional_key_suffix]));
        for (character, entity) in KEY_ENTITIES {
            if !node.children.contains_key(&character) {
                let entity = base.byte_string(entity.as_bytes());
                choices.push(base.sequence(&[entity, optional_key_suffix]));
            }
        }
        for (&character, child) in &node.children {
            let spelled = self.xml.base.byte_string(serialize_key_char(character).as_bytes());
            let rest = self.key_excluding_body(child, depth + 1);
            choices.push(self.xml.base.sequence(&[spelled, rest]));
        }
        self.xml.base.choice(&choices)
    }
}

impl XmlConverter for CohereXmlToolCallingConverter {
    fn xml(&self) -> &XmlToolCallingConverter {
        &self.xml
    }

    fn xml_mut(&mut self) -> &mut XmlToolCallingConverter {
        &mut self.xml
    }
}

impl SchemaConverter for CohereXmlToolCallingConverter {
    fn base(&self) -> &JsonSchemaConverter {
        &self.xml.base
    }

    fn base_mut(&mut self) -> &mut JsonSchemaConverter {
        &mut self.xml.base
    }

    fn add_basic_rules(&mut self) -> Result<(), SchemaError> {
        // The dynamic key and recursive Any rules need stable targets before xml_object is built.
        for name in [COHERE_KEY, COHERE_ANY_SCALAR, COHERE_ANY_LIST] {
            self.xml.base.builder.add_empty_rule(name);
        }
        self.xml_add_basic_rules()?;
        let base = &mut self.xml.base;
        let key = base.regex_expression(r#"([^\x00"&<>]|&amp;|&lt;|&gt;|&quot;)+"#, false, false)?;
        base.builder.update_rule_body_by_name(COHERE_KEY, key);
        let scalars = [BASIC_NUMBER, BASIC_BOOLEAN, BASIC_NULL].map(|name| base.rule_ref_by_name(name));
        let scalar = base.choice(&scalars);
        base.builder.update_rule_body_by_name(COHERE_ANY_SCALAR, scalar);
        // xml_object already provides the named Any members; lists need unnamed Any items.
        let unnamed_any_value = self.param_prefix(None, None)?;
        let any_suffix = self.any_suffix();
        let base = &mut self.xml.base;
        let item = base.sequence(&[unnamed_any_value, any_suffix]);
        let list = base.repeat("cohere_any_list_items", item, 0, -1);
        base.builder.update_rule_body_by_name(COHERE_ANY_LIST, list);
        Ok(())
    }

    fn add_cache(
        &mut self,
        key: &str,
        rule_id: i32,
    ) {
        if !key.is_empty() {
            let context = i32::from(self.xml.nested_object_level > 1 && !self.in_value_context());
            self.xml.base.rule_cache.insert((key.to_owned(), context), rule_id);
        }
    }

    fn get_cache(
        &self,
        key: &str,
    ) -> Option<i32> {
        if key.is_empty() {
            return None;
        }
        // `true` and {} are equivalent: an unrestricted dictionary at the tool-arguments root.
        if self.at_root() && (key == "{}" || key == "true") {
            return Some(self.xml.base.builder.get_rule_id(XML_OBJECT));
        }
        let context = i32::from(self.xml.nested_object_level > 1 && !self.in_value_context());
        self.xml.base.rule_cache.get(&(key.to_owned(), context)).copied()
    }

    fn ref_cache_key(
        &self,
        uri: &str,
    ) -> String {
        // Recursive dict/list contents remain tagged values regardless of depth.
        let context = if self.at_root() {
            0
        } else if self.in_value_context() {
            1
        } else {
            2
        };
        format!("{context}:{uri}")
    }

    fn next_separator(
        &mut self,
        is_end: bool,
    ) -> String {
        if self.in_value_context() {
            return self.xml.base.whitespace_pattern();
        }
        self.xml.base.indent_manager.next_separator(is_end)
    }

    fn key_pattern(&self) -> &'static str {
        if self.in_value_context() {
            COHERE_KEY
        } else {
            super::json_schema_converter::BASIC_STRING
        }
    }

    fn basic_any_rule_name(&self) -> &'static str {
        self.xml.basic_any_rule_name()
    }

    fn key_pattern_excluding(
        &mut self,
        properties: &[Property],
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            return self.json_key_pattern_excluding(properties, rule_name);
        }
        if properties.is_empty() {
            return Ok(self.key_pattern_expression());
        }
        for property in properties {
            check_key(&property.name)?;
        }
        let trie = KeyTrie::new(
            properties.iter().filter(|property| !property.name.is_empty()).map(|property| property.name.chars()),
        );
        let key_rule_id = self.xml.base.builder.add_empty_rule_with_hint(&format!("{rule_name}_cohere_addl_key"));
        let body = self.key_excluding_body(&trie, 0);
        self.xml.base.builder.update_rule_body(key_rule_id, body);
        Ok(self.xml.base.rule_ref(key_rule_id))
    }

    fn generate_string(
        &mut self,
        spec: &StringSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            return self.json_generate_string(spec, rule_name);
        }
        let bounded = spec.min_length != 0 || spec.max_length != -1;
        if spec.pattern.is_none() && spec.format.is_none() && !bounded {
            return Ok(self.xml.base.rule_ref_by_name(XML_STRING));
        }
        if let Some(regex) = spec.format.as_deref().and_then(json_format_to_regex_pattern) {
            return self.xml.base.regex_expression(&regex, false, true);
        }
        if let Some(pattern) = &spec.pattern {
            return self.xml.base.regex_expression(pattern, false, true);
        }
        if bounded {
            return Ok(self.xml.base.raw_characters(spec, rule_name));
        }
        self.json_generate_string(spec, rule_name)
    }

    fn generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            self.xml.nested_object_level += 1;
            let result = self.json_generate_array(spec, rule_name);
            self.xml.nested_object_level -= 1;
            return result;
        }
        self.array_level += 1;
        let patterns = self.array_item_patterns(spec, rule_name);
        self.array_level -= 1;
        let (item_patterns, additional_item_pattern) = patterns?;

        let base = &mut self.xml.base;
        let max_count = |offset: i64| {
            if spec.max_items == -1 {
                -1
            } else {
                (spec.max_items - offset) as i32
            }
        };
        if item_patterns.is_empty() {
            let Some(additional) = additional_item_pattern.filter(|_| spec.max_items != 0) else {
                return Ok(base.empty());
            };
            return Ok(base.repeat(&format!("{rule_name}_items"), additional, spec.min_items as i32, max_count(0)));
        }
        let prefix = base.sequence(&item_patterns);
        let Some(additional) = additional_item_pattern else {
            return Ok(prefix);
        };
        let item_count = item_patterns.len() as i64;
        let minimum_additional = (spec.min_items - item_count).max(0) as i32;
        let rest = base.repeat(
            &format!("{rule_name}_additional_items"),
            additional,
            minimum_additional,
            max_count(item_count),
        );
        Ok(base.sequence(&[prefix, rest]))
    }

    fn generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.xml.nested_object_level += 1;
        let result = if self.in_value_context() {
            self.object_depth += 1;
            let result = self.json_generate_object(spec, rule_name, false);
            self.object_depth -= 1;
            result
        } else {
            let need_braces = self.xml.nested_object_level > 1;
            self.json_generate_object(spec, rule_name, need_braces)
        };
        self.xml.nested_object_level -= 1;
        result
    }

    fn generate_any(
        &mut self,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            return Ok(self.json_generate_any());
        }
        let at_root = self.at_root();
        let base = &mut self.xml.base;
        if at_root {
            return Ok(base.rule_ref_by_name(XML_OBJECT));
        }
        let choices =
            [XML_STRING, COHERE_ANY_SCALAR, XML_OBJECT, COHERE_ANY_LIST].map(|name| base.rule_ref_by_name(name));
        Ok(base.choice(&choices))
    }

    fn generate_const(
        &mut self,
        spec: &ConstSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            return Ok(self.json_generate_literals(std::slice::from_ref(&spec.json_value)));
        }
        let value = XmlToolCallingConverter::xml_value(&spec.json_value);
        Ok(self.xml.base.byte_string(value.as_bytes()))
    }

    fn generate_enum(
        &mut self,
        spec: &EnumSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if !self.in_value_context() {
            return Ok(self.json_generate_literals(&spec.json_values));
        }
        let base = &mut self.xml.base;
        let values: Vec<i32> = spec
            .json_values
            .iter()
            .map(|value| base.byte_string(XmlToolCallingConverter::xml_value(value).as_bytes()))
            .collect();
        Ok(base.choice(&values))
    }

    fn format_property_key(
        &mut self,
        key: &str,
        schema: &SchemaSpecPtr,
    ) -> i32 {
        self.xml_format_property_key(key, schema)
    }

    fn format_property(
        &mut self,
        key: &str,
        value_rule_id: i32,
        _rule_name: &str,
        _index: usize,
        schema: &SchemaSpecPtr,
    ) -> Result<i32, SchemaError> {
        if self.object_depth > 0 {
            return self.format_param(Some(key), None, schema, value_rule_id);
        }
        self.xml_format_property(key, value_rule_id, schema)
    }

    fn format_other_property(
        &mut self,
        key_pattern: i32,
        value_rule_id: i32,
        rule_name: &str,
        rule_name_suffix: &str,
        schema: Option<&SchemaSpecPtr>,
    ) -> Result<i32, SchemaError> {
        if let Some(schema) = schema {
            return self.format_param(None, Some(key_pattern), schema, value_rule_id);
        }
        if self.in_value_context() {
            let any = SchemaSpec::make(SchemaSpecVariant::Any, "");
            let any_rule_id = self.create_rule(&any, &format!("{rule_name}_{rule_name_suffix}_cohere_any"))?;
            return self.format_param(None, Some(key_pattern), &any, any_rule_id);
        }
        self.xml_format_other_property(key_pattern, value_rule_id, schema)
    }
}

impl CohereXmlToolCallingConverter {
    /// The unnamed item parameters of a Cohere list: the prefix items and the additional item.
    fn array_item_patterns(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<(Vec<i32>, Option<i32>), SchemaError> {
        let mut item_patterns = Vec::with_capacity(spec.prefix_items.len());
        for (index, item) in spec.prefix_items.iter().enumerate() {
            let item_rule_id = self.create_rule(item, &format!("{rule_name}_item_{index}"))?;
            item_patterns.push(self.format_param(None, None, item, item_rule_id)?);
        }
        let additional_item_pattern = match (&spec.additional_items, spec.allow_additional_items) {
            (Some(additional), true) => {
                let additional_rule_id = self.create_rule(additional, &format!("{rule_name}_additional"))?;
                Some(self.format_param(None, None, additional, additional_rule_id)?)
            },
            _ => None,
        };
        Ok((item_patterns, additional_item_pattern))
    }
}

/// The alternatives of a schema whose options render with different Cohere types: unions,
/// single-schema allOf, and enums mixing raw and JSON literals.
fn composite_options(schema: &SchemaSpecPtr) -> Option<Vec<SchemaSpecPtr>> {
    match &schema.spec {
        SchemaSpecVariant::AnyOf(spec) => Some(spec.options.clone()),
        SchemaSpecVariant::AllOf(spec) if spec.schemas.len() == 1 => Some(spec.schemas.clone()),
        SchemaSpecVariant::TypeArray(spec) => Some(spec.type_schemas.clone()),
        SchemaSpecVariant::Enum(spec) if !spec.json_values.is_empty() && common_type(&spec.json_values).is_none() => {
            Some(
                spec.json_values
                    .iter()
                    .map(|value| {
                        SchemaSpec::make(
                            SchemaSpecVariant::Const(ConstSpec {
                                json_value: value.clone(),
                            }),
                            "",
                        )
                    })
                    .collect(),
            )
        },
        _ => None,
    }
}

/// The Cohere type of a literal: strings are raw text, everything else is JSON.
fn type_for_literal(json_value: &str) -> &'static str {
    if parse_literal(json_value).is_string() {
        "raw"
    } else {
        "json"
    }
}

/// The Cohere type shared by all literals, if they agree.
fn common_type(json_values: &[String]) -> Option<&'static str> {
    let mut types = json_values.iter().map(|value| type_for_literal(value));
    let first = types.next()?;
    types.all(|ty| ty == first).then_some(first)
}

/// Rejects property names that cannot be spelled as a Cohere name attribute.
fn check_key(key: &str) -> Result<(), SchemaError> {
    if key.contains('\0') {
        return Err(SchemaError::invalid("Cohere property names cannot contain NUL"));
    }
    Ok(())
}

/// A property name as a Cohere name attribute, with XML-sensitive characters as entities.
fn serialize_key(key: &str) -> Result<String, SchemaError> {
    check_key(key)?;
    Ok(key.chars().map(serialize_key_char).collect())
}

fn serialize_key_char(character: char) -> String {
    KEY_ENTITIES
        .iter()
        .find(|(entity_character, _)| *entity_character == character)
        .map_or_else(|| character.to_string(), |(_, entity)| (*entity).to_owned())
}

/// The ordinary key characters that are neither NUL, XML-sensitive (spelled as entities), nor
/// one of `children` (handled by a trie branch), as positive codepoint ranges.
fn ordinary_key_ranges_excluding<'a>(children: impl Iterator<Item = &'a char>) -> Vec<CharacterClassElement> {
    let mut excluded: Vec<i32> = KEY_ENTITIES.iter().map(|(character, _)| *character as i32).collect();
    excluded.push(0);
    excluded.extend(children.map(|&character| character as i32));
    excluded.sort_unstable();
    excluded.dedup();
    let mut ranges = Vec::with_capacity(excluded.len() + 1);
    let mut range_start = 0;
    for codepoint in excluded {
        if range_start < codepoint {
            ranges.push(CharacterClassElement::new(range_start, codepoint - 1));
        }
        range_start = codepoint + 1;
    }
    if range_start <= 0x10_FFFF {
        ranges.push(CharacterClassElement::new(range_start, 0x10_FFFF));
    }
    ranges
}
