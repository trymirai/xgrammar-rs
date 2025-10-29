use std::pin::Pin;

use autocxx::prelude::*;

use crate::{
    ffi::{cxx_utils, xgrammar::Grammar as FFIGrammar},
    grammar::structural_tag_item::StructuralTagItem,
};

/// This class represents a grammar object in XGrammar, and can be used later in the
/// grammar-guided generation.
///
/// The Grammar object supports context-free grammar (CFG). EBNF (extended Backus-Naur Form) is
/// used as the format of the grammar. There are many specifications for EBNF in the literature,
/// and we follow the specification of GBNF (GGML BNF) in
/// https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md.
///
/// When formatted with Display, the grammar will be converted to GBNF format.
pub struct Grammar {
    inner: Pin<Box<FFIGrammar>>,
}

impl core::fmt::Display for Grammar {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{}", self.inner.ToString().to_string())
    }
}

impl Grammar {
    /// Print the BNF grammar to a string, in EBNF format.
    pub fn to_string_ebnf(&self) -> String {
        self.inner.ToString().to_string()
    }

    /// Construct a grammar from EBNF string. The EBNF string should follow the format
    /// in https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md.
    ///
    /// Parameters
    /// - `ebnf_string`: The grammar string in EBNF format.
    /// - `root_rule_name`: The name of the root rule in the grammar.
    ///
    /// Errors
    /// - Panics if converting the EBNF fails. The C++ layer would normally throw with details.
    pub fn from_ebnf(
        ebnf_string: &str,
        root_rule_name: &str,
    ) -> Self {
        cxx::let_cxx_string!(ebnf_cxx = ebnf_string);
        cxx::let_cxx_string!(root_rule_name_cxx = root_rule_name);
        let ffi_grammar = FFIGrammar::FromEBNF(&ebnf_cxx, &root_rule_name_cxx);
        Self {
            inner: ffi_grammar.within_box(),
        }
    }

    /// Construct a grammar from JSON schema.
    ///
    /// It allows any whitespace by default. If you want to specify the format of the JSON,
    /// set `any_whitespace` to false and use the `indent` and `separators` parameters. The
    /// meaning and the default values of the parameters follows the convention in `serde_json`
    ////python `json.dumps()`.
    ///
    /// It internally converts the JSON schema to an EBNF grammar.
    ///
    /// Parameters
    /// - `schema`: The schema string (JSON schema as a string).
    /// - `any_whitespace`: Whether to use any whitespace. If true, the generated grammar will
    ///   ignore the indent and separators parameters, and allow any whitespace.
    /// - `indent`: The number of spaces for indentation. If None, the output will be in one line.
    ///   Note that specifying the indentation means forcing the LLM to generate JSON strings
    ///   strictly formatted. However, some models may tend to generate JSON strings that are not
    ///   strictly formatted. In this case, forcing the LLM to generate strictly formatted JSON
    ///   strings may degrade the generation quality.
    /// - `separators`: Two separators used in the schema: comma and colon. Examples: (",", ":"),
    ///   (", ", ": "). If None, the default separators will be used: (",", ": ") when the
    ///   indent is Some, and (", ", ": ") otherwise.
    /// - `strict_mode`: Whether to use strict mode. In strict mode, the generated grammar will not
    ///   allow properties and items that is not specified in the schema. This is equivalent to
    ///   setting unevaluatedProperties and unevaluatedItems to false.
    /// - `print_converted_ebnf`: If true, the converted EBNF string will be printed. For debugging.
    ///
    /// Errors
    /// - Panics if converting the JSON schema fails. The C++ layer would normally throw with details.
    pub fn from_json_schema(
        schema: &str,
        any_whitespace: bool,
        indent: Option<i32>,
        separators: Option<(impl AsRef<str>, impl AsRef<str>)>,
        strict_mode: bool,
        print_converted_ebnf: bool,
    ) -> Self {
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
        cxx::let_cxx_string!(separator_comma_cxx = separator_comma.as_str());
        cxx::let_cxx_string!(separator_colon_cxx = separator_colon.as_str());
        let ffi_grammar = cxx_utils::grammar_from_json_schema(
            &schema_cxx,
            any_whitespace,
            has_indent,
            indent_i32,
            has_separators,
            &separator_comma_cxx,
            &separator_colon_cxx,
            strict_mode,
            print_converted_ebnf,
        );
        Self {
            inner: ffi_grammar.within_box(),
        }
    }

    /// Create a grammar from a regular expression string.
    ///
    /// Parameters
    /// - `regex_string`: The regular expression pattern to create the grammar from.
    /// - `print_converted_ebnf`: If true, print the converted EBNF for debugging.
    ///
    /// Errors
    /// - Panics if parsing the regex fails. The C++ layer would normally throw with details.
    pub fn from_regex(
        regex_string: &str,
        print_converted_ebnf: bool,
    ) -> Self {
        cxx::let_cxx_string!(regex_cxx = regex_string);
        let ffi_grammar =
            FFIGrammar::FromRegex(&regex_cxx, print_converted_ebnf);
        Self {
            inner: ffi_grammar.within_box(),
        }
    }

    /// Create a grammar from structural tags.
    ///
    /// The structural tag handles the dispatching of different grammars based on the tags and
    /// triggers: it initially allows any output, until a trigger is encountered, then dispatch to
    /// the corresponding tag; when the end tag is encountered, the grammar will allow any following
    /// output, until the next trigger is encountered.
    ///
    /// Parameters
    /// - `tags`: The structural tags.
    /// - `triggers`: The triggers. Each trigger should be a prefix of a provided begin tag.
    ///
    /// Returns
    /// - The constructed grammar.
    pub fn from_structural_tag(
        tags: &[StructuralTagItem],
        triggers: &[impl AsRef<str>],
    ) -> Self {
        let mut structural_tag_vector = cxx_utils::new_structural_tag_vector();
        let mut trigger_string_vector = cxx_utils::new_string_vector();

        {
            let mut structural_tag_vector_pin = structural_tag_vector.pin_mut();
            let mut trigger_vector_pin = trigger_string_vector.pin_mut();

            cxx_utils::structural_tag_vec_reserve(
                structural_tag_vector_pin.as_mut(),
                tags.len(),
            );
            cxx_utils::string_vec_reserve(
                trigger_vector_pin.as_mut(),
                triggers.len(),
            );

            for tag in tags {
                cxx::let_cxx_string!(begin_cxx = tag.begin.as_str());
                cxx::let_cxx_string!(schema_cxx = tag.schema.as_str());
                cxx::let_cxx_string!(end_cxx = tag.end.as_str());
                cxx_utils::structural_tag_vec_push(
                    structural_tag_vector_pin.as_mut(),
                    &begin_cxx,
                    &schema_cxx,
                    &end_cxx,
                );
            }
            for trig in triggers {
                let trigger_bytes = trig.as_ref().as_bytes();
                unsafe {
                    cxx_utils::string_vec_push_bytes(
                        trigger_vector_pin.as_mut(),
                        trigger_bytes.as_ptr() as *const i8,
                        trigger_bytes.len(),
                    );
                }
            }
        }

        let ffi_grammar = cxx_utils::grammar_from_structural_tags(
            structural_tag_vector.as_ref().unwrap(),
            trigger_string_vector.as_ref().unwrap(),
        );
        Self {
            inner: ffi_grammar.within_box(),
        }
    }

    /// Get the grammar of standard JSON. This is compatible with the official JSON grammar
    /// specification in https://www.json.org/json-en.html.
    pub fn builtin_json_grammar() -> Self {
        let ffi_grammar = FFIGrammar::BuiltinJSONGrammar();
        Self {
            inner: ffi_grammar.within_box(),
        }
    }

    /// Create a grammar that matches the concatenation of the grammars in the slice. That is
    /// equivalent to using the `+` operator to concatenate the grammars in the slice.
    pub fn concat(grammars: &[Grammar]) -> Self {
        assert!(!grammars.is_empty(), "concat requires at least one grammar");
        let mut vec = cxx_utils::new_grammar_vector();
        {
            let mut vec_pin = vec.pin_mut();
            cxx_utils::grammar_vec_reserve(vec_pin.as_mut(), grammars.len());
            for g in grammars {
                cxx_utils::grammar_vec_push(vec_pin.as_mut(), g.ffi_ref());
            }
        }
        let combined = cxx_utils::grammar_concat(vec.as_ref().unwrap());
        Self {
            inner: combined.within_box(),
        }
    }

    /// Create a grammar that matches any of the grammars in the slice. That is equivalent to
    /// using the `|` operator to create the union of the grammars in the slice.
    pub fn union(grammars: &[Grammar]) -> Self {
        assert!(!grammars.is_empty(), "union requires at least one grammar");
        let mut vec = cxx_utils::new_grammar_vector();
        {
            let mut vec_pin = vec.pin_mut();
            cxx_utils::grammar_vec_reserve(vec_pin.as_mut(), grammars.len());
            for g in grammars {
                cxx_utils::grammar_vec_push(vec_pin.as_mut(), g.ffi_ref());
            }
        }
        let combined = cxx_utils::grammar_union(vec.as_ref().unwrap());
        Self {
            inner: combined.within_box(),
        }
    }

    /// Serialize the grammar to a JSON string.
    pub fn serialize_json(&self) -> String {
        self.inner.SerializeJSON().to_string()
    }

    /// Deserialize a grammar from a JSON string.
    ///
    /// Returns
    /// - `Ok(Grammar)` on success
    /// - `Err(String)` when deserialization fails due to invalid JSON, format mismatch, or
    ///   serialization version mismatch (via the `__VERSION__` field). The error string mirrors
    ///   the C++ exception message.
    pub fn deserialize_json(json_string: &str) -> Result<Self, String> {
        cxx::let_cxx_string!(json_cxx = json_string);
        cxx::let_cxx_string!(error_out_cxx = "");
        let unique_ptr = unsafe {
            cxx_utils::grammar_deserialize_json_or_error(
                &json_cxx,
                error_out_cxx.as_mut().get_unchecked_mut(),
            )
        };
        if unique_ptr.is_null() {
            return Err(error_out_cxx.to_string());
        }
        let raw_ptr = unique_ptr.into_raw();
        let boxed_ffi = unsafe { Box::from_raw(raw_ptr) };
        let pinned_ffi = unsafe { Pin::new_unchecked(boxed_ffi) };
        Ok(Self {
            inner: pinned_ffi,
        })
    }

    pub(crate) fn ffi_ref(&self) -> &FFIGrammar {
        self.inner.as_ref().get_ref()
    }

    pub(crate) fn from_pinned_ffi(inner: Pin<Box<FFIGrammar>>) -> Self {
        Self {
            inner,
        }
    }
}
