//! The parameter tags of the XML tool-calling formats.

use super::json_format::JsonFormat;

/// The literal pieces around one XML tool-call parameter: `key_prefix` NAME `key_suffix`
/// `value_prefix` VALUE `parameter_suffix`.
#[derive(Debug, Clone, Copy)]
pub struct XmlWrapper {
    pub key_prefix: &'static str,
    pub key_suffix: &'static str,
    pub value_prefix: &'static str,
    pub parameter_suffix: &'static str,
}

impl XmlWrapper {
    /// Kimi-K3 renders the value's type between the key and `<|sep|>`; see
    /// [`Self::KIMI_K3_TYPES`].
    pub const KIMI_K3_TYPE_PREFIX: &'static str = "\" type=\"";
    /// The type attribute values of Kimi-K3 parameters.
    pub const KIMI_K3_TYPES: [&'static str; 7] = ["string", "number", "integer", "boolean", "object", "array", "null"];
    /// The end of a Kimi-K3 parameter's opening tag.
    pub const KIMI_K3_TYPE_SUFFIX: &'static str = "\"<|sep|>";

    /// The wrapper of an XML parameter-list format, or `None` for the other formats.
    #[must_use]
    pub fn for_format(format: JsonFormat) -> Option<Self> {
        let (key_prefix, key_suffix, value_prefix, parameter_suffix) = match format {
            JsonFormat::QwenXml => ("<parameter=", ">", "", "</parameter>"),
            JsonFormat::MiniMaxXml => ("<parameter name=\"", "\">", "", "</parameter>"),
            JsonFormat::DeepSeekXml => ("<｜DSML｜parameter name=\"", "", "", "</｜DSML｜parameter>"),
            JsonFormat::DeepSeekV41Xml => ("<｜DSML｜ parameter name=\"", "", "", "</｜DSML｜ parameter>"),
            JsonFormat::GlmXml => ("<arg_key>", "</arg_key>", "<arg_value>", "</arg_value>"),
            JsonFormat::CohereXml => ("<cofl:value", ">", "", "</cofl:value>"),
            // The key suffix (type attribute and <|sep|>) is generated per property.
            JsonFormat::KimiK3Xml => ("<|open|>argument key=\"", "", "", "<|close|>argument<|sep|>"),
            JsonFormat::Json | JsonFormat::MiniMaxM3Xml => return None,
        };
        Some(Self {
            key_prefix,
            key_suffix,
            value_prefix,
            parameter_suffix,
        })
    }
}
