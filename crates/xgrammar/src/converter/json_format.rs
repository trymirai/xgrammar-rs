//! The output encoding of a JSON-schema conversion.

/// How the converted grammar encodes a JSON Schema instance.
///
/// [`JsonFormat::Json`] produces plain JSON. The XML styles encode the root object as a
/// model-specific tool-call parameter list while nested values stay JSON (MiniMax-M3 and
/// Cohere encode nested objects and arrays recursively in their own tags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonFormat {
    /// Plain JSON.
    Json,
    /// `<parameter=name>value</parameter>`.
    QwenXml,
    /// `<parameter name="name">value</parameter>`.
    MiniMaxXml,
    /// DeepSeek DSML parameters with a `string` attribute.
    DeepSeekXml,
    /// `<arg_key>name</arg_key><arg_value>value</arg_value>`.
    GlmXml,
    /// Recursive Cohere `<cofl:value>` tags.
    CohereXml,
    /// Kimi-K3 `<|open|>argument key=".." type=".."<|sep|>` parameters.
    KimiK3Xml,
    /// MiniMax-M3 namespace-prefixed recursive elements.
    MiniMaxM3Xml,
    /// DeepSeek V4.1 DSML parameters.
    DeepSeekV41Xml,
}

impl JsonFormat {
    /// Parses a format name: `"json"`, `"qwen_xml"`, `"minimax_xml"`, `"minimax_m3_xml"`,
    /// `"deepseek_xml"`, `"deepseek_v4_1_xml"`, `"glm_xml"`, `"cohere_xml"`, or `"kimi_k3_xml"`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "json" => Self::Json,
            "qwen_xml" => Self::QwenXml,
            "minimax_xml" => Self::MiniMaxXml,
            "minimax_m3_xml" => Self::MiniMaxM3Xml,
            "deepseek_xml" => Self::DeepSeekXml,
            "deepseek_v4_1_xml" => Self::DeepSeekV41Xml,
            "glm_xml" => Self::GlmXml,
            "cohere_xml" => Self::CohereXml,
            "kimi_k3_xml" => Self::KimiK3Xml,
            _ => return None,
        })
    }

    /// Whether this is one of the DeepSeek DSML styles.
    #[must_use]
    pub fn is_deepseek(self) -> bool {
        matches!(self, Self::DeepSeekXml | Self::DeepSeekV41Xml)
    }
}
