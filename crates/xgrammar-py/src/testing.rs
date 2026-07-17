//! `testing` submodule — converter and debugging helpers used by upstream tests.

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};

use crate::{
    bitmask_util::{i32_shape_2d, read_i64_1d, with_writable_i32_buffer},
    compiler::CompiledGrammar,
    error::map_error,
    grammar::Grammar,
    matcher::GrammarMatcher,
};

/// Registers testing helpers on `m`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(_json_schema_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_regex_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_ebnf_to_grammar_no_normalization, m)?)?;
    m.add_function(wrap_pyfunction!(_get_masked_tokens_from_bitmask, m)?)?;
    m.add_function(wrap_pyfunction!(_is_single_token_bitmask, m)?)?;
    m.add_function(wrap_pyfunction!(_get_allow_empty_rule_ids, m)?)?;
    m.add_function(wrap_pyfunction!(_generate_range_regex, m)?)?;
    m.add_function(wrap_pyfunction!(_generate_float_regex, m)?)?;
    m.add_function(wrap_pyfunction!(_print_grammar_fsms, m)?)?;
    m.add_function(wrap_pyfunction!(_qwen_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_minimax_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_deepseek_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_glm_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_traverse_draft_tree, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(signature = (schema, any_whitespace=true, indent=None, separators=None, strict_mode=true, max_whitespace_cnt=None, json_format="json", any_order=false))]
fn _json_schema_to_ebnf(
    schema: String,
    any_whitespace: bool,
    indent: Option<i32>,
    separators: Option<(String, String)>,
    strict_mode: bool,
    max_whitespace_cnt: Option<i32>,
    json_format: &str,
    any_order: bool,
) -> PyResult<String> {
    let seps = separators.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
    let result = if json_format == "json" {
        xgrammar::converter::json_schema_to_ebnf_with_any_order(
            &schema,
            any_whitespace,
            indent,
            seps,
            strict_mode,
            max_whitespace_cnt,
            any_order,
        )
    } else {
        let format = match json_format {
            "qwen_xml" => xgrammar::converter::XmlJsonFormat::Qwen,
            "minimax_xml" => xgrammar::converter::XmlJsonFormat::MiniMax,
            "deepseek_xml" => xgrammar::converter::XmlJsonFormat::DeepSeek,
            "glm_xml" => xgrammar::converter::XmlJsonFormat::Glm,
            other => {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "unsupported json format: {other}"
                )));
            },
        };
        xgrammar::converter::json_schema_to_ebnf_xml_with_options(
            &schema,
            format,
            max_whitespace_cnt,
            any_order,
        )
    };
    result.map_err(map_error)
}

#[pyfunction]
#[pyo3(signature = (regex, with_rule_name=true))]
fn _regex_to_ebnf(
    regex: String,
    with_rule_name: bool,
) -> PyResult<String> {
    xgrammar::converter::regex_to_ebnf(&regex, with_rule_name)
        .map_err(map_error)
}

#[pyfunction]
#[pyo3(signature = (ebnf_string, root_rule_name="root"))]
fn _ebnf_to_grammar_no_normalization(
    ebnf_string: String,
    root_rule_name: &str,
) -> PyResult<Grammar> {
    Ok(Grammar::wrap(
        xgrammar::parser::ebnf_to_grammar_no_normalization(
            &ebnf_string,
            root_rule_name,
        )
        .map_err(map_error)?,
    ))
}

#[pyfunction]
fn _get_masked_tokens_from_bitmask(
    bitmask_ptr: usize,
    shape: Vec<i64>,
    vocab_size: i32,
    index: i32,
) -> PyResult<Vec<i32>> {
    with_raw_bitmask_row(bitmask_ptr, &shape, vocab_size, index, |row| {
        xgrammar::matcher::get_masked_tokens_from_bitmask(row, vocab_size, 0)
    })
}

#[pyfunction]
fn _is_single_token_bitmask(
    bitmask_ptr: usize,
    shape: Vec<i64>,
    vocab_size: i32,
    index: i32,
) -> PyResult<(bool, i32)> {
    with_raw_bitmask_row(bitmask_ptr, &shape, vocab_size, index, |row| {
        xgrammar::matcher::is_single_token_bitmask(row, vocab_size, 0)
    })
}

/// Borrows one row from the raw CPU `torch.int32` pointer passed by upstream's Python helper.
///
/// Keeping the pointer conversion here mirrors the C++ binding boundary while the xgrammar core
/// continues to operate only on safe Rust slices.
fn with_raw_bitmask_row<R>(
    bitmask_ptr: usize,
    shape: &[i64],
    vocab_size: i32,
    index: i32,
    f: impl FnOnce(&[i32]) -> R,
) -> PyResult<R> {
    if bitmask_ptr == 0 || bitmask_ptr % std::mem::align_of::<i32>() != 0 {
        return Err(PyRuntimeError::new_err("invalid bitmask data pointer"));
    }
    if vocab_size < 0 {
        return Err(PyRuntimeError::new_err("vocab_size must be non-negative"));
    }
    let row_words = xgrammar::matcher::get_bitmask_size(vocab_size) as usize;
    let row_index = match shape {
        [words] if *words == row_words as i64 && index == 0 => 0,
        [_, _] if index < 0 => {
            return Err(PyRuntimeError::new_err(
                "The provided index is out of bounds",
            ));
        },
        [rows, words]
            if *words == row_words as i64 && i64::from(index) < *rows =>
        {
            index as usize
        },
        [_] => {
            return Err(PyRuntimeError::new_err(
                "The index should be 0 and shape must match for a 1D bitmask",
            ));
        },
        [_, _] => {
            return Err(PyRuntimeError::new_err(
                "The provided bitmask shape or index is not valid",
            ));
        },
        _ => {
            return Err(PyRuntimeError::new_err(
                "token_bitmask tensor must be 1D or 2D",
            ));
        },
    };
    let offset = row_index
        .checked_mul(row_words)
        .ok_or_else(|| PyRuntimeError::new_err("bitmask offset overflow"))?;

    // SAFETY: the Python facade passes `Tensor.data_ptr()` for a contiguous CPU int32 tensor.
    // The shape and row index are validated above, and the resulting slice does not outlive this
    // function call. The Python tensor remains owned by the caller for the duration of the call.
    let row = unsafe {
        std::slice::from_raw_parts(
            (bitmask_ptr as *const i32).add(offset),
            row_words,
        )
    };
    Ok(f(row))
}

#[pyfunction]
fn _get_allow_empty_rule_ids(compiled_grammar: &CompiledGrammar) -> Vec<i32> {
    compiled_grammar.inner.grammar().allow_empty_rule_ids().to_vec()
}

#[pyfunction]
fn _generate_range_regex(
    start: Option<i64>,
    end: Option<i64>,
) -> String {
    xgrammar::converter::generate_range_regex(start, end)
}

#[pyfunction]
fn _generate_float_regex(
    start: Option<f64>,
    end: Option<f64>,
    exclusive_start: bool,
    exclusive_end: bool,
) -> String {
    xgrammar::converter::generate_float_range_regex_with_options(
        start,
        end,
        exclusive_start,
        exclusive_end,
    )
}

#[pyfunction]
fn _print_grammar_fsms(grammar: &Grammar) -> String {
    xgrammar::testing::print_grammar_fsms(&grammar.inner)
}

#[pyfunction]
fn _qwen_xml_tool_calling_to_ebnf(schema: String) -> PyResult<String> {
    xgrammar::converter::qwen_xml_tool_calling_to_ebnf(&schema)
        .map_err(map_schema_error)
}

#[pyfunction]
fn _minimax_xml_tool_calling_to_ebnf(schema: String) -> PyResult<String> {
    xgrammar::converter::minimax_xml_tool_calling_to_ebnf(&schema)
        .map_err(map_schema_error)
}

#[pyfunction]
fn _deepseek_xml_tool_calling_to_ebnf(schema: String) -> PyResult<String> {
    xgrammar::converter::deepseek_xml_tool_calling_to_ebnf(&schema)
        .map_err(map_schema_error)
}

#[pyfunction]
fn _glm_xml_tool_calling_to_ebnf(schema: String) -> PyResult<String> {
    xgrammar::converter::glm_xml_tool_calling_to_ebnf(&schema)
        .map_err(map_schema_error)
}

fn map_schema_error(error: xgrammar::converter::SchemaError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(error.to_string())
}

/// Port of the C++ `TraverseDraftTree` from `cpp/testing.cc`.
///
/// Called by `testing.py::_traverse_draft_tree`. All three tree tensors must be 1-D int64;
/// the bitmask must be a 2-D int32 CPU tensor (num_nodes × bitmask_words).
///
/// Returns `True` if the full traversal completed, `False` if it timed out
/// (`time_threshold > 0` and elapsed seconds exceeded it).
#[pyfunction]
#[pyo3(signature = (retrieve_next_token, retrieve_next_sibling, draft_tokens, matcher, bitmask, time_threshold=-1.0))]
fn _traverse_draft_tree(
    py: Python<'_>,
    retrieve_next_token: &Bound<'_, PyAny>,
    retrieve_next_sibling: &Bound<'_, PyAny>,
    draft_tokens: &Bound<'_, PyAny>,
    matcher: &mut GrammarMatcher,
    bitmask: &Bound<'_, PyAny>,
    time_threshold: f64,
) -> PyResult<bool> {
    let next_tok = read_i64_1d(py, retrieve_next_token, "retrieve_next_token")?;
    let next_sib =
        read_i64_1d(py, retrieve_next_sibling, "retrieve_next_sibling")?;
    let tokens = read_i64_1d(py, draft_tokens, "draft_tokens")?;
    let shape = i32_shape_2d(py, bitmask, "token_bitmask")?;
    if shape[0] != next_tok.len() as i64 {
        return Err(PyRuntimeError::new_err(
            "the token_bitmask batch size must match the number of nodes in the tree",
        ));
    }
    with_writable_i32_buffer(py, bitmask, |buf| {
        matcher
            .lock()
            .traverse_draft_tree(
                &next_tok,
                &next_sib,
                &tokens,
                buf,
                time_threshold,
            )
            .map_err(PyRuntimeError::new_err)
    })
}
