//! `testing` submodule — converter and debugging helpers used by upstream tests.

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModuleMethods};

use crate::{
    bitmask_util::with_writable_i32_buffer, compiler::CompiledGrammar,
    error::map_error, grammar::Grammar, matcher::GrammarMatcher,
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
    // Read int64 tensors — resolve via numpy if needed.
    let read_i64 = |obj: &Bound<'_, PyAny>| -> PyResult<Vec<i64>> {
        let np = py.import("numpy")?;
        let arr = np.call_method1("asarray", (obj,))?;
        let dtype = arr.getattr("dtype")?;
        let kind = dtype.getattr("kind")?.extract::<String>()?;
        let bits = dtype.getattr("itemsize")?.extract::<i32>()?;
        if kind != "i" || bits != 8 {
            return Err(PyRuntimeError::new_err(
                "retrieve_next_token/sibling/draft_tokens must be int64",
            ));
        }
        let flat = arr.call_method0("ravel")?;
        use pyo3::buffer::PyBuffer;
        let buf = PyBuffer::<i64>::get(&flat)?;
        Ok(buf.to_vec(py)?)
    };

    let next_tok = read_i64(retrieve_next_token)?;
    let next_sib = read_i64(retrieve_next_sibling)?;
    let tokens = read_i64(draft_tokens)?;

    let n = next_tok.len();
    if next_sib.len() != n || tokens.len() != n {
        return Err(PyRuntimeError::new_err(
            "retrieve_next_token, retrieve_next_sibling, and draft_tokens must have the same length",
        ));
    }

    // bitmask shape
    let bitmask_shape: Vec<i64> = {
        let np = py.import("numpy")?;
        let arr = np.call_method1("asarray", (bitmask,))?;
        arr.getattr("shape")?
            .extract::<Vec<i64>>()
            .map_err(|_| PyRuntimeError::new_err("bitmask must be 2D"))?
    };
    if bitmask_shape.len() != 2 {
        return Err(PyRuntimeError::new_err("bitmask must be 2-dimensional"));
    }
    let bitmask_words = bitmask_shape[1] as usize;

    let start = std::time::Instant::now();

    with_writable_i32_buffer(py, bitmask, |buf| {
        Ok(traverse_dfs(
            0,
            usize::MAX,
            &next_tok,
            &next_sib,
            &tokens,
            &mut matcher.inner,
            buf,
            bitmask_words,
            time_threshold,
            start,
        ))
    })
}

fn traverse_dfs(
    curr: usize,
    parent_pos: usize,
    next_tok: &[i64],
    next_sib: &[i64],
    draft_tokens: &[i64],
    matcher: &mut xgrammar::matcher::GrammarMatcher,
    bitmask: &mut [i32],
    bitmask_words: usize,
    time_threshold: f64,
    start: std::time::Instant,
) -> bool {
    // Is the current node accepted by the grammar?
    let accepted = if curr == 0 {
        true // root is always accepted (it represents the current target-model position)
    } else {
        // Check whether the parent's bitmask allows this token.
        let token = draft_tokens[curr] as usize;
        let word = token / 32;
        let bit = token % 32;
        let parent_row = if parent_pos == usize::MAX {
            &bitmask[..bitmask_words]
        } else {
            &bitmask
                [parent_pos * bitmask_words..(parent_pos + 1) * bitmask_words]
        };
        word < parent_row.len() && (parent_row[word] >> bit) & 1 == 1
    };

    // Timeout check — only for non-root nodes.
    if curr != 0 && time_threshold > 0.0 {
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > time_threshold {
            return false;
        }
    }

    if accepted {
        if curr != 0 {
            matcher.accept_token(draft_tokens[curr] as i32);
        }

        if !matcher.is_terminated() {
            // Fill the bitmask row for this node using index=curr (batch row).
            let _ = matcher.fill_next_token_bitmask(bitmask, curr as i32);

            // Recurse to child.
            let child = next_tok[curr];
            if child != -1 {
                let success = traverse_dfs(
                    child as usize,
                    curr,
                    next_tok,
                    next_sib,
                    draft_tokens,
                    matcher,
                    bitmask,
                    bitmask_words,
                    time_threshold,
                    start,
                );
                if !success {
                    if curr != 0 {
                        matcher.rollback(1);
                    }
                    return false;
                }
            }
        }

        if curr != 0 {
            matcher.rollback(1);
        }
    }

    // Recurse to sibling.
    let sib = next_sib[curr];
    if sib != -1 {
        if !traverse_dfs(
            sib as usize,
            parent_pos,
            next_tok,
            next_sib,
            draft_tokens,
            matcher,
            bitmask,
            bitmask_words,
            time_threshold,
            start,
        ) {
            return false;
        }
    }

    true
}
