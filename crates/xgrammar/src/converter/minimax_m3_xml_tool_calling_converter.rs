//! The MiniMax-M3 XML tool-calling converter.

use std::collections::HashSet;

use serde_json::Value;

use super::{
    json_schema_converter::{
        BASIC_BOOLEAN, BASIC_INTEGER, BASIC_NULL, BASIC_NUMBER, BASIC_STRING, JsonSchemaConverter,
        json_format_to_regex_pattern, parse_literal,
    },
    schema_converter::SchemaConverter,
    schema_error::SchemaError,
    schema_parser::serialize_json,
    schema_spec::{ArraySpec, ConstSpec, EnumSpec, IntegerSpec, NumberSpec, ObjectSpec, SchemaSpecPtr, StringSpec},
};

/// The namespace marker that prefixes every MiniMax-M3 element tag.
const NAMESPACE: &str = "]<]minimax[>[";
/// The element name of array items.
const ARRAY_ITEM_NAME: &str = "item";

/// Converts a JSON Schema into MiniMax-M3's recursive namespace-prefixed elements:
/// `]<]minimax[>[<name>value]<]minimax[>[</name>`, where objects are sequences of named
/// elements and arrays sequences of `item` elements. Element names must be known when the
/// grammar is built, so schemas requiring runtime names are rejected.
pub struct MiniMaxM3XmlToolCallingConverter {
    base: JsonSchemaConverter,
}

impl MiniMaxM3XmlToolCallingConverter {
    /// Creates the converter.
    #[must_use]
    pub fn new(base: JsonSchemaConverter) -> Self {
        Self {
            base,
        }
    }

    /// One element named `name` around the value rule.
    fn format_element(
        &mut self,
        name: &str,
        value_rule_id: i32,
    ) -> Result<i32, SchemaError> {
        validate_element_name(name)?;
        let open = self.base.byte_string(format!("{NAMESPACE}<{name}>").as_bytes());
        let value = self.base.rule_ref(value_rule_id);
        let close = self.base.byte_string(format!("{NAMESPACE}</{name}>").as_bytes());
        Ok(self.base.sequence(&[open, value, close]))
    }

    /// A literal value: strings are raw text, objects and arrays nested elements.
    fn generate_literal(
        &mut self,
        value: &Value,
    ) -> Result<i32, SchemaError> {
        match value {
            Value::String(text) => {
                if text.contains(NAMESPACE) {
                    return Err(SchemaError::invalid(
                        "A minimax_m3_xml string literal cannot contain the namespace marker",
                    ));
                }
                Ok(self.base.byte_string(text.as_bytes()))
            },
            Value::Object(members) => {
                let mut properties = Vec::with_capacity(members.len());
                for (key, member) in members {
                    let value_expr = self.generate_literal(member)?;
                    let value_rule_id = self.base.builder.add_rule_with_hint(&format!("literal_{key}"), value_expr);
                    properties.push(self.format_element(key, value_rule_id)?);
                }
                Ok(self.base.sequence(&properties))
            },
            Value::Array(items) => {
                let mut elements = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let value_expr = self.generate_literal(item)?;
                    let value_rule_id =
                        self.base.builder.add_rule_with_hint(&format!("literal_item_{index}"), value_expr);
                    elements.push(self.format_element(ARRAY_ITEM_NAME, value_rule_id)?);
                }
                Ok(self.base.sequence(&elements))
            },
            _ => Ok(self.base.byte_string(serialize_json(value).as_bytes())),
        }
    }
}

impl SchemaConverter for MiniMaxM3XmlToolCallingConverter {
    fn base(&self) -> &JsonSchemaConverter {
        &self.base
    }

    fn base_mut(&mut self) -> &mut JsonSchemaConverter {
        &mut self.base
    }

    fn add_basic_rules(&mut self) -> Result<(), SchemaError> {
        for name in [BASIC_INTEGER, BASIC_NUMBER, BASIC_STRING, BASIC_BOOLEAN, BASIC_NULL] {
            self.base.builder.add_empty_rule(name);
        }
        let body = self.base.generate_integer(&IntegerSpec::default(), BASIC_INTEGER)?;
        self.set_basic_rule(BASIC_INTEGER, body, "{\"type\":\"integer\"}");
        let body = self.base.generate_number(&NumberSpec::default(), BASIC_NUMBER)?;
        self.set_basic_rule(BASIC_NUMBER, body, "{\"type\":\"number\"}");
        let body = self.base.tag_dispatch(false, &[NAMESPACE.to_owned()]);
        self.set_basic_rule(BASIC_STRING, body, "{\"type\":\"string\"}");
        let body = self.base.generate_boolean();
        self.set_basic_rule(BASIC_BOOLEAN, body, "{\"type\":\"boolean\"}");
        let body = self.base.generate_null();
        self.set_basic_rule(BASIC_NULL, body, "{\"type\":\"null\"}");
        Ok(())
    }

    fn next_separator(
        &mut self,
        _is_end: bool,
    ) -> String {
        self.base.whitespace_pattern()
    }

    fn generate_string(
        &mut self,
        spec: &StringSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let has_known_format = spec.format.as_deref().and_then(json_format_to_regex_pattern).is_some();
        if spec.pattern.is_some() || has_known_format || spec.min_length != 0 || spec.max_length != -1 {
            return Err(SchemaError::invalid(
                "String pattern, recognized format, and length constraints are not supported by minimax_m3_xml \
                 because they cannot be combined with the namespace-marker exclusion",
            ));
        }
        Ok(self.base.rule_ref_by_name(BASIC_STRING))
    }

    fn generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let mut prefix_items = Vec::with_capacity(spec.prefix_items.len());
        for (index, item) in spec.prefix_items.iter().enumerate() {
            let item_rule_id = self.create_rule(item, &format!("{rule_name}_item_{index}"))?;
            prefix_items.push(self.format_element(ARRAY_ITEM_NAME, item_rule_id)?);
        }
        let additional_item = if spec.allow_additional_items {
            let additional = spec
                .additional_items
                .as_ref()
                .ok_or_else(|| SchemaError::invalid("minimax_m3_xml requires a fixed schema for array items"))?;
            let item_rule_id = self.create_rule(additional, &format!("{rule_name}_additional"))?;
            Some(self.format_element(ARRAY_ITEM_NAME, item_rule_id)?)
        } else {
            None
        };

        let base = &mut self.base;
        let empty = base.empty();
        let whitespace = base.whitespace_expression();
        // The parser bounds item counts to int32.
        let min_items = spec.min_items as i32;
        let max_items = spec.max_items as i32;
        let Some(first_item) = prefix_items.first().copied() else {
            let Some(additional_item) = additional_item.filter(|_| max_items != 0) else {
                return Ok(empty);
            };
            let separated = base.sequence(&[whitespace, additional_item]);
            let rest = base.repeat(
                &format!("{rule_name}_items"),
                separated,
                (min_items - 1).max(0),
                if max_items == -1 {
                    -1
                } else {
                    (max_items - 1).max(0)
                },
            );
            let nonempty = base.sequence(&[whitespace, additional_item, rest, whitespace]);
            return Ok(if min_items == 0 {
                base.choice(&[nonempty, empty])
            } else {
                nonempty
            });
        };

        let prefix_count = prefix_items.len() as i32;
        let mut tail = empty;
        if let Some(additional_item) = additional_item {
            let separated = base.sequence(&[whitespace, additional_item]);
            tail = base.repeat(
                &format!("{rule_name}_additional_items"),
                separated,
                (min_items - prefix_count).max(0),
                if max_items == -1 {
                    -1
                } else {
                    (max_items - prefix_count).max(0)
                },
            );
        }
        // A prefix item constrains its position without making it mandatory: the chain can stop
        // once minItems is satisfied.
        for index in (0..prefix_count - 1).rev() {
            let emitted_count = index + 1;
            let can_stop = emitted_count >= min_items;
            let can_continue = max_items == -1 || emitted_count < max_items;
            let mut body = empty;
            if can_continue {
                let continuation = base.sequence(&[whitespace, prefix_items[index as usize + 1], tail]);
                body = if can_stop {
                    base.choice(&[continuation, empty])
                } else {
                    continuation
                };
            }
            let tail_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_prefix_tail_{index}"), body);
            tail = base.rule_ref(tail_rule_id);
        }
        if max_items == 0 {
            return Ok(empty);
        }
        let nonempty = base.sequence(&[whitespace, first_item, tail, whitespace]);
        Ok(if min_items == 0 {
            base.choice(&[nonempty, empty])
        } else {
            nonempty
        })
    }

    fn generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        validate_object(spec)?;
        let saved_any_whitespace = std::mem::replace(&mut self.base.any_whitespace, false);
        let result = self.json_generate_object(spec, rule_name, false);
        self.base.any_whitespace = saved_any_whitespace;
        result
    }

    fn generate_any(
        &mut self,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        Err(SchemaError::invalid("minimax_m3_xml does not support unconstrained schemas"))
    }

    fn generate_const(
        &mut self,
        spec: &ConstSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.generate_literal(&parse_literal(&spec.json_value))
    }

    fn generate_enum(
        &mut self,
        spec: &EnumSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let mut values = Vec::with_capacity(spec.json_values.len());
        for json_value in &spec.json_values {
            values.push(self.generate_literal(&parse_literal(json_value))?);
        }
        Ok(self.base.choice(&values))
    }

    fn format_property(
        &mut self,
        key: &str,
        value_rule_id: i32,
        _rule_name: &str,
        _index: usize,
        _schema: &SchemaSpecPtr,
    ) -> Result<i32, SchemaError> {
        self.format_element(key, value_rule_id)
    }
}

/// Rejects objects whose element names are not all known when the grammar is built.
fn validate_object(spec: &ObjectSpec) -> Result<(), SchemaError> {
    if spec.allow_additional_properties
        || spec.additional_properties_schema.is_some()
        || spec.allow_unevaluated_properties
        || spec.unevaluated_properties_schema.is_some()
        || !spec.pattern_properties.is_empty()
        || spec.property_names.is_some()
    {
        return Err(SchemaError::invalid(
            "minimax_m3_xml requires fixed object property names; additionalProperties, unevaluatedProperties, \
             patternProperties, and propertyNames are not supported",
        ));
    }
    let mut property_names = HashSet::with_capacity(spec.properties.len());
    for property in &spec.properties {
        validate_element_name(&property.name)?;
        property_names.insert(property.name.as_str());
    }
    if let Some(required) = spec.required.iter().find(|required| !property_names.contains(required.as_str())) {
        return Err(SchemaError::invalid(format!("minimax_m3_xml required property has no fixed schema: {required}")));
    }
    Ok(())
}

/// Rejects element names that cannot be parsed back from the tags.
fn validate_element_name(name: &str) -> Result<(), SchemaError> {
    if name.is_empty() || name.starts_with('/') || name.contains('>') {
        return Err(SchemaError::invalid(format!("Invalid minimax_m3_xml element name: {name}")));
    }
    if name.bytes().all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c | 0x0b)) {
        return Err(SchemaError::invalid("minimax_m3_xml element names cannot be blank"));
    }
    Ok(())
}
