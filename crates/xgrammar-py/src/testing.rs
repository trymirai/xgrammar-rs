//! `testing` submodule — converter and debugging helpers used by upstream tests.

use pyo3::{prelude::*, types::PyModuleMethods};

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
    m.add_function(wrap_pyfunction!(_qwen_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_minimax_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_deepseek_xml_tool_calling_to_ebnf, m)?)?;
    m.add_function(wrap_pyfunction!(_glm_xml_tool_calling_to_ebnf, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(signature = (schema, any_whitespace=true, indent=None, separators=None, strict_mode=true, max_whitespace_cnt=None))]
fn _json_schema_to_ebnf(
    schema: String,
    any_whitespace: bool,
    indent: Option<i32>,
    separators: Option<(String, String)>,
    strict_mode: bool,
    max_whitespace_cnt: Option<i32>,
) -> PyResult<String> {
    let seps = separators.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
    xgrammar::converter::json_schema_to_ebnf(
        &schema,
        any_whitespace,
        indent,
        seps,
        strict_mode,
        max_whitespace_cnt,
    )
    .map_err(map_error)
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
    py: Python<'_>,
    bitmask: &Bound<'_, PyAny>,
    shape: Vec<i64>,
    vocab_size: i32,
    index: i32,
) -> PyResult<Vec<i32>> {
    let data = crate::bitmask_util::read_i32_buffer(py, bitmask)?;
    let row = crate::bitmask_util::bitmask_row_slice(&data, &shape, index)?;
    Ok(xgrammar::matcher::get_masked_tokens_from_bitmask(row, vocab_size, 0))
}

#[pyfunction]
fn _is_single_token_bitmask(
    py: Python<'_>,
    bitmask: &Bound<'_, PyAny>,
    shape: Vec<i64>,
    vocab_size: i32,
    index: i32,
) -> PyResult<(bool, i32)> {
    let data = crate::bitmask_util::read_i32_buffer(py, bitmask)?;
    let row = crate::bitmask_util::bitmask_row_slice(&data, &shape, index)?;
    Ok(xgrammar::matcher::is_single_token_bitmask(row, vocab_size, 0))
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
) -> String {
    xgrammar::converter::generate_float_range_regex(start, end)
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
