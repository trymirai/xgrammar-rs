//! XML-style function-call schema converters (Qwen, MiniMax, DeepSeek, GLM) — a port of
//! `XMLToolCallingConverter` in `cpp/json_schema_converter_ext.{h,cc}`.

use super::{
    json_schema_converter::json_schema_to_ebnf_xml, schema_error::SchemaError,
};

/// Which XML tool-calling wire format to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlJsonFormat {
    /// `<parameter=name>value</parameter>`
    Qwen,
    /// `<parameter name="name">value</parameter>`
    MiniMax,
    /// DeepSeek DSML parameter tags.
    DeepSeek,
    /// `<arg_key>key</arg_key><arg_value>value</arg_value>`
    Glm,
}

/// Tag wrappers for a given [`XmlJsonFormat`].
pub(crate) struct XmlWrapper {
    pub key_prefix: &'static str,
    pub key_suffix: &'static str,
    pub value_prefix: &'static str,
    pub param_suffix: &'static str,
}

/// Returns the XML tag wrappers for `format`.
#[must_use]
pub(crate) fn xml_wrapper(format: XmlJsonFormat) -> XmlWrapper {
    match format {
        XmlJsonFormat::Qwen => XmlWrapper {
            key_prefix: "<parameter=",
            key_suffix: ">",
            value_prefix: "",
            param_suffix: "</parameter>",
        },
        XmlJsonFormat::MiniMax => XmlWrapper {
            key_prefix: "<parameter name=\\\"",
            key_suffix: "\\\">",
            value_prefix: "",
            param_suffix: "</parameter>",
        },
        XmlJsonFormat::DeepSeek => XmlWrapper {
            key_prefix: "<｜DSML｜parameter name=\\\"",
            key_suffix: "\\\" string=\\\"\" (\"true\" | \"false\") \"\\\">",
            value_prefix: "",
            param_suffix: "</｜DSML｜parameter>",
        },
        XmlJsonFormat::Glm => XmlWrapper {
            key_prefix: "<arg_key>",
            key_suffix: "</arg_key>",
            value_prefix: "<arg_value>",
            param_suffix: "</arg_value>",
        },
    }
}

/// Parses a style string into [`XmlJsonFormat`].
pub(crate) fn xml_format_from_style(
    style: &str
) -> Result<XmlJsonFormat, SchemaError> {
    match style {
        "qwen_xml" => Ok(XmlJsonFormat::Qwen),
        "minimax_xml" => Ok(XmlJsonFormat::MiniMax),
        "deepseek_xml" => Ok(XmlJsonFormat::DeepSeek),
        "glm_xml" => Ok(XmlJsonFormat::Glm),
        other => {
            Err(SchemaError::invalid(format!("unsupported xml style: {other}")))
        },
    }
}

/// Converts a JSON Schema to EBNF using the given XML style string.
///
/// # Errors
/// Returns [`SchemaError`] when the schema or style is invalid.
pub fn xml_tool_calling_to_ebnf(
    schema: &str,
    style: &str,
) -> Result<String, SchemaError> {
    json_schema_to_ebnf_xml(schema, xml_format_from_style(style)?)
}

/// Qwen XML tool-calling EBNF (`<parameter=name>…</parameter>`).
///
/// # Errors
/// Returns [`SchemaError`] when the schema is invalid.
pub fn qwen_xml_tool_calling_to_ebnf(
    schema: &str
) -> Result<String, SchemaError> {
    json_schema_to_ebnf_xml(schema, XmlJsonFormat::Qwen)
}

/// MiniMax XML tool-calling EBNF.
///
/// # Errors
/// Returns [`SchemaError`] when the schema is invalid.
pub fn minimax_xml_tool_calling_to_ebnf(
    schema: &str
) -> Result<String, SchemaError> {
    json_schema_to_ebnf_xml(schema, XmlJsonFormat::MiniMax)
}

/// DeepSeek XML tool-calling EBNF.
///
/// # Errors
/// Returns [`SchemaError`] when the schema is invalid.
pub fn deepseek_xml_tool_calling_to_ebnf(
    schema: &str
) -> Result<String, SchemaError> {
    json_schema_to_ebnf_xml(schema, XmlJsonFormat::DeepSeek)
}

/// GLM XML tool-calling EBNF.
///
/// # Errors
/// Returns [`SchemaError`] when the schema is invalid.
pub fn glm_xml_tool_calling_to_ebnf(
    schema: &str
) -> Result<String, SchemaError> {
    json_schema_to_ebnf_xml(schema, XmlJsonFormat::Glm)
}
