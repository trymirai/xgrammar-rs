//! The intermediate representation for a parsed JSON Schema — a port of the `SchemaSpec`
//! variant family in `cpp/json_schema_converter.h`.
//!
//! `SchemaParser` produces a tree of these (shared via [`SchemaSpecPtr`] for `$ref` reuse
//! and de-duplication); `JsonSchemaConverter` walks the tree to emit EBNF.

use std::{collections::HashSet, rc::Rc};

/// A reference-counted, shared schema spec node.
pub(crate) type SchemaSpecPtr = Rc<SchemaSpec>;

/// `integer` type constraints.
#[derive(Debug, Clone, Default)]
pub(crate) struct IntegerSpec {
    pub minimum: Option<i64>,
    pub maximum: Option<i64>,
    pub exclusive_minimum: Option<i64>,
    pub exclusive_maximum: Option<i64>,
}

/// `number` type constraints.
#[derive(Debug, Clone, Default)]
pub(crate) struct NumberSpec {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub exclusive_minimum: Option<f64>,
    pub exclusive_maximum: Option<f64>,
}

/// `string` type constraints.
#[derive(Debug, Clone)]
pub(crate) struct StringSpec {
    pub pattern: Option<String>,
    pub format: Option<String>,
    pub min_length: i32,
    /// `-1` means no limit.
    pub max_length: i32,
}

impl Default for StringSpec {
    fn default() -> Self {
        Self {
            pattern: None,
            format: None,
            min_length: 0,
            max_length: -1,
        }
    }
}

/// `array` type constraints.
#[derive(Debug, Clone)]
pub(crate) struct ArraySpec {
    pub prefix_items: Vec<SchemaSpecPtr>,
    pub allow_additional_items: bool,
    /// `None` means additional items are not allowed.
    pub additional_items: Option<SchemaSpecPtr>,
    pub min_items: i64,
    /// `-1` means no limit.
    pub max_items: i64,
}

impl Default for ArraySpec {
    fn default() -> Self {
        Self {
            prefix_items: Vec::new(),
            allow_additional_items: true,
            additional_items: None,
            min_items: 0,
            max_items: -1,
        }
    }
}

/// A named object property.
#[derive(Debug, Clone)]
pub(crate) struct Property {
    pub name: String,
    pub schema: SchemaSpecPtr,
}

/// A `patternProperties` entry: a key-regex and its value schema.
#[derive(Debug, Clone)]
pub(crate) struct PatternProperty {
    pub pattern: String,
    pub schema: SchemaSpecPtr,
}

/// `object` type constraints.
#[derive(Debug, Clone)]
pub(crate) struct ObjectSpec {
    pub properties: Vec<Property>,
    pub pattern_properties: Vec<PatternProperty>,
    pub required: HashSet<String>,
    pub allow_additional_properties: bool,
    pub additional_properties_schema: Option<SchemaSpecPtr>,
    pub allow_unevaluated_properties: bool,
    pub unevaluated_properties_schema: Option<SchemaSpecPtr>,
    pub property_names: Option<SchemaSpecPtr>,
    pub min_properties: i32,
    /// `-1` means no limit.
    pub max_properties: i32,
}

impl Default for ObjectSpec {
    fn default() -> Self {
        Self {
            properties: Vec::new(),
            pattern_properties: Vec::new(),
            required: HashSet::new(),
            allow_additional_properties: false,
            additional_properties_schema: None,
            allow_unevaluated_properties: true,
            unevaluated_properties_schema: None,
            property_names: None,
            min_properties: 0,
            max_properties: -1,
        }
    }
}

/// A `const` value (stored as serialized JSON).
#[derive(Debug, Clone)]
pub(crate) struct ConstSpec {
    pub json_value: String,
}

/// An `enum` (stored as serialized JSON values).
#[derive(Debug, Clone)]
pub(crate) struct EnumSpec {
    pub json_values: Vec<String>,
}

/// A `$ref` to another part of the schema.
#[derive(Debug, Clone)]
pub(crate) struct RefSpec {
    pub uri: String,
}

/// An `anyOf` / `oneOf` alternation.
#[derive(Debug, Clone)]
pub(crate) struct AnyOfSpec {
    pub options: Vec<SchemaSpecPtr>,
}

/// An `allOf` intersection.
#[derive(Debug, Clone)]
pub(crate) struct AllOfSpec {
    pub schemas: Vec<SchemaSpecPtr>,
}

/// A `type: [...]` array of alternative type schemas.
#[derive(Debug, Clone)]
pub(crate) struct TypeArraySpec {
    pub type_schemas: Vec<SchemaSpecPtr>,
}

/// The tagged union of all schema kinds.
#[derive(Debug, Clone)]
pub(crate) enum SchemaSpecVariant {
    Integer(IntegerSpec),
    Number(NumberSpec),
    String(StringSpec),
    Boolean,
    Null,
    Array(ArraySpec),
    Object(ObjectSpec),
    Any,
    Const(ConstSpec),
    Enum(EnumSpec),
    Ref(RefSpec),
    AnyOf(AnyOfSpec),
    AllOf(AllOfSpec),
    TypeArray(TypeArraySpec),
}

/// A schema spec node: its kind plus a de-duplication cache key.
#[derive(Debug, Clone)]
pub(crate) struct SchemaSpec {
    pub spec: SchemaSpecVariant,
    pub cache_key: String,
}

impl SchemaSpec {
    /// Wraps a variant into a shared spec node.
    pub fn make(
        spec: SchemaSpecVariant,
        cache_key: impl Into<String>,
    ) -> SchemaSpecPtr {
        Rc::new(SchemaSpec {
            spec,
            cache_key: cache_key.into(),
        })
    }
}
