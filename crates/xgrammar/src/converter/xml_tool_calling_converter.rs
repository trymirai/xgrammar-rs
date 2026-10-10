//! The XML tool-calling converter (Qwen, MiniMax, DeepSeek, GLM, Kimi-K3 parameter lists).

use std::collections::HashMap;

use serde_json::Value;

use super::{
    json_format::JsonFormat,
    json_schema_converter::{
        BASIC_ANY, BASIC_ARRAY, BASIC_OBJECT, BASIC_STRING, JsonSchemaConverter, json_format_to_regex_pattern,
        parse_literal,
    },
    schema_converter::SchemaConverter,
    schema_error::SchemaError,
    schema_parser::serialize_json,
    schema_spec::{
        ArraySpec, ConstSpec, EnumSpec, ObjectSpec, Property, SchemaSpec, SchemaSpecPtr, SchemaSpecVariant, StringSpec,
    },
    xml_converter::XmlConverter,
    xml_wrapper::XmlWrapper,
};

pub const XML_STRING: &str = "xml_string";
pub const XML_ANY: &str = "xml_any";
pub const XML_OBJECT: &str = "xml_object";
pub const XML_VARIABLE_NAME: &str = "xml_variable_name";

const OBJECT_CACHE_KEY: &str = "{\"type\":\"object\"}";

/// Converts a JSON Schema into an XML tool call: the root object is a list of
/// `<key>value</key>`-style parameters whose values are raw strings or JSON.
pub struct XmlToolCallingConverter {
    pub base: JsonSchemaConverter,
    pub format: JsonFormat,
    /// 0 at the parameter list, 1 inside a parameter value, 2 and deeper inside JSON values.
    pub nested_object_level: i32,
    pub wrapper: XmlWrapper,
    /// DeepSeek parameter suffix rules keyed by `$ref` URI; they do not depend on the key.
    pub deepseek_param_ref_rules: HashMap<String, i32>,
}

impl XmlToolCallingConverter {
    /// Creates the converter for one of the XML parameter-list formats.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if `format` is not an XML parameter-list format, or if an
    /// exclusion starts or ends with whitespace (which the formatting could split).
    pub fn new(
        base: JsonSchemaConverter,
        format: JsonFormat,
    ) -> Result<Self, SchemaError> {
        let wrapper = XmlWrapper::for_format(format)
            .ok_or_else(|| SchemaError::invalid(format!("Invalid JSON format for XML tool calling: {format:?}")))?;
        let is_padding = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
        if base.excludes.iter().any(|excluded| excluded.starts_with(is_padding) || excluded.ends_with(is_padding)) {
            return Err(SchemaError::invalid(
                "XML JSONSchemaFormat.excludes must not start or end with formatting whitespace",
            ));
        }
        Ok(Self {
            base,
            format,
            nested_object_level: 0,
            wrapper,
            deepseek_param_ref_rules: HashMap::new(),
        })
    }

    /// The cache context: the parameter list, a raw parameter value, or nested JSON.
    #[must_use]
    pub fn encoding_context(&self) -> i32 {
        self.nested_object_level.min(2)
    }

    /// The single JSON type a value of `spec` is rendered with, when it is known without
    /// resolving references or combinators. Integers and numbers are both `"number"`.
    #[must_use]
    pub fn rendered_json_type(spec: Option<&SchemaSpecPtr>) -> Option<&'static str> {
        let type_of_json_value = |json_value: &str| match parse_literal(json_value) {
            Value::String(_) => "string",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::Null => "null",
            Value::Object(_) => "object",
            Value::Array(_) => "array",
        };
        match &spec?.spec {
            SchemaSpecVariant::String(_) => Some("string"),
            SchemaSpecVariant::Integer(_) | SchemaSpecVariant::Number(_) => Some("number"),
            SchemaSpecVariant::Boolean => Some("boolean"),
            SchemaSpecVariant::Null => Some("null"),
            SchemaSpecVariant::Array(_) => Some("array"),
            SchemaSpecVariant::Object(_) => Some("object"),
            SchemaSpecVariant::Const(spec) => Some(type_of_json_value(&spec.json_value)),
            // Only a single type when every alternative renders with it.
            SchemaSpecVariant::Enum(spec) => {
                let mut types = spec.json_values.iter().map(|value| type_of_json_value(value));
                let first = types.next()?;
                types.all(|ty| ty == first).then_some(first)
            },
            _ => None,
        }
    }

    /// The raw text of a JSON literal: a string's content, or the JSON text of other values.
    #[must_use]
    pub fn xml_value(json_value: &str) -> String {
        match parse_literal(json_value) {
            Value::String(text) => text,
            _ => json_value.to_owned(),
        }
    }

    /// Escapes an attribute value as Kimi-K3's renderer does (`&` and `"`); other formats keep it.
    #[must_use]
    pub fn escape_attr_value(
        &self,
        value: &str,
    ) -> String {
        if self.format != JsonFormat::KimiK3Xml {
            return value.to_owned();
        }
        value.replace('&', "&amp;").replace('"', "&quot;")
    }

    /// Runs `generate` one nesting level deeper.
    fn nested<T>(
        &mut self,
        generate: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.nested_object_level += 1;
        let result = generate(self);
        self.nested_object_level -= 1;
        result
    }
}

impl XmlConverter for XmlToolCallingConverter {
    fn xml(&self) -> &XmlToolCallingConverter {
        self
    }

    fn xml_mut(&mut self) -> &mut XmlToolCallingConverter {
        self
    }
}

impl SchemaConverter for XmlToolCallingConverter {
    fn base(&self) -> &JsonSchemaConverter {
        &self.base
    }

    fn base_mut(&mut self) -> &mut JsonSchemaConverter {
        &mut self.base
    }

    fn add_basic_rules(&mut self) -> Result<(), SchemaError> {
        self.xml_add_basic_rules()
    }

    fn add_cache(
        &mut self,
        key: &str,
        rule_id: i32,
    ) {
        if !key.is_empty() {
            let context = self.encoding_context();
            self.base.rule_cache.insert((key.to_owned(), context), rule_id);
        }
    }

    fn get_cache(
        &self,
        key: &str,
    ) -> Option<i32> {
        if key.is_empty() {
            return None;
        }
        let parameter_list_any =
            matches!(self.format, JsonFormat::QwenXml | JsonFormat::DeepSeekXml | JsonFormat::DeepSeekV41Xml)
                && self.nested_object_level == 0
                && key == "{}";
        let cache_key = if parameter_list_any {
            // Unconstrained tool arguments are a parameter list, not one parameter's raw value.
            (OBJECT_CACHE_KEY.to_owned(), 0)
        } else if self.nested_object_level == 1 && key == OBJECT_CACHE_KEY {
            // An object parameter value is a JSON object with braces, not a parameter list.
            (key.to_owned(), 2)
        } else {
            (key.to_owned(), self.encoding_context())
        };
        self.base.rule_cache.get(&cache_key).copied()
    }

    fn ref_cache_key(
        &self,
        uri: &str,
    ) -> String {
        format!("{}:{uri}", self.encoding_context())
    }

    fn next_separator(
        &mut self,
        is_end: bool,
    ) -> String {
        if self.nested_object_level <= 1 {
            return self.base.whitespace_pattern();
        }
        self.base.indent_manager.next_separator(is_end)
    }

    fn key_pattern(&self) -> &'static str {
        if self.nested_object_level <= 1 {
            XML_VARIABLE_NAME
        } else {
            BASIC_STRING
        }
    }

    fn basic_any_rule_name(&self) -> &'static str {
        if self.nested_object_level <= 1 {
            XML_ANY
        } else {
            BASIC_ANY
        }
    }

    fn key_pattern_excluding(
        &mut self,
        properties: &[Property],
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if self.nested_object_level <= 1 {
            return Ok(self.key_pattern_expression());
        }
        self.json_key_pattern_excluding(properties, rule_name)
    }

    fn generate_string(
        &mut self,
        spec: &StringSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if self.nested_object_level > 1 {
            return self.json_generate_string(spec, rule_name);
        }
        if let Some(regex) = spec.format.as_deref().and_then(json_format_to_regex_pattern) {
            return self.base.regex_expression(&regex, false, true);
        }
        if let Some(pattern) = &spec.pattern {
            return self.base.regex_expression(pattern, false, true);
        }
        let bounded = spec.min_length != 0 || spec.max_length != -1;
        // Without exclusions, a length bound or an unrecognized format keeps the plain repetition.
        if self.base.excludes.is_empty() && (bounded || spec.format.is_some()) {
            return Ok(self.base.raw_characters(spec, rule_name));
        }
        // With exclusions the raw string keeps only the exclusions.
        if bounded {
            JsonSchemaConverter::warn_dropped_length_constraints(spec, rule_name);
        }
        Ok(self.base.rule_ref_by_name(XML_STRING))
    }

    fn generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.nested(|converter| converter.json_generate_array(spec, rule_name))
    }

    fn generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.nested(|converter| {
            let need_braces = converter.nested_object_level > 1;
            converter.json_generate_object(spec, rule_name, need_braces)
        })
    }

    fn generate_any(
        &mut self,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        Ok(match self.nested_object_level {
            0 => self.base.rule_ref_by_name(XML_OBJECT),
            1 => {
                let choices = [XML_STRING, BASIC_ARRAY, BASIC_OBJECT].map(|name| self.base.rule_ref_by_name(name));
                self.base.choice(&choices)
            },
            _ => self.json_generate_any(),
        })
    }

    fn generate_const(
        &mut self,
        spec: &ConstSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if self.nested_object_level == 0 {
            if let Value::Object(members) = parse_literal(&spec.json_value) {
                // A root object is a parameter list, including when all its values are fixed.
                let mut object = ObjectSpec {
                    allow_unevaluated_properties: false,
                    ..ObjectSpec::default()
                };
                for (key, value) in members {
                    let schema = SchemaSpec::make(
                        SchemaSpecVariant::Const(ConstSpec {
                            json_value: serialize_json(&value),
                        }),
                        "",
                    );
                    object.required.insert(key.clone());
                    object.properties.push(Property {
                        name: key,
                        schema,
                    });
                }
                // A fixed order even with any_order: the any-order rule admits repeated keys.
                let saved_any_order = std::mem::replace(&mut self.base.any_order, false);
                let result = self.generate_object(&object, rule_name);
                self.base.any_order = saved_any_order;
                return result;
            }
        }
        if self.nested_object_level > 1 {
            return Ok(self.json_generate_literals(std::slice::from_ref(&spec.json_value)));
        }
        if !self.base.is_allowed_json_literal(&spec.json_value, true) {
            return Ok(self.base.unsatisfiable());
        }
        Ok(self.base.byte_string(Self::xml_value(&spec.json_value).as_bytes()))
    }

    fn generate_enum(
        &mut self,
        spec: &EnumSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if self.nested_object_level > 1 {
            return Ok(self.json_generate_literals(&spec.json_values));
        }
        let mut values = Vec::with_capacity(spec.json_values.len());
        for json_value in &spec.json_values {
            let literal = ConstSpec {
                json_value: json_value.clone(),
            };
            values.push(self.generate_const(&literal, rule_name)?);
        }
        Ok(self.base.choice(&values))
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
        self.xml_format_property(key, value_rule_id, schema)
    }

    fn format_other_property(
        &mut self,
        key_pattern: i32,
        value_rule_id: i32,
        _rule_name: &str,
        _rule_name_suffix: &str,
        schema: Option<&SchemaSpecPtr>,
    ) -> Result<i32, SchemaError> {
        self.xml_format_other_property(key_pattern, value_rule_id, schema)
    }
}
