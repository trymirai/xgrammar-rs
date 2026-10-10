//! The JSON-schema generation algorithm, expressed over overridable hooks.

use super::{
    indent_manager::IndentManager,
    json_schema_converter::{
        BASIC_ANY, BASIC_ARRAY, BASIC_BOOLEAN, BASIC_ESCAPE, BASIC_INTEGER, BASIC_NULL, BASIC_NUMBER, BASIC_OBJECT,
        BASIC_STRING, BASIC_STRING_SUB, JSON_CACHE_CONTEXT, JsonSchemaConverter, ascii_class,
        json_format_to_regex_pattern,
    },
    schema_error::SchemaError,
    schema_spec::{
        AllOfSpec, AnyOfSpec, ArraySpec, ConstSpec, EnumSpec, IntegerSpec, NumberSpec, ObjectSpec, Property, RefSpec,
        SchemaSpec, SchemaSpecPtr, SchemaSpecVariant, StringSpec, TypeArraySpec,
    },
};
use crate::grammar::Grammar;

/// A JSON-schema converter. The provided methods generate the JSON encoding; a converter for
/// another encoding overrides the hooks (the methods without a `json_` prefix) and calls the
/// `json_*` implementations for the parts that stay JSON. The `json_*` methods and the
/// non-hook methods are the shared algorithm and are not overridden.
pub trait SchemaConverter {
    /// The shared conversion state.
    fn base(&self) -> &JsonSchemaConverter;

    /// The shared conversion state, mutably.
    fn base_mut(&mut self) -> &mut JsonSchemaConverter;

    /* -------------------------------- hooks -------------------------------- */

    /// Allocates and fills the format's basic rules.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a basic rule cannot be built.
    fn add_basic_rules(&mut self) -> Result<(), SchemaError> {
        self.json_add_basic_rules(&[])
    }

    /// Records the rule generated for a schema cache key.
    fn add_cache(
        &mut self,
        key: &str,
        rule_id: i32,
    ) {
        if !key.is_empty() {
            self.base_mut().rule_cache.insert((key.to_owned(), JSON_CACHE_CONTEXT), rule_id);
        }
    }

    /// The rule generated for a schema cache key, if any.
    fn get_cache(
        &self,
        key: &str,
    ) -> Option<i32> {
        if key.is_empty() {
            return None;
        }
        self.base().rule_cache.get(&(key.to_owned(), JSON_CACHE_CONTEXT)).copied()
    }

    /// The key under which a `$ref` rule is shared; formats key it by their encoding context.
    fn ref_cache_key(
        &self,
        uri: &str,
    ) -> String {
        uri.to_owned()
    }

    /// The next separator fragment between object members.
    fn next_separator(
        &mut self,
        is_end: bool,
    ) -> String {
        self.base_mut().indent_manager.next_separator(is_end)
    }

    /// The rule matching a free-form property key.
    fn key_pattern(&self) -> &'static str {
        BASIC_STRING
    }

    /// The rule matching any value.
    fn basic_any_rule_name(&self) -> &'static str {
        BASIC_ANY
    }

    /// A free-form property key that is none of the declared `properties`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the exclusions cannot be applied to keys.
    fn key_pattern_excluding(
        &mut self,
        properties: &[Property],
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.json_key_pattern_excluding(properties, rule_name)
    }

    /// `string`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a pattern or format cannot be converted.
    fn generate_string(
        &mut self,
        spec: &StringSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.json_generate_string(spec, rule_name)
    }

    /// `array`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if an item schema cannot be converted.
    fn generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.json_generate_array(spec, rule_name)
    }

    /// `object`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a property schema cannot be converted.
    fn generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        self.json_generate_object(spec, rule_name, true)
    }

    /// Any value (`{}` or `true`).
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the format cannot encode unconstrained values.
    fn generate_any(
        &mut self,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        Ok(self.json_generate_any())
    }

    /// `const`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the format cannot encode the literal.
    fn generate_const(
        &mut self,
        spec: &ConstSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        Ok(self.json_generate_literals(std::slice::from_ref(&spec.json_value)))
    }

    /// `enum`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the format cannot encode a literal.
    fn generate_enum(
        &mut self,
        spec: &EnumSpec,
        _rule_name: &str,
    ) -> Result<i32, SchemaError> {
        Ok(self.json_generate_literals(&spec.json_values))
    }

    /// The fixed spelling of a declared property key. `schema` is the property's value schema.
    fn format_property_key(
        &mut self,
        key: &str,
        _schema: &SchemaSpecPtr,
    ) -> i32 {
        self.base_mut().json_property_key(key)
    }

    /// A declared property: its key, the separator, and its value rule. `index` is the
    /// property's position among the object's declared properties.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the value schema cannot be converted.
    fn format_property(
        &mut self,
        key: &str,
        value_rule_id: i32,
        _rule_name: &str,
        _index: usize,
        schema: &SchemaSpecPtr,
    ) -> Result<i32, SchemaError> {
        let key = self.format_property_key(key, schema);
        Ok(self.base_mut().json_member(key, value_rule_id))
    }

    /// A free-form (additional, pattern, or named-by-`propertyNames`) property. `schema` is
    /// the value schema, or `None` when the free-form key has no single schema.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the value schema cannot be converted.
    fn format_other_property(
        &mut self,
        key_pattern: i32,
        value_rule_id: i32,
        _rule_name: &str,
        _rule_name_suffix: &str,
        _schema: Option<&SchemaSpecPtr>,
    ) -> Result<i32, SchemaError> {
        Ok(self.base_mut().json_member(key_pattern, value_rule_id))
    }

    /* ---------------------------- the algorithm ---------------------------- */

    /// Converts the parsed schema into the grammar, consuming the converter.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if any part of the schema cannot be converted.
    fn into_grammar(
        mut self,
        spec: &SchemaSpecPtr,
    ) -> Result<Grammar, SchemaError>
    where
        Self: Sized,
    {
        self.add_basic_rules()?;
        // Register the root rule so that `$ref: "#"` resolves to it.
        let root_rule_id = self.base_mut().builder.add_empty_rule_with_hint("root");
        let root_rule_name = self.base().builder.get_rule(root_rule_id).name.clone();
        let root_key = self.ref_cache_key("#");
        self.base_mut().uri_to_rule_id.insert(root_key, root_rule_id);
        let body = if let Some(cached) = self.get_cache(&spec.cache_key) {
            self.base_mut().rule_ref(cached)
        } else {
            self.add_cache(&spec.cache_key, root_rule_id);
            self.generate_from_spec(spec, &root_rule_name)?
        };
        let base = self.base_mut();
        base.builder.update_rule_body(root_rule_id, body);
        Ok(std::mem::take(&mut base.builder).into_grammar_with_root_id(root_rule_id))
    }

    /// The rule for `spec`: a cached rule, or a new rule named after `rule_name_hint`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the schema cannot be converted.
    fn create_rule(
        &mut self,
        spec: &SchemaSpecPtr,
        rule_name_hint: &str,
    ) -> Result<i32, SchemaError> {
        if let Some(cached) = self.get_cache(&spec.cache_key) {
            return Ok(cached);
        }
        let rule_id = self.base_mut().builder.add_empty_rule_with_hint(rule_name_hint);
        let rule_name = self.base().builder.get_rule(rule_id).name.clone();
        let body = self.generate_from_spec(spec, &rule_name)?;
        self.base_mut().builder.update_rule_body(rule_id, body);
        Ok(rule_id)
    }

    /// Dispatches to the generator of the spec's kind.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the schema cannot be converted.
    fn generate_from_spec(
        &mut self,
        spec: &SchemaSpecPtr,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        match &spec.spec {
            SchemaSpecVariant::Integer(spec) => self.base_mut().generate_integer(spec, rule_name),
            SchemaSpecVariant::Number(spec) => self.base_mut().generate_number(spec, rule_name),
            SchemaSpecVariant::String(spec) => self.generate_string(spec, rule_name),
            SchemaSpecVariant::Boolean => Ok(self.base_mut().generate_boolean()),
            SchemaSpecVariant::Null => Ok(self.base_mut().generate_null()),
            SchemaSpecVariant::Array(spec) => self.generate_array(spec, rule_name),
            SchemaSpecVariant::Object(spec) => self.generate_object(spec, rule_name),
            SchemaSpecVariant::Any => self.generate_any(rule_name),
            SchemaSpecVariant::Const(spec) => self.generate_const(spec, rule_name),
            SchemaSpecVariant::Enum(spec) => self.generate_enum(spec, rule_name),
            SchemaSpecVariant::Ref(spec) => self.generate_ref(spec),
            SchemaSpecVariant::AnyOf(AnyOfSpec {
                options,
            })
            | SchemaSpecVariant::TypeArray(TypeArraySpec {
                type_schemas: options,
            }) => {
                let case = if matches!(spec.spec, SchemaSpecVariant::AnyOf(_)) {
                    "case"
                } else {
                    "type"
                };
                let mut choices = Vec::with_capacity(options.len());
                for (index, option) in options.iter().enumerate() {
                    let rule_id = self.create_rule(option, &format!("{rule_name}_{case}_{index}"))?;
                    choices.push(self.base_mut().rule_ref(rule_id));
                }
                Ok(self.base_mut().choice(&choices))
            },
            SchemaSpecVariant::AllOf(AllOfSpec {
                schemas,
            }) => {
                if let [schema] = schemas.as_slice() {
                    return self.generate_from_spec(schema, &format!("{rule_name}_case_0"));
                }
                eprintln!("Support for allOf with multiple options is still ongoing");
                self.generate_any(rule_name)
            },
        }
    }

    /// `$ref`: one rule per reference key, allocated before resolving so that recursive
    /// references point back at it.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the reference cannot be resolved or converted.
    fn generate_ref(
        &mut self,
        spec: &RefSpec,
    ) -> Result<i32, SchemaError> {
        let cache_key = self.ref_cache_key(&spec.uri);
        if let Some(&rule_id) = self.base().uri_to_rule_id.get(&cache_key) {
            return Ok(self.base_mut().rule_ref(rule_id));
        }
        // Name the rule after the URI path so that the same $ref always gets the same name.
        let mut rule_name_hint = "ref".to_owned();
        if let Some(path) = spec.uri.strip_prefix("#/") {
            let prefix = path
                .split('/')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    part.chars().filter(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '-' | '.')).collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("_");
            if !prefix.is_empty() {
                rule_name_hint = prefix;
            }
        }
        let base = self.base_mut();
        let rule_id = base.builder.add_empty_rule_with_hint(&rule_name_hint);
        let rule_name = base.builder.get_rule(rule_id).name.clone();
        base.uri_to_rule_id.insert(cache_key, rule_id);
        let resolved = base.resolve_ref(&spec.uri)?;
        let body = self.generate_from_spec(&resolved, &rule_name)?;
        self.base_mut().builder.update_rule_body(rule_id, body);
        self.add_cache(&resolved.cache_key, rule_id);
        Ok(self.base_mut().rule_ref(rule_id))
    }

    /// A reference to the free-form key rule.
    fn key_pattern_expression(&mut self) -> i32 {
        let key_pattern = self.key_pattern();
        self.base_mut().rule_ref_by_name(key_pattern)
    }

    /// The next separator between object members, as an expression.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the separator cannot be encoded.
    fn next_separator_expression(
        &mut self,
        is_end: bool,
    ) -> Result<i32, SchemaError> {
        let separator = self.next_separator(is_end);
        self.base_mut().formatting_expression(&separator)
    }

    /// The JSON basic rules (`basic_escape` … `basic_object`), plus empty rules for
    /// `additional_rule_names` allocated right after them.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a basic rule cannot be built.
    fn json_add_basic_rules(
        &mut self,
        additional_rule_names: &[&str],
    ) -> Result<(), SchemaError> {
        let base = self.base_mut();
        for name in [
            BASIC_ESCAPE,
            BASIC_STRING_SUB,
            BASIC_ANY,
            BASIC_INTEGER,
            BASIC_NUMBER,
            BASIC_STRING,
            BASIC_BOOLEAN,
            BASIC_NULL,
            BASIC_ARRAY,
            BASIC_OBJECT,
        ]
        .iter()
        .chain(additional_rule_names)
        {
            base.builder.add_empty_rule(*name);
        }
        base.add_helper_rules()?;

        // The basic rules are compact regardless of the requested indentation.
        let any_whitespace = base.any_whitespace;
        let compact = IndentManager::new(
            None,
            if any_whitespace {
                ","
            } else {
                ", "
            },
            any_whitespace,
            base.max_whitespace_cnt.filter(|_| any_whitespace),
        );
        let saved_indent_manager = std::mem::replace(&mut base.indent_manager, compact);

        let any_spec = SchemaSpec::make(SchemaSpecVariant::Any, "{}");
        let body = self.generate_any(BASIC_ANY)?;
        self.set_basic_rule(BASIC_ANY, body, "{}");
        let body = self.base_mut().generate_integer(&IntegerSpec::default(), BASIC_INTEGER)?;
        self.set_basic_rule(BASIC_INTEGER, body, "{\"type\":\"integer\"}");
        let body = self.base_mut().generate_number(&NumberSpec::default(), BASIC_NUMBER)?;
        self.set_basic_rule(BASIC_NUMBER, body, "{\"type\":\"number\"}");
        let base = self.base_mut();
        let quote = base.byte_string(b"\"");
        let string_sub = base.rule_ref_by_name(BASIC_STRING_SUB);
        let body = base.sequence(&[quote, string_sub]);
        self.set_basic_rule(BASIC_STRING, body, "{\"type\":\"string\"}");
        let body = self.base_mut().generate_boolean();
        self.set_basic_rule(BASIC_BOOLEAN, body, "{\"type\":\"boolean\"}");
        let body = self.base_mut().generate_null();
        self.set_basic_rule(BASIC_NULL, body, "{\"type\":\"null\"}");
        let array_spec = ArraySpec {
            allow_additional_items: true,
            additional_items: Some(any_spec.clone()),
            ..ArraySpec::default()
        };
        let body = self.generate_array(&array_spec, BASIC_ARRAY)?;
        self.set_basic_rule(BASIC_ARRAY, body, "{\"type\":\"array\"}");
        let object_spec = ObjectSpec {
            allow_additional_properties: true,
            additional_properties_schema: Some(any_spec),
            ..ObjectSpec::default()
        };
        let body = self.generate_object(&object_spec, BASIC_OBJECT)?;
        self.set_basic_rule(BASIC_OBJECT, body, "{\"type\":\"object\"}");

        self.base_mut().indent_manager = saved_indent_manager;
        Ok(())
    }

    /// Sets the body of the basic rule `name` and caches it under `cache_key`.
    fn set_basic_rule(
        &mut self,
        name: &str,
        body: i32,
        cache_key: &str,
    ) {
        let builder = &mut self.base_mut().builder;
        builder.update_rule_body_by_name(name, body);
        let rule_id = builder.get_rule_id(name);
        self.add_cache(cache_key, rule_id);
    }

    /// The JSON `string`: a format or pattern regex, a length bound, or any string.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a pattern or format cannot be converted.
    fn json_generate_string(
        &mut self,
        spec: &StringSpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let base = self.base_mut();
        let quote = base.byte_string(b"\"");
        if let Some(regex) = spec.format.as_deref().and_then(json_format_to_regex_pattern) {
            // The format regexes use constructs that the automaton engine does not fully support
            // yet (e.g. quoted email local parts), so they keep the CFG expansion.
            let body = base.regex_expression(&regex, false, true)?;
            return Ok(base.sequence(&[quote, body, quote]));
        }
        if let Some(pattern) = &spec.pattern {
            let body = base.regex_expression(pattern, true, false)?;
            return Ok(base.sequence(&[quote, body, quote]));
        }
        // Length bounds are dropped together with exclusions: intersecting the unrolled bound with
        // the exclusion automaton would emit one rule per position and automaton state.
        if spec.min_length != 0 || spec.max_length != -1 {
            if base.excludes.is_empty() {
                let character = base.builder.add_character_class(&ascii_class(b"\"\\\r\n"), true);
                let body = base.repeat(&format!("{rule_name}_characters"), character, spec.min_length, spec.max_length);
                return Ok(base.sequence(&[quote, body, quote]));
            }
            JsonSchemaConverter::warn_dropped_length_constraints(spec, rule_name);
        }
        let string_sub = base.rule_ref_by_name(BASIC_STRING_SUB);
        Ok(base.sequence(&[quote, string_sub]))
    }

    /// The JSON `array`. `prefixItems` entries are positional: the array may end after any
    /// prefix position (subject to `minItems`), and additional items only follow the full prefix.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if an item schema cannot be converted.
    fn json_generate_array(
        &mut self,
        spec: &ArraySpec,
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        let base = self.base_mut();
        base.indent_manager.start_indent();
        let start_separator = base.indent_manager.start_separator();
        let start_separator = base.formatting_expression(&start_separator)?;
        let middle_separator = base.indent_manager.middle_separator();
        let middle_separator = base.formatting_expression(&middle_separator)?;
        let end_separator = base.indent_manager.end_separator();
        let end_separator = base.formatting_expression(&end_separator)?;
        let empty_separator = base.indent_manager.empty_separator();
        let empty_separator = base.formatting_expression(&empty_separator)?;

        let mut item_rule_ids = Vec::with_capacity(spec.prefix_items.len());
        for (index, item) in spec.prefix_items.iter().enumerate() {
            item_rule_ids.push(self.create_rule(item, &format!("{rule_name}_item_{index}"))?);
        }
        let additional_rule_id = match (&spec.additional_items, spec.allow_additional_items) {
            (Some(additional), true) => Some(self.create_rule(additional, &format!("{rule_name}_additional"))?),
            _ => None,
        };
        let base = self.base_mut();
        base.indent_manager.end_indent();

        let left_bracket = base.byte_string(b"[");
        let right_bracket = base.byte_string(b"]");
        let empty_array = base.sequence(&[left_bracket, empty_separator, right_bracket]);
        let max_items = |offset: i64| {
            if spec.max_items == -1 {
                -1
            } else {
                (spec.max_items - offset) as i32
            }
        };

        if item_rule_ids.is_empty() {
            let Some(additional_rule_id) = additional_rule_id.filter(|_| spec.max_items != 0) else {
                return Ok(empty_array);
            };
            let additional = base.rule_ref(additional_rule_id);
            let separated = base.sequence(&[middle_separator, additional]);
            let tail =
                base.repeat(&format!("{rule_name}_items"), separated, (spec.min_items - 1).max(0) as i32, max_items(1));
            let nonempty =
                base.sequence(&[left_bracket, start_separator, additional, tail, end_separator, right_bracket]);
            return Ok(if spec.min_items == 0 {
                base.choice(&[nonempty, empty_array])
            } else {
                nonempty
            });
        }

        let item_count = item_rule_ids.len();
        let mandatory_count = (spec.min_items.max(0) as usize).min(item_count);
        // The mandatory head: the first min(minItems, n) items, separated.
        let mut content = Vec::with_capacity(2 * mandatory_count);
        for (index, &item_rule_id) in item_rule_ids[..mandatory_count].iter().enumerate() {
            if index != 0 {
                content.push(middle_separator);
            }
            content.push(base.rule_ref(item_rule_id));
        }
        // The rest is a right-recursive chain `suffix_k ::= "" | sep item_k suffix_{k+1}` ending
        // with the additional-items tail, so each position is encoded once.
        let mut suffix = base.empty();
        if let Some(additional_rule_id) = additional_rule_id {
            let additional = base.rule_ref(additional_rule_id);
            let separated = base.sequence(&[middle_separator, additional]);
            let minimum_additional = (spec.min_items - item_count as i64).max(0) as i32;
            suffix = base.repeat(
                &format!("{rule_name}_additional_items"),
                separated,
                minimum_additional,
                max_items(item_count as i64),
            );
        }
        for k in (mandatory_count.max(1)..item_count).rev() {
            let item = base.rule_ref(item_rule_ids[k]);
            let with_item = base.sequence(&[middle_separator, item, suffix]);
            let empty = base.empty();
            let body = base.choice(&[empty, with_item]);
            let suffix_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_suffix_{k}"), body);
            suffix = base.rule_ref(suffix_rule_id);
        }
        if mandatory_count == 0 {
            let item = base.rule_ref(item_rule_ids[0]);
            let with_first = base.sequence(&[item, suffix]);
            let empty = base.empty();
            let body = base.choice(&[empty, with_first]);
            let suffix_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_suffix_0"), body);
            suffix = base.rule_ref(suffix_rule_id);
        }
        content.push(suffix);
        let items = base.sequence(&content);
        Ok(base.sequence(&[left_bracket, start_separator, items, end_separator, right_bracket]))
    }

    /// The JSON `object`; `need_braces` is false for formats that encode the members only.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a property schema cannot be converted.
    fn json_generate_object(
        &mut self,
        spec: &ObjectSpec,
        rule_name: &str,
        need_braces: bool,
    ) -> Result<i32, SchemaError> {
        let (additional_suffix, additional_property) = object_additional_property(spec);
        self.base_mut().indent_manager.start_indent();
        let mut has_content = false;
        let could_be_empty;
        let mut content = self.base_mut().empty();

        if !spec.properties.is_empty() && (!spec.pattern_properties.is_empty() || spec.property_names.is_some()) {
            // Named properties plus patternProperties/propertyNames, which become the
            // additional-property pattern.
            let mut effective_additional = additional_property.clone();
            let mut effective_suffix = additional_suffix;
            let mut additional_override = None;
            if !spec.pattern_properties.is_empty() {
                let mut patterns = Vec::with_capacity(spec.pattern_properties.len() + 1);
                for (index, pattern_property) in spec.pattern_properties.iter().enumerate() {
                    let pattern_suffix = format!("pp_{index}");
                    let key_rule_id = self.create_pattern_key_rule(
                        &pattern_property.pattern,
                        &format!("{rule_name}_{pattern_suffix}_key"),
                    )?;
                    let value_rule_id =
                        self.create_rule(&pattern_property.schema, &format!("{rule_name}_{pattern_suffix}"))?;
                    let key = self.base_mut().rule_ref(key_rule_id);
                    patterns.push(self.format_other_property(
                        key,
                        value_rule_id,
                        rule_name,
                        &pattern_suffix,
                        Some(&pattern_property.schema),
                    )?);
                }
                if let Some(additional) = &effective_additional {
                    let value_rule_id = self.create_rule(additional, &format!("{rule_name}_{effective_suffix}"))?;
                    let key = self.key_pattern_expression();
                    patterns.push(self.format_other_property(
                        key,
                        value_rule_id,
                        rule_name,
                        effective_suffix,
                        Some(additional),
                    )?);
                }
                additional_override = Some(self.base_mut().choice(&patterns));
                effective_additional.get_or_insert_with(|| SchemaSpec::make(SchemaSpecVariant::Any, ""));
                effective_suffix = "pp";
            } else if let (Some(property_names), Some(additional)) = (&spec.property_names, &effective_additional) {
                // propertyNames constrains the keys of additional properties; it only applies when
                // additional properties are allowed.
                let key_rule_id = self.create_rule(property_names, &format!("{rule_name}_name"))?;
                let value_rule_id = self.create_rule(additional, &format!("{rule_name}_{effective_suffix}"))?;
                let key = self.base_mut().rule_ref(key_rule_id);
                additional_override =
                    Some(self.format_other_property(key, value_rule_id, rule_name, "pn", Some(additional))?);
                effective_suffix = "pn";
            }
            content = self.partial_rule_for_properties(
                spec,
                effective_additional.as_ref(),
                rule_name,
                effective_suffix,
                additional_override,
            )?;
            has_content = spec.max_properties != 0;
            could_be_empty = spec.required.is_empty() && spec.min_properties == 0;
        } else if !spec.pattern_properties.is_empty() || spec.property_names.is_some() {
            // patternProperties or propertyNames without named properties.
            if spec.max_properties != 0 {
                let beginning_separator = self.next_separator_expression(false)?;
                let mut property_choices = Vec::new();
                if spec.pattern_properties.is_empty() {
                    let property_names = spec.property_names.as_ref().expect("checked above");
                    let key_rule_id = self.create_rule(property_names, &format!("{rule_name}_name"))?;
                    // propertyNames constrains only the key; a typed additionalProperties schema
                    // still applies to the value.
                    let value_rule_id = match &additional_property {
                        Some(additional) => {
                            self.create_rule(additional, &format!("{rule_name}_{additional_suffix}"))?
                        },
                        None => {
                            let any_rule_name = self.basic_any_rule_name();
                            self.base().builder.get_rule_id(any_rule_name)
                        },
                    };
                    let key = self.base_mut().rule_ref(key_rule_id);
                    let property =
                        self.format_other_property(key, value_rule_id, rule_name, "pn", additional_property.as_ref())?;
                    property_choices.push(self.base_mut().sequence(&[beginning_separator, property]));
                } else {
                    for (index, pattern_property) in spec.pattern_properties.iter().enumerate() {
                        let pattern_suffix = format!("prop_{index}");
                        let key_rule_id = self.create_pattern_key_rule(
                            &pattern_property.pattern,
                            &format!("{rule_name}_{pattern_suffix}_key"),
                        )?;
                        let value_rule_id =
                            self.create_rule(&pattern_property.schema, &format!("{rule_name}_{pattern_suffix}"))?;
                        let key = self.base_mut().rule_ref(key_rule_id);
                        let property = self.format_other_property(
                            key,
                            value_rule_id,
                            rule_name,
                            &pattern_suffix,
                            Some(&pattern_property.schema),
                        )?;
                        property_choices.push(self.base_mut().sequence(&[beginning_separator, property]));
                    }
                }
                let base = self.base_mut();
                let choices = base.choice(&property_choices);
                let property_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_prop"), choices);
                let separator = self.next_separator_expression(false)?;
                let base = self.base_mut();
                let property = base.rule_ref(property_rule_id);
                let subsequent_property = base.sequence(&[separator, property]);
                let rest = base.property_with_number_constraints(
                    subsequent_property,
                    spec.min_properties,
                    spec.max_properties,
                    1,
                    rule_name,
                );
                let end_separator = self.next_separator_expression(true)?;
                content = self.base_mut().sequence(&[property, rest, end_separator]);
                has_content = true;
                could_be_empty = spec.min_properties == 0;
            } else {
                could_be_empty = true;
            }
        } else if !spec.properties.is_empty() {
            content = self.partial_rule_for_properties(
                spec,
                additional_property.as_ref(),
                rule_name,
                additional_suffix,
                None,
            )?;
            has_content = spec.max_properties != 0;
            could_be_empty = spec.required.is_empty() && spec.min_properties == 0;
        } else if let Some(additional) = &additional_property {
            // No declared properties, additional properties allowed.
            if spec.max_properties != 0 {
                let value_rule_id = self.create_rule(additional, &format!("{rule_name}_{additional_suffix}"))?;
                let key = self.key_pattern_expression();
                let property =
                    self.format_other_property(key, value_rule_id, rule_name, additional_suffix, Some(additional))?;
                let first_separator = self.next_separator_expression(false)?;
                let separator = self.next_separator_expression(false)?;
                let base = self.base_mut();
                let subsequent_property = base.sequence(&[separator, property]);
                let rest = base.property_with_number_constraints(
                    subsequent_property,
                    spec.min_properties,
                    spec.max_properties,
                    1,
                    rule_name,
                );
                let end_separator = self.next_separator_expression(true)?;
                content = self.base_mut().sequence(&[first_separator, property, rest, end_separator]);
                has_content = true;
            }
            could_be_empty = spec.min_properties == 0;
        } else {
            // No properties of any kind: the object is unconditionally empty.
            could_be_empty = true;
        }

        let base = self.base_mut();
        base.indent_manager.end_indent();
        let left_brace = base.byte_string(b"{");
        let right_brace = base.byte_string(b"}");
        let result = if need_braces {
            base.sequence(&[left_brace, content, right_brace])
        } else {
            content
        };
        if !could_be_empty {
            return Ok(result);
        }
        let empty_content = if base.any_whitespace {
            base.whitespace_expression()
        } else {
            base.empty()
        };
        let empty_result = if need_braces {
            base.sequence(&[left_brace, empty_content, right_brace])
        } else {
            empty_content
        };
        Ok(if has_content {
            base.choice(&[result, empty_result])
        } else {
            empty_result
        })
    }

    /// The rule of a string key matching `pattern`. Built through [`Self::generate_string`] so
    /// that formats with unquoted keys produce their own key body.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the pattern cannot be converted.
    fn create_pattern_key_rule(
        &mut self,
        pattern: &str,
        rule_name_hint: &str,
    ) -> Result<i32, SchemaError> {
        let key_spec = StringSpec {
            pattern: Some(pattern.to_owned()),
            ..StringSpec::default()
        };
        self.create_rule(&SchemaSpec::make(SchemaSpecVariant::String(key_spec), ""), rule_name_hint)
    }

    /// The object members for the declared properties (in declaration order, or any order),
    /// followed by the additional properties (`additional_override`, or `key: additional`).
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if a property schema cannot be converted.
    fn partial_rule_for_properties(
        &mut self,
        spec: &ObjectSpec,
        additional: Option<&SchemaSpecPtr>,
        rule_name: &str,
        additional_suffix: &str,
        additional_override: Option<i32>,
    ) -> Result<i32, SchemaError> {
        let (properties, required) = (&spec.properties, &spec.required);
        let (min_properties, max_properties) = (spec.min_properties, spec.max_properties);
        if max_properties == 0 {
            return Ok(self.base_mut().empty());
        }
        let first_separator = self.next_separator_expression(false)?;
        let middle_separator = self.next_separator_expression(false)?;
        let last_separator = self.next_separator_expression(true)?;
        let mut property_patterns = Vec::with_capacity(properties.len());
        for (index, property) in properties.iter().enumerate() {
            let value_rule_id = self.create_rule(&property.schema, &format!("{rule_name}_prop_{index}"))?;
            property_patterns.push(self.format_property(
                &property.name,
                value_rule_id,
                rule_name,
                index,
                &property.schema,
            )?);
        }
        let mut pattern = additional_override;
        let mut additional_pattern = |converter: &mut Self| match (pattern, additional) {
            (Some(pattern), _) => Ok(pattern),
            (None, Some(additional)) => {
                let built = converter.json_additional_pattern(properties, additional, rule_name, additional_suffix)?;
                pattern = Some(built);
                Ok(built)
            },
            (None, None) => unreachable!("only requested when additional properties are allowed"),
        };

        if self.base().any_order {
            // One alternation over every key, repeated between max(minProperties, #required)
            // and maxProperties times: only the entry count is bounded, not which keys appear.
            let mut items = property_patterns;
            if additional.is_some() {
                items.push(additional_pattern(self)?);
            }
            let base = self.base_mut();
            let choices = base.choice(&items);
            let item_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_item"), choices);
            let item = base.rule_ref(item_rule_id);
            let separated = base.sequence(&[middle_separator, item]);
            let minimum_count = min_properties.max(required.len() as i32);
            let rest = base.property_with_number_constraints(separated, minimum_count, max_properties, 1, rule_name);
            return Ok(base.sequence(&[first_separator, item, rest, last_separator]));
        }

        let property_count = properties.len();
        let last = property_count - 1;
        let is_required = |index: usize| required.contains(&properties[index].name);
        let allow_additional = additional.is_some();
        let mut choices = Vec::new();

        if min_properties == 0 && max_properties == -1 {
            // No property count constraints: tails[i] follows property i.
            let mut tails = vec![self.base_mut().empty(); property_count];
            if allow_additional {
                let pattern = additional_pattern(self)?;
                let base = self.base_mut();
                let separated = base.sequence(&[middle_separator, pattern]);
                let repeated = base.repeat(&format!("{rule_name}_additional_properties"), separated, 0, -1);
                let tail_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_part_{last}"), repeated);
                tails[last] = base.rule_ref(tail_rule_id);
            }
            let base = self.base_mut();
            for index in (0..last).rev() {
                let with_property = base.sequence(&[middle_separator, property_patterns[index + 1], tails[index + 1]]);
                let body = if is_required(index + 1) {
                    with_property
                } else {
                    base.choice(&[tails[index + 1], with_property])
                };
                let tail_rule_id = base.builder.add_rule_with_hint(&format!("{rule_name}_part_{index}"), body);
                tails[index] = base.rule_ref(tail_rule_id);
            }
            for index in 0..property_count {
                choices.push(base.sequence(&[property_patterns[index], tails[index]]));
                if is_required(index) {
                    break;
                }
            }
            if allow_additional && required.is_empty() {
                let pattern = additional_pattern(self)?;
                choices.push(self.base_mut().sequence(&[pattern, tails[last]]));
            }
        } else {
            // matched_min/matched_max bound how many properties can have been emitted when
            // property i is the last one so far; tails[i][m - matched_min[i]] continues from there.
            let mut matched_min = vec![1; property_count];
            let mut found_required = is_required(0);
            for index in 1..property_count {
                matched_min[index] = if !found_required {
                    1
                } else {
                    matched_min[index - 1] + i32::from(is_required(index))
                };
                found_required |= is_required(index);
            }
            let mut matched_max: Vec<i32> = (1..=property_count as i32).collect();
            matched_min[last] = if allow_additional {
                matched_min[last].max(1)
            } else {
                matched_min[last].max(min_properties)
            };
            if max_properties != -1 {
                matched_max[last] = matched_max[last].min(max_properties);
            }
            for index in (0..last).rev() {
                matched_min[index] = matched_min[index].max(matched_min[index + 1] - 1);
                if max_properties != -1 {
                    let next_max = matched_max[index + 1] - i32::from(is_required(index + 1));
                    matched_max[index] = matched_max[index].min(next_max);
                }
            }
            let mut tails: Vec<Vec<i32>> = vec![Vec::new(); property_count];
            for matched in matched_min[last]..=matched_max[last] {
                if allow_additional {
                    let pattern = additional_pattern(self)?;
                    let base = self.base_mut();
                    let separated = base.sequence(&[middle_separator, pattern]);
                    let body = base.property_with_number_constraints(
                        separated,
                        min_properties,
                        max_properties,
                        matched,
                        rule_name,
                    );
                    let tail_rule_id =
                        base.builder.add_rule_with_hint(&format!("{rule_name}_part_{last}_{matched}"), body);
                    tails[last].push(base.rule_ref(tail_rule_id));
                } else {
                    tails[last].push(self.base_mut().empty());
                }
            }
            let base = self.base_mut();
            for index in (0..last).rev() {
                let next = &tails[index + 1];
                let next_min = matched_min[index + 1];
                let mut current = Vec::new();
                for matched in matched_min[index]..=matched_max[index] {
                    let body = if matched == matched_max[index + 1] {
                        next[(matched - next_min) as usize]
                    } else {
                        let with_property = base.sequence(&[
                            middle_separator,
                            property_patterns[index + 1],
                            next[(matched + 1 - next_min) as usize],
                        ]);
                        if is_required(index + 1) || matched == next_min - 1 {
                            with_property
                        } else {
                            base.choice(&[next[(matched - next_min) as usize], with_property])
                        }
                    };
                    let tail_rule_id =
                        base.builder.add_rule_with_hint(&format!("{rule_name}_part_{index}_{matched}"), body);
                    current.push(base.rule_ref(tail_rule_id));
                }
                tails[index] = current;
            }
            for index in 0..property_count {
                if max_properties != -1 && matched_max[index] < matched_min[index] {
                    continue;
                }
                if matched_min[index] > 1 {
                    break;
                }
                choices
                    .push(base.sequence(&[property_patterns[index], tails[index][(1 - matched_min[index]) as usize]]));
                if is_required(index) {
                    break;
                }
            }
            if allow_additional && required.is_empty() {
                let pattern = additional_pattern(self)?;
                let base = self.base_mut();
                let separated = base.sequence(&[middle_separator, pattern]);
                let rest =
                    base.property_with_number_constraints(separated, min_properties, max_properties, 1, rule_name);
                choices.push(base.sequence(&[pattern, rest]));
            }
        }
        let base = self.base_mut();
        let members = base.choice(&choices);
        Ok(base.sequence(&[first_separator, members, last_separator]))
    }

    /// The JSON key pattern excluding the declared property names.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the exclusions cannot be applied to keys.
    fn json_key_pattern_excluding(
        &mut self,
        properties: &[Property],
        rule_name: &str,
    ) -> Result<i32, SchemaError> {
        if properties.is_empty() {
            return Ok(self.key_pattern_expression());
        }
        let names: Vec<&str> = properties.iter().map(|property| property.name.as_str()).collect();
        self.base_mut().key_excluding_names(&names, rule_name)
    }

    /// The member pattern `key: value` of an object's additional properties, whose keys exclude
    /// the declared `properties`.
    ///
    /// # Errors
    /// Returns a [`SchemaError`] if the value schema cannot be converted.
    fn json_additional_pattern(
        &mut self,
        properties: &[Property],
        additional: &SchemaSpecPtr,
        rule_name: &str,
        suffix: &str,
    ) -> Result<i32, SchemaError> {
        let value_rule_id = self.create_rule(additional, &format!("{rule_name}_{suffix}"))?;
        let key = self.key_pattern_excluding(properties, rule_name)?;
        self.format_other_property(key, value_rule_id, rule_name, suffix, Some(additional))
    }

    /// Any JSON value.
    fn json_generate_any(&mut self) -> i32 {
        let base = self.base_mut();
        let choices = [BASIC_NUMBER, BASIC_STRING, BASIC_BOOLEAN, BASIC_NULL, BASIC_ARRAY, BASIC_OBJECT]
            .map(|name| base.rule_ref_by_name(name));
        base.choice(&choices)
    }

    /// The JSON literals that contain no exclusion, as alternatives.
    fn json_generate_literals(
        &mut self,
        json_values: &[String],
    ) -> i32 {
        let base = self.base_mut();
        let mut values = Vec::with_capacity(json_values.len());
        for value in json_values {
            if base.is_allowed_json_literal(value, false) {
                values.push(base.byte_string(value.as_bytes()));
            }
        }
        if values.is_empty() {
            return base.unsatisfiable();
        }
        base.choice(&values)
    }
}

/// The schema of the additional members of `spec` and its rule-name suffix: additional
/// properties, then unevaluated properties, then any value when either is merely allowed.
pub fn object_additional_property(spec: &ObjectSpec) -> (&'static str, Option<SchemaSpecPtr>) {
    if spec.allow_additional_properties && spec.additional_properties_schema.is_some() {
        ("addl", spec.additional_properties_schema.clone())
    } else if spec.allow_unevaluated_properties && spec.unevaluated_properties_schema.is_some() {
        ("uneval", spec.unevaluated_properties_schema.clone())
    } else if spec.allow_additional_properties || spec.allow_unevaluated_properties {
        ("addl", Some(SchemaSpec::make(SchemaSpecVariant::Any, "")))
    } else {
        ("", None)
    }
}
