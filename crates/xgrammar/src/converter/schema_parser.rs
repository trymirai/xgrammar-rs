//! Parses a JSON Schema (a [`serde_json::Value`]) into the [`SchemaSpec`] IR — a port of
//! `SchemaParser` in `cpp/json_schema_converter.cc`.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{
    schema_error::SchemaError,
    schema_spec::{
        AllOfSpec, AnyOfSpec, ArraySpec, ConstSpec, EnumSpec, IntegerSpec, NumberSpec, ObjectSpec, PatternProperty,
        Property, RefSpec, SchemaSpec, SchemaSpecPtr, SchemaSpecVariant, StringSpec, TypeArraySpec,
    },
};

/// Object keys ignored when computing a schema's de-duplication cache key.
const SKIPPED_CACHE_KEYS: &[&str] =
    &["title", "default", "description", "examples", "deprecated", "readOnly", "writeOnly", "$comment", "$schema"];

const UNSUPPORTED_ONE_OF_MESSAGE: &str = "oneOf with overlapping or non-provably-disjoint branches cannot be represented exactly; falling back to anyOf semantics";

fn has_only_schema_keys(
    schema: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> bool {
    schema.keys().all(|key| allowed.contains(&key.as_str()) || SKIPPED_CACHE_KEYS.contains(&key.as_str()))
}

fn normalize_type_set(value: &Value) -> Option<HashSet<String>> {
    const SUPPORTED_TYPES: &[&str] = &["null", "boolean", "object", "array", "number", "string", "integer"];
    let mut result = HashSet::new();
    match value {
        Value::String(ty) if SUPPORTED_TYPES.contains(&ty.as_str()) => {
            result.insert(ty.clone());
        },
        Value::Array(types) if !types.is_empty() => {
            for value in types {
                let ty = value.as_str()?;
                if !SUPPORTED_TYPES.contains(&ty) {
                    return None;
                }
                result.insert(ty.to_owned());
            }
        },
        _ => return None,
    }
    Some(result)
}

fn is_numeric_value(value: &Value) -> bool {
    value.is_number()
}

fn is_integer_value(value: &Value) -> bool {
    let Some(number) = value.as_number() else {
        return false;
    };
    if !number.is_f64() {
        return true;
    }
    number.as_f64().is_some_and(|value| value.is_finite() && value.floor() == value)
}

/// JSON-number comparisons are deliberately conservative when either side was parsed as a
/// floating-point value. This matches upstream and avoids claiming `oneOf` disjointness after a
/// potentially lossy conversion.
fn json_values_may_overlap(
    lhs: &Value,
    rhs: &Value,
) -> bool {
    if is_numeric_value(lhs) || is_numeric_value(rhs) {
        if !is_numeric_value(lhs) || !is_numeric_value(rhs) {
            return false;
        }
        let lhs_number = lhs.as_number().expect("checked numeric");
        let rhs_number = rhs.as_number().expect("checked numeric");
        return if !lhs_number.is_f64() && !rhs_number.is_f64() {
            lhs_number.to_string() == rhs_number.to_string()
        } else {
            true
        };
    }
    match (lhs, rhs) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(lhs), Value::Bool(rhs)) => lhs == rhs,
        (Value::String(lhs), Value::String(rhs)) => lhs == rhs,
        (Value::Array(lhs), Value::Array(rhs)) => {
            lhs.len() == rhs.len() && lhs.iter().zip(rhs).all(|(lhs, rhs)| json_values_may_overlap(lhs, rhs))
        },
        (Value::Object(lhs), Value::Object(rhs)) => {
            lhs.len() == rhs.len()
                && lhs.iter().all(|(key, lhs_value)| {
                    rhs.get(key).is_some_and(|rhs_value| json_values_may_overlap(lhs_value, rhs_value))
                })
        },
        _ => false,
    }
}

fn value_matches_type(
    value: &Value,
    ty: &str,
) -> bool {
    match ty {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "integer" => is_integer_value(value),
        "number" => is_numeric_value(value),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn finite_values(schema: &serde_json::Map<String, Value>) -> Option<Vec<Value>> {
    if let Some(value) = schema.get("const") {
        return Some(vec![value.clone()]);
    }
    let values = schema.get("enum")?.as_array()?;
    (!values.is_empty()).then(|| values.clone())
}

enum OneOfArmProof {
    TypeSet(HashSet<String>),
    FiniteValues(Vec<Value>),
}

fn classify_one_of_arm(option: &Value) -> Option<OneOfArmProof> {
    let schema = option.as_object()?;
    if ["$ref", "anyOf", "allOf", "oneOf"].iter().any(|key| schema.contains_key(*key)) {
        return None;
    }
    if let Some(values) = finite_values(schema) {
        return Some(OneOfArmProof::FiniteValues(values));
    }
    if !has_only_schema_keys(schema, &["type"]) {
        return None;
    }
    let type_set = normalize_type_set(schema.get("type")?)?;
    if type_set.contains("object") {
        return None;
    }
    Some(OneOfArmProof::TypeSet(type_set))
}

fn type_sets_overlap(
    lhs: &HashSet<String>,
    rhs: &HashSet<String>,
) -> bool {
    lhs.iter().any(|lhs_type| {
        rhs.iter().any(|rhs_type| {
            lhs_type == rhs_type
                || (["integer", "number"].contains(&lhs_type.as_str())
                    && ["integer", "number"].contains(&rhs_type.as_str()))
        })
    })
}

fn finite_values_overlap(
    lhs: &[Value],
    rhs: &[Value],
) -> bool {
    lhs.iter().any(|lhs_value| rhs.iter().any(|rhs_value| json_values_may_overlap(lhs_value, rhs_value)))
}

fn finite_values_overlap_type_set(
    values: &[Value],
    type_set: &HashSet<String>,
) -> bool {
    values.iter().any(|value| {
        (is_numeric_value(value) && (type_set.contains("integer") || type_set.contains("number")))
            || type_set.iter().any(|ty| value_matches_type(value, ty))
    })
}

fn one_of_arm_proofs_are_disjoint(
    lhs: &OneOfArmProof,
    rhs: &OneOfArmProof,
) -> bool {
    match (lhs, rhs) {
        (OneOfArmProof::TypeSet(lhs), OneOfArmProof::TypeSet(rhs)) => !type_sets_overlap(lhs, rhs),
        (OneOfArmProof::FiniteValues(lhs), OneOfArmProof::FiniteValues(rhs)) => !finite_values_overlap(lhs, rhs),
        (OneOfArmProof::FiniteValues(values), OneOfArmProof::TypeSet(types))
        | (OneOfArmProof::TypeSet(types), OneOfArmProof::FiniteValues(values)) => {
            !finite_values_overlap_type_set(values, types)
        },
    }
}

fn discriminator_values(
    option: &Value,
    discriminator_key: &str,
) -> Option<Vec<Value>> {
    let schema = option.as_object()?;
    if ["$ref", "anyOf", "allOf", "oneOf"].iter().any(|key| schema.contains_key(*key))
        || schema.get("type")?.as_str()? != "object"
    {
        return None;
    }
    let requires_key =
        schema.get("required")?.as_array()?.iter().any(|value| value.as_str() == Some(discriminator_key));
    if !requires_key {
        return None;
    }
    let property_schema = schema.get("properties")?.as_object()?.get(discriminator_key)?.as_object()?;
    finite_values(property_schema)
}

fn discriminator_candidates(option: &Value) -> Vec<String> {
    let Some(schema) = option.as_object() else {
        return Vec::new();
    };
    let (Some(required), Some(properties)) =
        (schema.get("required").and_then(Value::as_array), schema.get("properties").and_then(Value::as_object))
    else {
        return Vec::new();
    };
    required
        .iter()
        .filter_map(Value::as_str)
        .filter(|key| properties.get(*key).and_then(Value::as_object).and_then(finite_values).is_some())
        .map(str::to_owned)
        .collect()
}

fn try_prove_discriminator_one_of(options: &[Value]) -> bool {
    let Some(first) = options.first() else {
        return false;
    };
    discriminator_candidates(first).into_iter().any(|key| {
        let branch_values = options.iter().map(|option| discriminator_values(option, &key)).collect::<Option<Vec<_>>>();
        branch_values.is_some_and(|values| {
            (0..values.len())
                .all(|lhs| (lhs + 1..values.len()).all(|rhs| !finite_values_overlap(&values[lhs], &values[rhs])))
        })
    })
}

fn try_prove_type_or_finite_one_of(options: &[Value]) -> bool {
    let Some(proofs) = options.iter().map(classify_one_of_arm).collect::<Option<Vec<_>>>() else {
        return false;
    };
    (0..proofs.len())
        .all(|lhs| (lhs + 1..proofs.len()).all(|rhs| one_of_arm_proofs_are_disjoint(&proofs[lhs], &proofs[rhs])))
}

fn try_prove_pairwise_disjoint_one_of(options: &[Value]) -> bool {
    !options.is_empty() && (try_prove_discriminator_one_of(options) || try_prove_type_or_finite_one_of(options))
}

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
                let mut kvs: Vec<(&String, &Value)> =
                    map.iter().filter(|(k, _)| !SKIPPED_CACHE_KEYS.contains(&k.as_str())).collect();
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
                return Err(SchemaError::unsatisfiable("Schema 'false' cannot accept any value"));
            }
            let spec = SchemaSpec::make(SchemaSpecVariant::Any, cache_key.clone());
            self.schema_cache.insert(cache_key, spec.clone());
            return Ok(spec);
        }

        let Some(obj) = schema.as_object() else {
            return Err(SchemaError::invalid(format!("Schema should be an object or bool, but got {schema}")));
        };

        let make = |variant: SchemaSpecVariant| SchemaSpec::make(variant, cache_key.clone());

        let result: SchemaSpecPtr = if obj.contains_key("$ref") {
            make(SchemaSpecVariant::Ref(Self::parse_ref(obj)?))
        } else if obj.contains_key("const") {
            make(SchemaSpecVariant::Const(Self::parse_const(obj)))
        } else if obj.contains_key("enum") {
            make(SchemaSpecVariant::Enum(Self::parse_enum(obj)?))
        } else if obj.contains_key("anyOf") {
            make(SchemaSpecVariant::AnyOf(self.parse_any_of(obj)?))
        } else if obj.contains_key("oneOf") {
            let options = obj["oneOf"].as_array().ok_or_else(|| SchemaError::invalid("oneOf must be an array"))?;
            if !try_prove_pairwise_disjoint_one_of(options) {
                eprintln!("{UNSUPPORTED_ONE_OF_MESSAGE}");
            }
            make(SchemaSpecVariant::AnyOf(self.parse_any_of(obj)?))
        } else if obj.contains_key("allOf") {
            make(SchemaSpecVariant::AllOf(self.parse_all_of(obj)?))
        } else if obj.contains_key("type") || default_type.is_some() {
            if obj.get("type").is_some_and(Value::is_array) {
                make(SchemaSpecVariant::TypeArray(self.parse_type_array(obj)?))
            } else {
                if obj.contains_key("type") && !obj["type"].is_string() {
                    return Err(SchemaError::invalid("Type should be a string"));
                }
                let ty = if obj.contains_key("type") {
                    obj["type"].as_str().unwrap()
                } else {
                    default_type.unwrap()
                };
                match ty {
                    "integer" => make(SchemaSpecVariant::Integer(Self::parse_integer(obj)?)),
                    "number" => make(SchemaSpecVariant::Number(Self::parse_number(obj)?)),
                    "string" => make(SchemaSpecVariant::String(Self::parse_string(obj)?)),
                    "boolean" => make(SchemaSpecVariant::Boolean),
                    "null" => make(SchemaSpecVariant::Null),
                    "array" => make(SchemaSpecVariant::Array(self.parse_array(obj)?)),
                    "object" => make(SchemaSpecVariant::Object(self.parse_object(obj)?)),
                    other => {
                        return Err(SchemaError::invalid(format!("Unsupported type \"{other}\"")));
                    },
                }
            }
        } else if obj.contains_key("properties")
            || obj.contains_key("additionalProperties")
            || obj.contains_key("unevaluatedProperties")
        {
            make(SchemaSpecVariant::Object(self.parse_object(obj)?))
        } else if obj.contains_key("items") || obj.contains_key("prefixItems") || obj.contains_key("unevaluatedItems") {
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
            return Err(SchemaError::invalid("Integer constraint must be a whole number"));
        }
        if val > i64::MAX as f64 {
            return Err(SchemaError::invalid("Integer exceeds maximum limit"));
        }
        if val < i64::MIN as f64 {
            return Err(SchemaError::invalid("Integer exceeds minimum limit"));
        }
        Ok(val as i64)
    }

    fn parse_integer(schema: &serde_json::Map<String, Value>) -> Result<IntegerSpec, SchemaError> {
        const INTEGER_MULTIPLE_OF_MAX: f64 = 1024.0;
        const INTEGER_MULTIPLE_OF_RANGE_WIDTH_MAX: i128 = 10_000;

        let mut spec = IntegerSpec::default();
        if let Some(value) = schema.get("multipleOf") {
            let Some(multiple_of) = value.as_f64() else {
                return Err(SchemaError::invalid("Value must be a number"));
            };
            if multiple_of <= 0.0 {
                return Err(SchemaError::invalid("multipleOf must be greater than 0"));
            }
            if multiple_of != multiple_of.floor() {
                eprintln!("multipleOf for type:integer must be an integer; ignoring multipleOf");
            } else if multiple_of > INTEGER_MULTIPLE_OF_MAX {
                eprintln!("multipleOf for type:integer must be > 0 and <= 1024; ignoring multipleOf");
            } else {
                spec.multiple_of = Some(multiple_of as i64);
            }
        }
        if let Some(v) = schema.get("minimum") {
            spec.minimum = Some(Self::check_integer_bound(v)?);
        }
        if let Some(v) = schema.get("maximum") {
            spec.maximum = Some(Self::check_integer_bound(v)?);
        }
        if let Some(v) = schema.get("exclusiveMinimum") {
            let val = Self::check_integer_bound(v)?;
            if val == i64::MAX {
                return Err(SchemaError::unsatisfiable("exclusiveMinimum would cause integer overflow"));
            }
            spec.exclusive_minimum = Some(val);
        }
        if let Some(v) = schema.get("exclusiveMaximum") {
            let val = Self::check_integer_bound(v)?;
            if val == i64::MIN {
                return Err(SchemaError::unsatisfiable("exclusiveMaximum would cause integer underflow"));
            }
            spec.exclusive_maximum = Some(val);
        }

        let (start, end) = spec.effective_range();
        let effective_min = start.unwrap_or(i64::MIN);
        let effective_max = end.unwrap_or(i64::MAX);
        if effective_min > effective_max {
            return Err(SchemaError::unsatisfiable("Invalid range: minimum greater than maximum"));
        }
        if let Some(multiple_of) = spec.multiple_of {
            let has_lower_bound = start.is_some();
            let has_upper_bound = end.is_some();
            if has_lower_bound || has_upper_bound {
                let range_width = i128::from(effective_max) - i128::from(effective_min) + 1;
                if !has_lower_bound || !has_upper_bound || range_width > INTEGER_MULTIPLE_OF_RANGE_WIDTH_MAX {
                    eprintln!("range + multipleOf combination not yet supported; ignoring multipleOf");
                    spec.multiple_of = None;
                } else if !(effective_min..=effective_max).any(|value| value % multiple_of == 0) {
                    return Err(SchemaError::unsatisfiable("range contains no multipleOf value"));
                }
            }
        }
        Ok(spec)
    }

    fn parse_number(schema: &serde_json::Map<String, Value>) -> Result<NumberSpec, SchemaError> {
        let get_double = |v: &Value| -> Result<f64, SchemaError> {
            v.as_f64().ok_or_else(|| SchemaError::invalid("Value must be a number"))
        };
        if let Some(value) = schema.get("multipleOf") {
            let Some(multiple_of) = value.as_f64() else {
                return Err(SchemaError::invalid("Value must be a number"));
            };
            if multiple_of <= 0.0 {
                return Err(SchemaError::invalid("multipleOf must be greater than 0"));
            }
            eprintln!("multipleOf is not supported for type:number; ignoring multipleOf");
        }

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

        let empty_range = spec.minimum.zip(spec.maximum).is_some_and(|(minimum, maximum)| minimum > maximum)
            || spec.minimum.zip(spec.exclusive_maximum).is_some_and(|(minimum, maximum)| minimum >= maximum)
            || spec.exclusive_minimum.zip(spec.maximum).is_some_and(|(minimum, maximum)| minimum >= maximum)
            || spec.exclusive_minimum.zip(spec.exclusive_maximum).is_some_and(|(minimum, maximum)| minimum >= maximum);
        if empty_range {
            return Err(SchemaError::unsatisfiable("Invalid range: empty range"));
        }
        Ok(spec)
    }

    fn parse_string(schema: &serde_json::Map<String, Value>) -> Result<StringSpec, SchemaError> {
        let mut spec = StringSpec::default();
        if let Some(v) = schema.get("format") {
            spec.format = v.as_str().map(str::to_owned);
        }
        if let Some(v) = schema.get("pattern") {
            spec.pattern = v.as_str().map(str::to_owned);
        }
        if let Some(v) = schema.get("minLength") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("minLength must be an integer"));
            };
            spec.min_length = n as i32;
        }
        if let Some(v) = schema.get("maxLength") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("maxLength must be an integer"));
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
                return Err(SchemaError::invalid("prefixItems must be an array"));
            };
            for item in arr {
                if item.as_bool() == Some(false) {
                    return Err(SchemaError::unsatisfiable("prefixItems contains false"));
                } else if !item.is_object() {
                    return Err(SchemaError::invalid("prefixItems must be an array of objects or booleans"));
                }
                spec.prefix_items.push(self.parse(item, None)?);
            }
        }

        if let Some(items) = schema.get("items") {
            if !items.is_boolean() && !items.is_object() {
                return Err(SchemaError::invalid("items must be a boolean or an object"));
            }
            if items.as_bool() == Some(false) {
                spec.allow_additional_items = false;
            } else {
                spec.allow_additional_items = true;
                spec.additional_items = Some(self.parse(items, None)?);
            }
        } else if let Some(uneval) = schema.get("unevaluatedItems") {
            if !uneval.is_boolean() && !uneval.is_object() {
                return Err(SchemaError::invalid("unevaluatedItems must be a boolean or an object"));
            }
            if uneval.as_bool() == Some(false) {
                spec.allow_additional_items = false;
            } else {
                spec.allow_additional_items = true;
                spec.additional_items = Some(self.parse(uneval, None)?);
            }
        } else if !self.strict_mode {
            spec.allow_additional_items = true;
            spec.additional_items = Some(SchemaSpec::make(SchemaSpecVariant::Any, ""));
        } else {
            spec.allow_additional_items = false;
        }

        if let Some(v) = schema.get("minItems") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("minItems must be an integer"));
            };
            spec.min_items = n.max(0);
        }
        if let Some(v) = schema.get("minContains") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("minContains must be an integer"));
            };
            spec.min_items = spec.min_items.max(n);
        }
        if let Some(v) = schema.get("maxItems") {
            let n = v.as_i64().filter(|n| *n >= 0);
            let Some(n) = n else {
                return Err(SchemaError::invalid("maxItems must be a non-negative integer"));
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
                return Err(SchemaError::invalid("properties must be an object"));
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
                return Err(SchemaError::invalid("patternProperties must be an object"));
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
                return Err(SchemaError::invalid("propertyNames must be an object"));
            };
            if pn_obj.get("type").and_then(Value::as_str).is_some_and(|t| t != "string") {
                return Err(SchemaError::unsatisfiable("propertyNames must be an object that validates string"));
            }
            spec.property_names = Some(self.parse(pn, Some("string"))?);
        }

        spec.allow_additional_properties = !self.strict_mode;
        if let Some(add) = schema.get("additionalProperties") {
            if let Some(b) = add.as_bool() {
                spec.allow_additional_properties = b;
            } else {
                spec.allow_additional_properties = true;
                spec.additional_properties_schema = Some(self.parse(add, None)?);
            }
        }

        spec.allow_unevaluated_properties = true;
        if schema.contains_key("additionalProperties") {
            spec.allow_unevaluated_properties = spec.allow_additional_properties;
        } else if let Some(uneval) = schema.get("unevaluatedProperties") {
            if let Some(b) = uneval.as_bool() {
                spec.allow_unevaluated_properties = b;
            } else {
                spec.allow_unevaluated_properties = true;
                spec.unevaluated_properties_schema = Some(self.parse(uneval, None)?);
            }
        } else if self.strict_mode {
            spec.allow_unevaluated_properties = false;
        }

        if let Some(v) = schema.get("minProperties") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("minProperties must be an integer"));
            };
            spec.min_properties = n as i32;
            if spec.min_properties < 0 {
                return Err(SchemaError::unsatisfiable("minProperties must be a non-negative integer"));
            }
        }
        if let Some(v) = schema.get("maxProperties") {
            let Some(n) = v.as_i64() else {
                return Err(SchemaError::invalid("maxProperties must be an integer"));
            };
            spec.max_properties = n as i32;
            if spec.max_properties < 0 {
                return Err(SchemaError::unsatisfiable("maxProperties must be a non-negative integer"));
            }
        }

        if spec.max_properties != -1 && spec.min_properties > spec.max_properties {
            return Err(SchemaError::unsatisfiable(format!(
                "minProperties is greater than maxProperties: {} > {}",
                spec.min_properties, spec.max_properties
            )));
        }
        if spec.max_properties != -1 && spec.required.len() as i32 > spec.max_properties {
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

    fn parse_enum(schema: &serde_json::Map<String, Value>) -> Result<EnumSpec, SchemaError> {
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

    fn parse_ref(schema: &serde_json::Map<String, Value>) -> Result<RefSpec, SchemaError> {
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

        let parts: Vec<&str> = uri[2..].split('/').filter(|p| !p.is_empty()).collect();
        let mut current = &self.root_schema;
        for p in &parts {
            let Some(next) = current.as_object().and_then(|o| o.get(*p)) else {
                return Err(SchemaError::invalid(format!("Cannot find field {p} in {uri}")));
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
            return Err(SchemaError::invalid(format!("{key} must be an array")));
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
                return Err(SchemaError::invalid("type must be a string or an array of strings"));
            }
            schema_copy.insert("type".to_owned(), ty.clone());
            let parsed = self.parse(&Value::Object(schema_copy.clone()), None)?;
            spec.type_schemas.push(parsed);
        }
        Ok(spec)
    }
}
