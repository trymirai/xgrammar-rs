//! Parses a JSON Schema (a [`serde_json::Value`]) into the [`SchemaSpec`] IR — a port of
//! `SchemaParser` in `cpp/json_schema_converter.cc`.

use std::collections::HashMap;

use serde_json::Value;

use super::{
    schema_error::SchemaError,
    schema_spec::{
        AllOfSpec, AnyOfSpec, ArraySpec, ConstSpec, EnumSpec, IntegerSpec,
        NumberSpec, ObjectSpec, PatternProperty, Property, RefSpec, SchemaSpec,
        SchemaSpecPtr, SchemaSpecVariant, StringSpec, TypeArraySpec,
    },
};

/// Object keys ignored when computing a schema's de-duplication cache key.
const SKIPPED_CACHE_KEYS: &[&str] = &[
    "title",
    "default",
    "description",
    "examples",
    "deprecated",
    "readOnly",
    "writeOnly",
    "$comment",
    "$schema",
];

/// Parses JSON schemas into [`SchemaSpec`] trees, resolving `$ref`s against the root.
pub(crate) struct SchemaParser {
    strict_mode: bool,
    root_schema: Value,
    ref_cache: HashMap<String, SchemaSpecPtr>,
    schema_cache: HashMap<String, SchemaSpecPtr>,
}

impl SchemaParser {
    pub fn new(
        root_schema: Value,
        strict_mode: bool,
    ) -> Self {
        Self {
            strict_mode,
            root_schema,
            ref_cache: HashMap::new(),
            schema_cache: HashMap::new(),
        }
    }

    /// Deterministic cache key for `value` (objects sorted, cosmetic keys dropped).
    fn compute_cache_key(value: &Value) -> String {
        match value {
            Value::Object(map) => {
                let mut kvs: Vec<(&String, &Value)> = map
                    .iter()
                    .filter(|(k, _)| !SKIPPED_CACHE_KEYS.contains(&k.as_str()))
                    .collect();
                kvs.sort_by(|a, b| a.0.cmp(b.0));
                let mut result = String::from("{");
                for (i, (k, v)) in kvs.iter().enumerate() {
                    if i != 0 {
                        result.push(',');
                    }
                    result.push('"');
                    result.push_str(k);
                    result.push_str("\":");
                    result.push_str(&Self::compute_cache_key(v));
                }
                result.push('}');
                result
            },
            Value::Array(arr) => {
                let mut result = String::from("[");
                for (i, item) in arr.iter().enumerate() {
                    if i != 0 {
                        result.push(',');
                    }
                    result.push_str(&Self::compute_cache_key(item));
                }
                result.push(']');
                result
            },
            other => other.to_string(),
        }
    }

    pub fn parse(
        &mut self,
        schema: &Value,
        default_type: Option<&str>,
    ) -> Result<SchemaSpecPtr, SchemaError> {
        let cache_key = Self::compute_cache_key(schema);
        if let Some(spec) = self.schema_cache.get(&cache_key) {
            return Ok(spec.clone());
        }

        if let Value::Bool(b) = schema {
            if !b {
                return Err(SchemaError::unsatisfiable(
                    "Schema 'false' cannot accept any value",
                ));
            }
            let spec =
                SchemaSpec::make(SchemaSpecVariant::Any, cache_key.clone());
            self.schema_cache.insert(cache_key, spec.clone());
            return Ok(spec);
        }

        let Some(obj) = schema.as_object() else {
            return Err(SchemaError::invalid(format!(
                "Schema should be an object or bool, but got {schema}"
            )));
        };

        let make = |variant: SchemaSpecVariant| {
            SchemaSpec::make(variant, cache_key.clone())
        };

        let result: SchemaSpecPtr = if obj.contains_key("$ref") {
            make(SchemaSpecVariant::Ref(Self::parse_ref(obj)?))
        } else if obj.contains_key("const") {
            make(SchemaSpecVariant::Const(Self::parse_const(obj)))
        } else if obj.contains_key("enum") {
            make(SchemaSpecVariant::Enum(Self::parse_enum(obj)?))
        } else if obj.contains_key("anyOf") || obj.contains_key("oneOf") {
            make(SchemaSpecVariant::AnyOf(self.parse_any_of(obj)?))
        } else if obj.contains_key("allOf") {
            make(SchemaSpecVariant::AllOf(self.parse_all_of(obj)?))
        } else if obj.contains_key("type") || default_type.is_some() {
            if obj.get("type").is_some_and(Value::is_array) {
                make(SchemaSpecVariant::TypeArray(self.parse_type_array(obj)?))
            } else {
                if obj.contains_key("type") && !obj["type"].is_string() {
                    return Err(SchemaError::invalid(
                        "Type should be a string",
                    ));
                }
                let ty = if obj.contains_key("type") {
                    obj["type"].as_str().unwrap()
                } else {
                    default_type.unwrap()
                };
                match ty {
                    "integer" => make(SchemaSpecVariant::Integer(
                        Self::parse_integer(obj)?,
                    )),
                    "number" => make(SchemaSpecVariant::Number(
                        Self::parse_number(obj)?,
                    )),
                    "string" => make(SchemaSpecVariant::String(
                        Self::parse_string(obj)?,
                    )),
                    "boolean" => make(SchemaSpecVariant::Boolean),
                    "null" => make(SchemaSpecVariant::Null),
                    "array" => {
                        make(SchemaSpecVariant::Array(self.parse_array(obj)?))
                    },
                    "object" => {
                        make(SchemaSpecVariant::Object(self.parse_object(obj)?))
                    },
                    other => {
                        return Err(SchemaError::invalid(format!(
                            "Unsupported type \"{other}\""
                        )));
                    },
                }
            }
        } else if obj.contains_key("properties")
            || obj.contains_key("additionalProperties")
            || obj.contains_key("unevaluatedProperties")
        {
            make(SchemaSpecVariant::Object(self.parse_object(obj)?))
        } else if obj.contains_key("items")
            || obj.contains_key("prefixItems")
            || obj.contains_key("unevaluatedItems")
        {
            make(SchemaSpecVariant::Array(self.parse_array(obj)?))
        } else {
            make(SchemaSpecVariant::Any)
        };

        self.schema_cache.insert(cache_key, result.clone());
        Ok(result)
    }

    fn check_integer_bound(value: &Value) -> Result<i64, SchemaError> {
        if let Some(i) = value.as_i64() {
            return Ok(i);
        }
        let Some(val) = value.as_f64() else {
            return Err(SchemaError::invalid("Value must be a number"));
        };
        if val != val.floor() {
            return Err(SchemaError::invalid(
                "Integer constraint must be a whole number",
            ));
        }
        if val > i64::MAX as f64 {
            return Err(SchemaError::invalid("Integer exceeds maximum limit"));
        }
        if val < i64::MIN as f64 {
            return Err(SchemaError::invalid("Integer exceeds minimum limit"));
        }
        Ok(val as i64)
    }

    fn parse_integer(
        schema: &serde_json::Map<String, Value>
    ) -> Result<IntegerSpec, SchemaError> {
        let mut spec = IntegerSpec::default();
        if let Some(v) = schema.get("minimum") {
            spec.minimum = Some(Self::check_integer_bound(v)?);
        }
        if let Some(v) = schema.get("maximum") {
            spec.maximum = Some(Self::check_integer_bound(v)?);
        }
        if let Some(v) = schema.get("exclusiveMinimum") {
            let val = Self::check_integer_bound(v)?;
            if val == i64::MAX {
                return Err(SchemaError::unsatisfiable(
                    "exclusiveMinimum would cause integer overflow",
                ));
            }
            spec.exclusive_minimum = Some(val);
        }
        if let Some(v) = schema.get("exclusiveMaximum") {
            let val = Self::check_integer_bound(v)?;
            if val == i64::MIN {
                return Err(SchemaError::unsatisfiable(
                    "exclusiveMaximum would cause integer underflow",
                ));
            }
            spec.exclusive_maximum = Some(val);
        }

        let mut effective_min = spec.minimum.unwrap_or(i64::MIN);
        let mut effective_max = spec.maximum.unwrap_or(i64::MAX);
        if let Some(e) = spec.exclusive_minimum {
            effective_min = effective_min.max(e + 1);
        }
        if let Some(e) = spec.exclusive_maximum {
            effective_max = effective_max.min(e - 1);
        }
        if effective_min > effective_max {
            return Err(SchemaError::unsatisfiable(
                "Invalid range: minimum greater than maximum",
            ));
        }
        Ok(spec)
    }

    fn parse_number(
        schema: &serde_json::Map<String, Value>
    ) -> Result<NumberSpec, SchemaError> {
        let get_double = |v: &Value| -> Result<f64, SchemaError> {
            v.as_f64()
                .ok_or_else(|| SchemaError::invalid("Value must be a number"))
        };
        let mut spec = NumberSpec::default();
        if let Some(v) = schema.get("minimum") {
            spec.minimum = Some(get_double(v)?);
        }
        if let Some(v) = schema.get("maximum") {
            spec.maximum = Some(get_double(v)?);
        }
        if let Some(v) = schema.get("exclusiveMinimum") {
            spec.exclusive_minimum = Some(get_double(v)?);
        }
        if let Some(v) = schema.get("exclusiveMaximum") {
            spec.exclusive_maximum = Some(get_double(v)?);
        }

        let mut effective_min = spec.minimum.unwrap_or(f64::NEG_INFINITY);
        let mut effective_max = spec.maximum.unwrap_or(f64::INFINITY);
        if let Some(e) = spec.exclusive_minimum {
            effective_min = effective_min.max(e);
        }
        if let Some(e) = spec.exclusive_maximum {
            effective_max = effective_max.min(e);
        }
        if effective_min > effective_max {
            return Err(SchemaError::unsatisfiable(
                "Invalid range: minimum greater than maximum",
            ));
        }
        Ok(spec)
    }

    fn parse_string(
        schema: &serde_json::Map<String, Value>
    ) -> Result<StringSpec, SchemaError> {
        let mut spec = StringSpec::default();
        if let Some(v) = schema.get("format") {
            spec.format = v.as_str().map(str::to_owned);
        }
        if let Some(v) = schema.get("pattern") {
            spec.pattern = v.as_str().map(str::to_owned);
        }
        if let Some(v) = schema.get("minLength") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "minLength must be an integer",
                ));
            };
            spec.min_length = n as i32;
        }
        if let Some(v) = schema.get("maxLength") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "maxLength must be an integer",
                ));
            };
            spec.max_length = n as i32;
        }
        if spec.max_length != -1 && spec.min_length > spec.max_length {
            return Err(SchemaError::unsatisfiable(format!(
                "minLength {} is greater than maxLength {}",
                spec.min_length, spec.max_length
            )));
        }
        Ok(spec)
    }

    fn parse_array(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<ArraySpec, SchemaError> {
        let mut spec = ArraySpec::default();

        if let Some(prefix) = schema.get("prefixItems") {
            let Some(arr) = prefix.as_array() else {
                return Err(SchemaError::invalid(
                    "prefixItems must be an array",
                ));
            };
            for item in arr {
                if item.as_bool() == Some(false) {
                    return Err(SchemaError::unsatisfiable(
                        "prefixItems contains false",
                    ));
                } else if !item.is_object() {
                    return Err(SchemaError::invalid(
                        "prefixItems must be an array of objects or booleans",
                    ));
                }
                spec.prefix_items.push(self.parse(item, None)?);
            }
        }

        if let Some(items) = schema.get("items") {
            if !items.is_boolean() && !items.is_object() {
                return Err(SchemaError::invalid(
                    "items must be a boolean or an object",
                ));
            }
            if items.as_bool() == Some(false) {
                spec.allow_additional_items = false;
            } else {
                spec.allow_additional_items = true;
                spec.additional_items = Some(self.parse(items, None)?);
            }
        } else if let Some(uneval) = schema.get("unevaluatedItems") {
            if !uneval.is_boolean() && !uneval.is_object() {
                return Err(SchemaError::invalid(
                    "unevaluatedItems must be a boolean or an object",
                ));
            }
            if uneval.as_bool() == Some(false) {
                spec.allow_additional_items = false;
            } else {
                spec.allow_additional_items = true;
                spec.additional_items = Some(self.parse(uneval, None)?);
            }
        } else if !self.strict_mode {
            spec.allow_additional_items = true;
            spec.additional_items =
                Some(SchemaSpec::make(SchemaSpecVariant::Any, ""));
        } else {
            spec.allow_additional_items = false;
        }

        if let Some(v) = schema.get("minItems") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "minItems must be an integer",
                ));
            };
            spec.min_items = n.max(0);
        }
        if let Some(v) = schema.get("minContains") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "minContains must be an integer",
                ));
            };
            spec.min_items = spec.min_items.max(n);
        }
        if let Some(v) = schema.get("maxItems") {
            let n = v.as_i64().filter(|n| *n >= 0);
            let Some(n) = n else {
                return Err(SchemaError::invalid(
                    "maxItems must be a non-negative integer",
                ));
            };
            spec.max_items = n;
        }

        if spec.max_items != -1 && spec.min_items > spec.max_items {
            return Err(SchemaError::unsatisfiable(format!(
                "minItems is greater than maxItems: {} > {}",
                spec.min_items, spec.max_items
            )));
        }
        let prefix_size = spec.prefix_items.len() as i64;
        if spec.max_items != -1 && spec.max_items < prefix_size {
            return Err(SchemaError::unsatisfiable(format!(
                "maxItems is less than the number of prefixItems: {} < {}",
                spec.max_items, prefix_size
            )));
        }
        if !spec.allow_additional_items {
            if prefix_size < spec.min_items {
                return Err(SchemaError::unsatisfiable(format!(
                    "minItems is greater than the number of prefixItems, but additional items are \
                     not allowed: {} > {}",
                    spec.min_items, prefix_size
                )));
            }
            if spec.max_items != -1 && prefix_size > spec.max_items {
                return Err(SchemaError::unsatisfiable(format!(
                    "maxItems is less than the number of prefixItems, but additional items are not \
                     allowed: {} < {}",
                    spec.max_items, prefix_size
                )));
            }
        }
        Ok(spec)
    }

    fn parse_object(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<ObjectSpec, SchemaError> {
        let mut spec = ObjectSpec::default();

        if let Some(props) = schema.get("properties") {
            let Some(props) = props.as_object() else {
                return Err(SchemaError::invalid(
                    "properties must be an object",
                ));
            };
            for (key, value) in props {
                let schema = self.parse(value, None)?;
                spec.properties.push(Property {
                    name: key.clone(),
                    schema,
                });
            }
        }

        if let Some(req) = schema.get("required") {
            let Some(arr) = req.as_array() else {
                return Err(SchemaError::invalid("required must be an array"));
            };
            for r in arr {
                if let Some(s) = r.as_str() {
                    spec.required.insert(s.to_owned());
                }
            }
        }

        if let Some(pp) = schema.get("patternProperties") {
            let Some(pp) = pp.as_object() else {
                return Err(SchemaError::invalid(
                    "patternProperties must be an object",
                ));
            };
            for (key, value) in pp {
                let schema = self.parse(value, None)?;
                spec.pattern_properties.push(PatternProperty {
                    pattern: key.clone(),
                    schema,
                });
            }
        }

        if let Some(pn) = schema.get("propertyNames") {
            let Some(pn_obj) = pn.as_object() else {
                return Err(SchemaError::invalid(
                    "propertyNames must be an object",
                ));
            };
            if pn_obj
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t != "string")
            {
                return Err(SchemaError::unsatisfiable(
                    "propertyNames must be an object that validates string",
                ));
            }
            spec.property_names = Some(self.parse(pn, Some("string"))?);
        }

        spec.allow_additional_properties = !self.strict_mode;
        if let Some(add) = schema.get("additionalProperties") {
            if let Some(b) = add.as_bool() {
                spec.allow_additional_properties = b;
            } else {
                spec.allow_additional_properties = true;
                spec.additional_properties_schema =
                    Some(self.parse(add, None)?);
            }
        }

        spec.allow_unevaluated_properties = true;
        if schema.contains_key("additionalProperties") {
            spec.allow_unevaluated_properties =
                spec.allow_additional_properties;
        } else if let Some(uneval) = schema.get("unevaluatedProperties") {
            if let Some(b) = uneval.as_bool() {
                spec.allow_unevaluated_properties = b;
            } else {
                spec.allow_unevaluated_properties = true;
                spec.unevaluated_properties_schema =
                    Some(self.parse(uneval, None)?);
            }
        } else if self.strict_mode {
            spec.allow_unevaluated_properties = false;
        }

        if let Some(v) = schema.get("minProperties") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "minProperties must be an integer",
                ));
            };
            spec.min_properties = n as i32;
            if spec.min_properties < 0 {
                return Err(SchemaError::unsatisfiable(
                    "minProperties must be a non-negative integer",
                ));
            }
        }
        if let Some(v) = schema.get("maxProperties") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid(
                    "maxProperties must be an integer",
                ));
            };
            spec.max_properties = n as i32;
            if spec.max_properties < 0 {
                return Err(SchemaError::unsatisfiable(
                    "maxProperties must be a non-negative integer",
                ));
            }
        }

        if spec.max_properties != -1
            && spec.min_properties > spec.max_properties
        {
            return Err(SchemaError::unsatisfiable(format!(
                "minProperties is greater than maxProperties: {} > {}",
                spec.min_properties, spec.max_properties
            )));
        }
        if spec.max_properties != -1
            && spec.required.len() as i32 > spec.max_properties
        {
            return Err(SchemaError::unsatisfiable(format!(
                "maxProperties is less than the number of required properties: {} < {}",
                spec.max_properties,
                spec.required.len()
            )));
        }
        if spec.pattern_properties.is_empty()
            && spec.property_names.is_none()
            && !spec.allow_additional_properties
            && !spec.allow_unevaluated_properties
            && spec.min_properties > spec.properties.len() as i32
        {
            return Err(SchemaError::unsatisfiable(format!(
                "minProperties is greater than the number of properties, but additional properties \
                 aren't allowed: {} > {}",
                spec.min_properties,
                spec.properties.len()
            )));
        }
        Ok(spec)
    }

    fn parse_const(schema: &serde_json::Map<String, Value>) -> ConstSpec {
        ConstSpec {
            json_value: schema["const"].to_string(),
        }
    }

    fn parse_enum(
        schema: &serde_json::Map<String, Value>
    ) -> Result<EnumSpec, SchemaError> {
        let Some(arr) = schema["enum"].as_array() else {
            return Err(SchemaError::invalid("enum must be an array"));
        };
        if arr.is_empty() {
            return Err(SchemaError::invalid("enum array must not be empty"));
        }
        Ok(EnumSpec {
            json_values: arr.iter().map(Value::to_string).collect(),
        })
    }

    fn parse_ref(
        schema: &serde_json::Map<String, Value>
    ) -> Result<RefSpec, SchemaError> {
        let Some(uri) = schema["$ref"].as_str() else {
            return Err(SchemaError::invalid("$ref must be a string"));
        };
        Ok(RefSpec {
            uri: uri.to_owned(),
        })
    }

    pub fn resolve_ref(
        &mut self,
        uri: &str,
        _rule_name_hint: &str,
    ) -> Result<SchemaSpecPtr, SchemaError> {
        if let Some(spec) = self.ref_cache.get(uri) {
            return Ok(spec.clone());
        }

        if uri == "#" {
            let placeholder = SchemaSpec::make(SchemaSpecVariant::Any, "");
            self.ref_cache.insert(uri.to_owned(), placeholder);
            let root = self.root_schema.clone();
            let resolved = self.parse(&root, None)?;
            self.ref_cache.insert(uri.to_owned(), resolved.clone());
            return Ok(resolved);
        }

        if uri.len() < 2 || !uri.starts_with("#/") {
            return Ok(SchemaSpec::make(SchemaSpecVariant::Any, ""));
        }

        let parts: Vec<&str> =
            uri[2..].split('/').filter(|p| !p.is_empty()).collect();
        let mut current = &self.root_schema;
        for p in &parts {
            let Some(next) = current.as_object().and_then(|o| o.get(*p)) else {
                return Err(SchemaError::invalid(format!(
                    "Cannot find field {p} in {uri}"
                )));
            };
            current = next;
        }
        let current = current.clone();
        let resolved = self.parse(&current, None)?;
        self.ref_cache.insert(uri.to_owned(), resolved.clone());
        Ok(resolved)
    }

    fn parse_any_of(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<AnyOfSpec, SchemaError> {
        let key = if schema.contains_key("anyOf") {
            "anyOf"
        } else {
            "oneOf"
        };
        let Some(arr) = schema[key].as_array() else {
            return Err(SchemaError::invalid(format!(
                "{key} must be an array"
            )));
        };
        let mut spec = AnyOfSpec {
            options: Vec::new(),
        };
        for option in arr {
            spec.options.push(self.parse(option, None)?);
        }
        Ok(spec)
    }

    fn parse_all_of(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<AllOfSpec, SchemaError> {
        let Some(arr) = schema["allOf"].as_array() else {
            return Err(SchemaError::invalid("allOf must be an array"));
        };
        let mut spec = AllOfSpec {
            schemas: Vec::new(),
        };
        for sub in arr {
            spec.schemas.push(self.parse(sub, None)?);
        }
        Ok(spec)
    }

    fn parse_type_array(
        &mut self,
        schema: &serde_json::Map<String, Value>,
    ) -> Result<TypeArraySpec, SchemaError> {
        let type_array = schema["type"].as_array().unwrap().clone();
        let mut spec = TypeArraySpec {
            type_schemas: Vec::new(),
        };
        let mut schema_copy = schema.clone();
        if type_array.is_empty() {
            schema_copy.remove("type");
            let any = self.parse(&Value::Object(schema_copy), None)?;
            spec.type_schemas.push(any);
            return Ok(spec);
        }
        for ty in &type_array {
            if !ty.is_string() {
                return Err(SchemaError::invalid(
                    "type must be a string or an array of strings",
                ));
            }
            schema_copy.insert("type".to_owned(), ty.clone());
            let parsed =
                self.parse(&Value::Object(schema_copy.clone()), None)?;
            spec.type_schemas.push(parsed);
        }
        Ok(spec)
    }
}
