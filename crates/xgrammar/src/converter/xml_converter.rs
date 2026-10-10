//! The XML tool-calling encoding shared by the XML parameter-list converters.

use super::{
    json_format::JsonFormat,
    json_schema_converter::{BASIC_ARRAY, BASIC_BOOLEAN, BASIC_NULL, BASIC_NUMBER, BASIC_OBJECT, ascii_class},
    schema_converter::SchemaConverter,
    schema_error::SchemaError,
    schema_spec::{ConstSpec, ObjectSpec, SchemaSpec, SchemaSpecPtr, SchemaSpecVariant},
    xml_tool_calling_converter::{XML_ANY, XML_OBJECT, XML_STRING, XML_VARIABLE_NAME, XmlToolCallingConverter},
    xml_wrapper::XmlWrapper,
};
use crate::grammar::CharacterClassElement;

/// A converter whose root object is an XML parameter list. Nesting level 0 is the parameter
/// list, level 1 a parameter's raw value, and deeper levels plain JSON. The provided methods
/// are the XML implementations of the [`SchemaConverter`] hooks, which an XML-family converter
/// can extend.
pub trait XmlConverter: SchemaConverter {
    /// The XML state.
    fn xml(&self) -> &XmlToolCallingConverter;

    /// The XML state, mutably.
    fn xml_mut(&mut self) -> &mut XmlToolCallingConverter;

    /// The JSON basic rules at level 2, then the XML rules: `xml_string` (raw text up to the
    /// parameter end), `xml_any`, `xml_object` (the parameter list), and `xml_variable_name`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a basic rule cannot be built.
    fn xml_add_basic_rules(&mut self) -> Result<(), SchemaError> {
        self.xml_mut().nested_object_level = 2;
        self.json_add_basic_rules(&[XML_STRING, XML_ANY, XML_OBJECT, XML_VARIABLE_NAME])?;
        let any_spec = SchemaSpec::make(SchemaSpecVariant::Any, "{}");

        self.xml_mut().nested_object_level = 1;
        // The unrestricted raw body is one TagDispatch; the parameter suffix is matched by the
        // enclosing property rule, outside this body's exclusion scope.
        let mut string_excludes = self.base().excludes.clone();
        string_excludes.push(self.xml().wrapper.parameter_suffix.to_owned());
        let body = self.base_mut().tag_dispatch(false, &string_excludes);
        self.set_basic_rule(XML_STRING, body, "{\"type\":\"string\"}");
        let body = self.generate_any(XML_ANY)?;
        self.set_basic_rule(XML_ANY, body, "{}");

        self.xml_mut().nested_object_level = 0;
        let object_spec = ObjectSpec {
            allow_additional_properties: true,
            additional_properties_schema: Some(any_spec),
            ..ObjectSpec::default()
        };
        let body = self.generate_object(&object_spec, XML_OBJECT)?;
        self.set_basic_rule(XML_OBJECT, body, "{\"type\":\"object\"}");

        let base = self.base_mut();
        let body = if base.excludes.is_empty() {
            let first = base.builder.add_character_class(&identifier_class(false), false);
            let rest = base.builder.add_character_class_star(&identifier_class(true), false);
            base.sequence(&[first, rest])
        } else {
            base.excluding_string("[a-zA-Z_][a-zA-Z0-9_]*", XML_VARIABLE_NAME, false, &[])?
        };
        base.builder.update_rule_body_by_name(XML_VARIABLE_NAME, body);
        Ok(())
    }

    /// The opening of a declared parameter (key prefix, escaped key, and key suffix).
    fn xml_format_property_key(
        &mut self,
        key: &str,
        schema: &SchemaSpecPtr,
    ) -> i32 {
        if self.xml().nested_object_level > 1 {
            return self.base_mut().json_property_key(key);
        }
        // Only kimi_k3_xml encodes the value's type next to the key.
        let pinned_type = if self.xml().format == JsonFormat::KimiK3Xml {
            XmlToolCallingConverter::rendered_json_type(Some(schema))
        } else {
            None
        };
        let escaped = self.xml().escape_attr_value(key);
        if !self.base().is_allowed_string(&escaped) {
            return self.base_mut().unsatisfiable();
        }
        let prefix = format!("{}{escaped}", self.xml().wrapper.key_prefix);
        let prefix = self.base_mut().byte_string(prefix.as_bytes());
        let suffix = self.xml_key_suffix(pinned_type);
        self.base_mut().sequence(&[prefix, suffix])
    }

    /// The expression between a parameter's key and its value. For Kimi-K3 it carries the type
    /// attribute: `pinned_type` for a declared property, every type for a free-form key.
    fn xml_key_suffix(
        &mut self,
        pinned_type: Option<&str>,
    ) -> i32 {
        let key_suffix = self.xml().wrapper.key_suffix;
        let is_kimi = self.xml().format == JsonFormat::KimiK3Xml;
        let base = self.base_mut();
        if !is_kimi {
            return base.byte_string(key_suffix.as_bytes());
        }
        let type_expr = match pinned_type {
            Some(pinned_type) => base.byte_string(pinned_type.as_bytes()),
            None => {
                let choices = XmlWrapper::KIMI_K3_TYPES.map(|value| base.byte_string(value.as_bytes()));
                base.choice(&choices)
            },
        };
        let prefix = base.byte_string(XmlWrapper::KIMI_K3_TYPE_PREFIX.as_bytes());
        let suffix = base.byte_string(XmlWrapper::KIMI_K3_TYPE_SUFFIX.as_bytes());
        base.sequence(&[prefix, type_expr, suffix])
    }

    /// A declared parameter at the parameter-list level, or a JSON member below it.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a DeepSeek value alternative cannot be converted.
    fn xml_format_property(
        &mut self,
        key: &str,
        value_rule_id: i32,
        schema: &SchemaSpecPtr,
    ) -> Result<i32, SchemaError> {
        if self.xml().nested_object_level > 1 {
            let key = self.base_mut().json_property_key(key);
            return Ok(self.base_mut().json_member(key, value_rule_id));
        }
        if self.xml().format.is_deepseek() {
            if !self.base().is_allowed_string(key) {
                return Ok(self.base_mut().unsatisfiable());
            }
            let prefix = format!("{}{key}", self.xml().wrapper.key_prefix);
            let prefix = self.base_mut().byte_string(prefix.as_bytes());
            let suffix = self.format_deepseek_param_suffix(Some(schema), Some(value_rule_id), "")?;
            return Ok(self.base_mut().sequence(&[prefix, suffix]));
        }
        let key = self.format_property_key(key, schema);
        Ok(self.xml_parameter_value(vec![key], value_rule_id))
    }

    /// A free-form parameter (`key_pattern` names it) at the parameter-list level, or a JSON
    /// member below it.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a DeepSeek value alternative cannot be converted.
    fn xml_format_other_property(
        &mut self,
        key_pattern: i32,
        value_rule_id: i32,
        schema: Option<&SchemaSpecPtr>,
    ) -> Result<i32, SchemaError> {
        if self.xml().nested_object_level > 1 {
            return Ok(self.base_mut().json_member(key_pattern, value_rule_id));
        }
        let key_prefix = self.xml().wrapper.key_prefix;
        let prefix = self.base_mut().byte_string(key_prefix.as_bytes());
        if self.xml().format.is_deepseek() {
            let suffix = self.format_deepseek_param_suffix(schema, Some(value_rule_id), "")?;
            return Ok(self.base_mut().sequence(&[prefix, key_pattern, suffix]));
        }
        let pinned_type = if self.xml().format == JsonFormat::KimiK3Xml {
            XmlToolCallingConverter::rendered_json_type(schema)
        } else {
            None
        };
        let suffix = self.xml_key_suffix(pinned_type);
        Ok(self.xml_parameter_value(vec![prefix, key_pattern, suffix], value_rule_id))
    }

    /// Appends the value wrapper, the value, and the parameter end to a parameter's opening.
    fn xml_parameter_value(
        &mut self,
        mut elements: Vec<i32>,
        value_rule_id: i32,
    ) -> i32 {
        let wrapper = self.xml().wrapper;
        let base = self.base_mut();
        if !wrapper.value_prefix.is_empty() {
            elements.push(base.whitespace_expression());
            elements.push(base.byte_string(wrapper.value_prefix.as_bytes()));
        }
        // xml_string already accepts whitespace; padding it would create one parser state for
        // every split of the leading and trailing whitespace.
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

    /// A DeepSeek parameter's `string` attribute, value, and closing tag: `string="true"` wraps
    /// raw strings, `string="false"` JSON values. Unions and mixed enums produce one alternative
    /// per option. Without `value_rule_id` the value rule is created when a typed leaf is reached.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a reference or option cannot be converted.
    fn format_deepseek_param_suffix(
        &mut self,
        schema: Option<&SchemaSpecPtr>,
        value_rule_id: Option<i32>,
        rule_name_hint: &str,
    ) -> Result<i32, SchemaError> {
        let value_rule_name = match value_rule_id {
            Some(rule_id) => self.base().builder.get_rule(rule_id).name.clone(),
            None => rule_name_hint.to_owned(),
        };
        if let Some(SchemaSpecVariant::Ref(reference)) = schema.map(|schema| &schema.spec) {
            if let Some(&rule_id) = self.xml().deepseek_param_ref_rules.get(&reference.uri) {
                return Ok(self.base_mut().rule_ref(rule_id));
            }
            // Register the rule before descending, so a recursive branch refers back to it and
            // shared acyclic subgraphs are built once.
            let param_rule_id =
                self.base_mut().builder.add_empty_rule_with_hint(&format!("{value_rule_name}_dsml_param"));
            self.xml_mut().deepseek_param_ref_rules.insert(reference.uri.clone(), param_rule_id);
            let resolved = self.base_mut().resolve_ref(&reference.uri)?;
            let body = self.format_deepseek_param_suffix(Some(&resolved), value_rule_id, &value_rule_name)?;
            let base = self.base_mut();
            base.builder.update_rule_body(param_rule_id, body);
            return Ok(base.rule_ref(param_rule_id));
        }

        // A schema rendered with a single type keeps the value rule built by the caller.
        if let (Some(pinned_type), Some(schema)) = (XmlToolCallingConverter::rendered_json_type(schema), schema) {
            let value_rule_id = match value_rule_id {
                Some(rule_id) => rule_id,
                None => self.create_rule(schema, &value_rule_name)?,
            };
            let value = self.base_mut().rule_ref(value_rule_id);
            return Ok(self.deepseek_value(value, pinned_type == "string"));
        }

        let options: Vec<SchemaSpecPtr> = match schema.map(|schema| &schema.spec) {
            Some(SchemaSpecVariant::AnyOf(spec)) => spec.options.clone(),
            Some(SchemaSpecVariant::TypeArray(spec)) => spec.type_schemas.clone(),
            Some(SchemaSpecVariant::AllOf(spec)) if spec.schemas.len() == 1 => spec.schemas.clone(),
            Some(SchemaSpecVariant::Enum(spec)) => spec
                .json_values
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
            _ => Vec::new(),
        };
        if options.is_empty() {
            // No schema, {}, and allOf with several schemas all render any value.
            let xml_string = self.base_mut().rule_ref_by_name(XML_STRING);
            let raw = self.deepseek_value(xml_string, true);
            let base = self.base_mut();
            let json_choices = [BASIC_NUMBER, BASIC_BOOLEAN, BASIC_NULL, BASIC_ARRAY, BASIC_OBJECT]
                .map(|name| base.rule_ref_by_name(name));
            let json = base.choice(&json_choices);
            let json = self.deepseek_value(json, false);
            return Ok(self.base_mut().choice(&[raw, json]));
        }
        let mut choices = Vec::with_capacity(options.len());
        for (index, option) in options.iter().enumerate() {
            // Only leaves need value rules; rebuilding a nested union's value subtree at each
            // level would create quadratically many rules.
            choices.push(self.format_deepseek_param_suffix(
                Some(option),
                None,
                &format!("{value_rule_name}_dsml_case_{index}"),
            )?);
        }
        Ok(self.base_mut().choice(&choices))
    }

    /// A DeepSeek value with its `string` attribute; JSON values may be padded by whitespace.
    fn deepseek_value(
        &mut self,
        value: i32,
        is_string: bool,
    ) -> i32 {
        let parameter_suffix = self.xml().wrapper.parameter_suffix;
        let base = self.base_mut();
        let attribute: &[u8] = if is_string {
            b"\" string=\"true\">"
        } else {
            b"\" string=\"false\">"
        };
        let mut elements = vec![base.byte_string(attribute)];
        if !is_string {
            elements.push(base.whitespace_expression());
        }
        elements.push(value);
        if !is_string {
            elements.push(base.whitespace_expression());
        }
        elements.push(base.byte_string(parameter_suffix.as_bytes()));
        base.sequence(&elements)
    }
}

/// `[a-zA-Z_]`, with `[0-9]` when `with_digits`.
fn identifier_class(with_digits: bool) -> Vec<CharacterClassElement> {
    let mut class = vec![CharacterClassElement::new(0x61, 0x7a), CharacterClassElement::new(0x41, 0x5a)];
    if with_digits {
        class.push(CharacterClassElement::new(0x30, 0x39));
    }
    class.extend(ascii_class(b"_"));
    class
}
