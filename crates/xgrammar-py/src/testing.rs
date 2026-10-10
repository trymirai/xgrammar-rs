//! `testing` submodule — converter and debugging helpers used by upstream tests.

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};

use crate::{compiler::CompiledGrammar, error::map_error, grammar::Grammar};

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
    m.add_function(wrap_pyfunction!(_is_rule_fsm_accept_string, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(signature = (schema, any_whitespace=true, indent=None, separators=None, strict_mode=true, max_whitespace_cnt=None, json_format="json", any_order=false))]
#[allow(clippy::too_many_arguments, reason = "mirrors the upstream `testing._json_schema_to_ebnf` signature")]
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
    let format = xgrammar::converter::JsonFormat::from_name(json_format)
        .ok_or_else(|| PyRuntimeError::new_err(format!("Invalid json_format: {json_format}")))?;
    xgrammar::converter::json_schema_to_ebnf(
        &schema,
        any_whitespace,
        indent,
        seps,
        strict_mode,
        max_whitespace_cnt,
        format,
        any_order,
    )
    .map_err(map_error)
}

#[pyfunction]
#[pyo3(signature = (regex, with_rule_name=true))]
fn _regex_to_ebnf(
    regex: String,
    with_rule_name: bool,
) -> PyResult<String> {
    xgrammar::converter::regex_to_ebnf(&regex, with_rule_name).map_err(map_error)
}

#[pyfunction]
#[pyo3(signature = (ebnf_string, root_rule_name="root"))]
fn _ebnf_to_grammar_no_normalization(
    ebnf_string: String,
    root_rule_name: &str,
) -> PyResult<Grammar> {
    Ok(Grammar::wrap(
        xgrammar::parser::ebnf_to_grammar_no_normalization(&ebnf_string, root_rule_name).map_err(map_error)?,
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
    let range = xgrammar::matcher::bitmask_row_range(shape, vocab_size, index).map_err(map_error)?;

    // SAFETY: the Python facade passes `Tensor.data_ptr()` for a contiguous CPU int32 tensor of
    // `shape`. The row range is validated against the shape above, and the resulting slice does
    // not outlive this call. The Python tensor remains owned by the caller for the call.
    let row = unsafe { std::slice::from_raw_parts((bitmask_ptr as *const i32).add(range.start), range.len()) };
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
    xgrammar::converter::generate_float_range_regex_with_options(start, end, exclusive_start, exclusive_end)
}

#[pyfunction]
fn _print_grammar_fsms(grammar: &Grammar) -> PyResult<String> {
    xgrammar::testing::print_grammar_fsms(&grammar.inner).map_err(map_error)
}

#[pyfunction]
fn _is_rule_fsm_accept_string(
    grammar: &Grammar,
    rule_id: i32,
    input_str: String,
) -> PyResult<bool> {
    xgrammar::testing::is_rule_fsm_accept_string(&grammar.inner, rule_id, &input_str).map_err(map_error)
}
