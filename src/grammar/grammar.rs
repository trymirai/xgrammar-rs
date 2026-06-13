use crate::{CxxUniquePtr, DeserializeError, StructuralTagError, TokenizerInfo, ffi};

/// This class represents a grammar object in XGrammar, and can be used later in the
/// grammar-guided generation.
///
/// The Grammar object supports context-free grammar (CFG). EBNF (extended Backus-Naur Form) is
/// used as the format of the grammar. There are many specifications for EBNF in the literature,
/// and we follow the specification of GBNF (GGML BNF) in
/// <https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md>
///
/// When formatted with Display, the grammar will be converted to GBNF format.
pub struct Grammar {
    inner: CxxUniquePtr<ffi::Grammar>,
}

impl core::fmt::Display for Grammar {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{}", self.to_string_ebnf())
    }
}

impl Grammar {
    /// Print the BNF grammar to a string, in EBNF format.
    ///
    /// # Returns
    ///
    /// The BNF grammar string.
    pub fn to_string_ebnf(&self) -> String {
        ffi::grammar_to_string(&self.inner).to_string()
    }

    /// Construct a grammar from EBNF string. The EBNF string should follow the format
    /// in <https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md>.
    ///
    /// # Parameters
    ///
    /// - `ebnf_string`: The grammar string in EBNF format.
    /// - `root_rule_name`: The name of the root rule in the grammar.
    ///
    /// # Errors
    ///
    /// Returns an error if the EBNF string is invalid or parsing fails.
    pub fn from_ebnf(
        ebnf_string: &str,
        root_rule_name: &str,
    ) -> Result<Self, String> {
        cxx::let_cxx_string!(ebnf_cxx = ebnf_string);
        cxx::let_cxx_string!(root_rule_name_cxx = root_rule_name);
        cxx::let_cxx_string!(error_out_cxx = "");
        let ffi_ptr = unsafe {
            ffi::grammar_from_ebnf(
                &ebnf_cxx,
                &root_rule_name_cxx,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if ffi_ptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        Ok(Self {
            inner: ffi_ptr,
        })
    }

    /// Construct a grammar from JSON schema.
    ///
    /// It allows any whitespace by default. If you want to specify the format of the JSON,
    /// set `any_whitespace` to false and use the `indent` and `separators` parameters. The
    /// meaning and the default values of the parameters follows the convention in `json.dumps()`.
    ///
    /// It internally converts the JSON schema to an EBNF grammar.
    ///
    /// # Parameters
    ///
    /// - `schema`: The schema string.
    /// - `any_whitespace`: Whether to use any whitespace. If true, the generated grammar will
    ///   ignore the indent and separators parameters, and allow any whitespace.
    /// - `indent`: The number of spaces for indentation. If `None`, the output will be in one line.
    ///   Note that specifying the indentation means forcing the LLM to generate JSON strings
    ///   strictly formatted. However, some models may tend to generate JSON strings that are not
    ///   strictly formatted. In this case, forcing the LLM to generate strictly formatted JSON
    ///   strings may degrade the generation quality. See
    ///   <https://github.com/sgl-project/sglang/issues/2216#issuecomment-2516192009> for more details.
    /// - `separators`: Two separators used in the schema: comma and colon. Examples: `(",", ":")`,
    ///   `(", ", ": ")`. If `None`, the default separators will be used: `(",", ": ")` when the
    ///   indent is not `None`, and `(", ", ": ")` otherwise.
    /// - `strict_mode`: Whether to use strict mode. In strict mode, the generated grammar will not
    ///   allow properties and items that is not specified in the schema. This is equivalent to
    ///   setting `unevaluatedProperties` and `unevaluatedItems` to false. This helps LLM to
    ///   generate accurate output in the grammar-guided generation with JSON schema.
    /// - `max_whitespace_cnt`: The maximum number of whitespace characters allowed between
    ///   elements, such like keys, values, separators and so on. If `None`, there is no limit
    ///   on the number of whitespace characters. If specified, it will limit the number of
    ///   whitespace characters to at most `max_whitespace_cnt`. It should be a positive integer.
    /// - `print_converted_ebnf`: If true, the converted EBNF string will be printed.
    ///   For debugging purposes.
    ///
    /// # Returns
    ///
    /// The constructed grammar.
    ///
    /// # Errors
    ///
    /// When converting the JSON schema fails, with details about the parsing error.
    pub fn from_json_schema(
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(impl AsRef<str>, impl AsRef<str>)>,
        strict_mode: bool,
        max_whitespace_cnt: Option<i32>,
        print_converted_ebnf: bool,
    ) -> Result<Self, String> {
        cxx::let_cxx_string!(schema_cxx = schema);
        let has_indent = indent.is_some();
        let indent_i32: i32 = indent.unwrap_or(0) as i32;
        let has_separators = separators.is_some();
        let (separator_comma, separator_colon) = if let Some((
            separator_comma_ref,
            separator_colon_ref,
        )) = separators
        {
            (
                separator_comma_ref.as_ref().to_string(),
                separator_colon_ref.as_ref().to_string(),
            )
        } else {
            (String::new(), String::new())
        };
        let has_max_whitespace_cnt = max_whitespace_cnt.is_some();
        let max_whitespace_cnt_i32: i32 = max_whitespace_cnt.unwrap_or(0);
        cxx::let_cxx_string!(separator_comma_cxx = separator_comma.as_str());
        cxx::let_cxx_string!(separator_colon_cxx = separator_colon.as_str());
        cxx::let_cxx_string!(error_out_cxx = "");
        let ffi_ptr = unsafe {
            ffi::grammar_from_json_schema(
                &schema_cxx,
                any_whitespace,
                has_indent,
                indent_i32,
                has_separators,
                &separator_comma_cxx,
                &separator_colon_cxx,
                strict_mode,
                has_max_whitespace_cnt,
                max_whitespace_cnt_i32,
                print_converted_ebnf,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if ffi_ptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        Ok(Self {
            inner: ffi_ptr,
        })
    }

    /// Create a grammar from a regular expression string.
    ///
    /// # Parameters
    ///
    /// - `regex_string`: The regular expression pattern to create the grammar from.
    /// - `print_converted_ebnf`: This method will convert the regex pattern to EBNF first.
    ///   If this is true, the converted EBNF string will be printed. For debugging purposes.
    ///
    /// # Returns
    ///
    /// The constructed grammar from the regex pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid or parsing fails.
    pub fn from_regex(
        regex_string: &str,
        print_converted_ebnf: bool,
    ) -> Result<Self, String> {
        cxx::let_cxx_string!(regex_cxx = regex_string);
        cxx::let_cxx_string!(error_out_cxx = "");
        let ffi_ptr = unsafe {
            ffi::grammar_from_regex(
                &regex_cxx,
                print_converted_ebnf,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if ffi_ptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        Ok(Self {
            inner: ffi_ptr,
        })
    }

    /// Create a grammar from a structural tag. See the Structural Tag Usage in XGrammar
    /// documentation for its usage.
    ///
    /// # Parameters
    ///
    /// - `structural_tag_json`: The structural tag either as a JSON string or a dictionary.
    ///
    /// # Returns
    ///
    /// The constructed grammar from the structural tag.
    ///
    /// # Errors
    ///
    /// - When the structural tag is not a valid JSON string.
    /// - When the structural tag is not valid.
    pub fn from_structural_tag(
        structural_tag_json: &str
    ) -> Result<Self, StructuralTagError> {
        Self::from_structural_tag_impl(structural_tag_json, std::ptr::null())
    }

    /// Tokenizer-aware variant of [`Self::from_structural_tag`] that resolves token-based formats
    /// against the given tokenizer info.
    pub fn from_structural_tag_with_tokenizer_info(
        structural_tag_json: &str,
        tokenizer_info: &TokenizerInfo,
    ) -> Result<Self, StructuralTagError> {
        Self::from_structural_tag_impl(structural_tag_json, tokenizer_info.ffi_ref() as *const _)
    }

    fn from_structural_tag_impl(
        structural_tag_json: &str,
        tokenizer_info: *const ffi::TokenizerInfo,
    ) -> Result<Self, StructuralTagError> {
        cxx::let_cxx_string!(json_cxx = structural_tag_json);
        cxx::let_cxx_string!(error_out_cxx = "");
        let mut error_kind: i32 = 0;
        let unique_ptr = unsafe {
            ffi::grammar_from_structural_tag(
                &json_cxx,
                tokenizer_info,
                &mut error_kind,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if unique_ptr.is_null() {
            return Err(StructuralTagError::from_parts(error_kind, error_out_cxx.to_string()));
        }
        Ok(Self {
            inner: unique_ptr,
        })
    }

    /// Get the grammar of standard JSON. This is compatible with the official JSON grammar
    /// specification in <https://www.json.org/json-en.html>.
    ///
    /// # Returns
    ///
    /// The JSON grammar.
    pub fn builtin_json_grammar() -> Self {
        let ffi_ptr = ffi::grammar_builtin_json_grammar();
        Self {
            inner: ffi_ptr,
        }
    }

    /// Create a grammar that matches the concatenation of the grammars in the list. That is
    /// equivalent to using the `+` operator to concatenate the grammars in the list.
    ///
    /// # Parameters
    ///
    /// - `grammars`: The grammars to create the concatenation of.
    ///
    /// # Returns
    ///
    /// The concatenation of the grammars.
    pub fn concat(grammars: &[Grammar]) -> Self {
        assert!(!grammars.is_empty(), "concat requires at least one grammar");
        let mut vec = ffi::new_grammar_vector();
        {
            let mut vec_pin = vec.pin_mut();
            ffi::grammar_vec_reserve(vec_pin.as_mut(), grammars.len());
            for grammar in grammars {
                ffi::grammar_vec_push(vec_pin.as_mut(), grammar.ffi_ref());
            }
        }
        let ffi_ptr = ffi::grammar_concat(vec.as_ref().unwrap());
        Self {
            inner: ffi_ptr,
        }
    }

    /// Create a grammar that matches any of the grammars in the list. That is equivalent to
    /// using the `|` operator to concatenate the grammars in the list.
    ///
    /// # Parameters
    ///
    /// - `grammars`: The grammars to create the union of.
    ///
    /// # Returns
    ///
    /// The union of the grammars.
    pub fn union(grammars: &[Grammar]) -> Self {
        assert!(!grammars.is_empty(), "union requires at least one grammar");
        let mut vec = ffi::new_grammar_vector();
        {
            let mut vec_pin = vec.pin_mut();
            ffi::grammar_vec_reserve(vec_pin.as_mut(), grammars.len());
            for g in grammars {
                ffi::grammar_vec_push(vec_pin.as_mut(), g.ffi_ref());
            }
        }
        let ffi_ptr = ffi::grammar_union(vec.as_ref().unwrap());
        Self {
            inner: ffi_ptr,
        }
    }

    /// Serialize the grammar to a JSON string.
    ///
    /// # Returns
    ///
    /// The JSON string.
    pub fn serialize_json(&self) -> String {
        ffi::grammar_serialize_json(
            self.inner.as_ref().expect("ffi::Grammar UniquePtr was null"),
        )
        .to_string()
    }

    /// Deserialize a grammar from a JSON string.
    ///
    /// # Parameters
    ///
    /// - `json_string`: The JSON string.
    ///
    /// # Returns
    ///
    /// The deserialized grammar.
    ///
    /// # Errors
    ///
    /// - When the JSON string is invalid.
    /// - When the JSON string does not follow the serialization format of the grammar.
    /// - When the `__VERSION__` field in the JSON string is not the same as the current version.
    pub fn deserialize_json(json_string: &str) -> Result<Self, DeserializeError> {
        cxx::let_cxx_string!(json_cxx = json_string);
        cxx::let_cxx_string!(error_out_cxx = "");
        let mut error_kind: i32 = 0;
        let unique_ptr = unsafe {
            ffi::grammar_deserialize_json_or_error(
                &json_cxx,
                &mut error_kind,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if unique_ptr.is_null() {
            return Err(DeserializeError::from_parts(error_kind, error_out_cxx.to_string()));
        }
        Ok(Self {
            inner: unique_ptr,
        })
    }

    pub(crate) fn ffi_ref(&self) -> &ffi::Grammar {
        self.inner.as_ref().expect("ffi::Grammar UniquePtr was null")
    }

    pub(crate) fn from_unique_ptr(inner: cxx::UniquePtr<ffi::Grammar>) -> Self {
        Self {
            inner,
        }
    }
}

impl Drop for Grammar {
    fn drop(&mut self) {}
}
