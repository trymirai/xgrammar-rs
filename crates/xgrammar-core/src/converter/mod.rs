//! Converters that turn external schema languages (regex, JSON Schema, structural tags)
//! into xgrammar grammars. Ported from `cpp/regex_converter.*`, `json_schema_converter.*`,
//! and `structural_tag.*`.
//!
//! One dedicated type per file; re-exported here.

mod regex_converter;
mod regex_error;

pub use regex_converter::regex_to_ebnf;
pub use regex_error::RegexError;
