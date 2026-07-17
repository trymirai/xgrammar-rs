//! Converters that turn external schema languages (regex, JSON Schema, structural tags)
//! into xgrammar grammars. Ported from `cpp/regex_converter.*`, `json_schema_converter.*`,
//! and `structural_tag.*`.
//!
//! One dedicated type per file; re-exported here.

mod ebnf_script_creator;
mod indent_manager;
mod json_schema_converter;
mod range_regex;
mod regex_converter;
mod regex_error;
mod schema_error;
mod schema_parser;
mod schema_spec;
mod structural_tag_converter;
mod structural_tag_error;
mod structural_tag_format;
mod structural_tag_parser;
mod xml_tool_calling_converter;

pub use ebnf_script_creator::EbnfScriptCreator;
pub use json_schema_converter::{
    json_schema_to_ebnf, json_schema_to_ebnf_with_any_order, json_schema_to_ebnf_xml_with_options,
};
pub use range_regex::{generate_float_range_regex, generate_float_range_regex_with_options, generate_range_regex};
pub use regex_converter::regex_to_ebnf;
pub use regex_error::RegexError;
pub use schema_error::{SchemaError, SchemaErrorKind};
pub use structural_tag_error::StructuralTagError;

/// Legacy structural-tag item (`begin` / `schema` / `end`) from the C++ public header.
///
/// Prefer the JSON structural-tag document + [`Grammar::from_structural_tag`]; this exists
/// for API parity with `xgrammar::StructuralTagItem`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralTagItem {
    /// Opening tag text.
    pub begin: String,
    /// JSON schema (as a string) between the tags.
    pub schema: String,
    /// Closing tag text.
    pub end: String,
}

pub use xml_tool_calling_converter::{
    XmlJsonFormat, deepseek_xml_tool_calling_to_ebnf, glm_xml_tool_calling_to_ebnf, minimax_xml_tool_calling_to_ebnf,
    qwen_xml_tool_calling_to_ebnf, xml_tool_calling_to_ebnf,
};
