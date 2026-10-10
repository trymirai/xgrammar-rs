//! `testing.grammar_functor` submodule — grammar transformation passes for tests.

use pyo3::{prelude::*, types::PyModuleMethods};

use crate::{error::map_error, grammar::Grammar};

/// Registers grammar-functor helpers on `m`.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(structure_normalizer, m)?)?;
    m.add_function(wrap_pyfunction!(rule_inliner, m)?)?;
    m.add_function(wrap_pyfunction!(byte_string_fuser, m)?)?;
    m.add_function(wrap_pyfunction!(dead_code_eliminator, m)?)?;
    m.add_function(wrap_pyfunction!(lookahead_assertion_analyzer, m)?)?;
    m.add_function(wrap_pyfunction!(grammar_optimizer, m)?)?;
    m.add_function(wrap_pyfunction!(fsm_builder, m)?)?;
    m.add_function(wrap_pyfunction!(repetition_normalizer, m)?)?;
    Ok(())
}

#[pyfunction]
fn structure_normalizer(grammar: &Grammar) -> Grammar {
    Grammar::from(xgrammar::functor::structure_normalizer(grammar.as_ref()))
}

#[pyfunction]
fn rule_inliner(grammar: &Grammar) -> Grammar {
    let mut g = grammar.as_ref().clone();
    xgrammar::functor::rule_inliner(&mut g);
    Grammar::from(g)
}

#[pyfunction]
fn byte_string_fuser(grammar: &Grammar) -> Grammar {
    let mut g = grammar.as_ref().clone();
    xgrammar::functor::byte_string_fuser(&mut g);
    Grammar::from(g)
}

#[pyfunction]
fn dead_code_eliminator(grammar: &Grammar) -> Grammar {
    Grammar::from(xgrammar::functor::dead_code_eliminator(grammar.as_ref()))
}

#[pyfunction]
fn lookahead_assertion_analyzer(grammar: &Grammar) -> Grammar {
    Grammar::from(xgrammar::functor::lookahead_assertion_analyzer(grammar.as_ref()))
}

#[pyfunction]
fn grammar_optimizer(grammar: &Grammar) -> PyResult<Grammar> {
    xgrammar::functor::grammar_optimizer(grammar.as_ref()).map(Grammar::from).map_err(map_error)
}

#[pyfunction]
fn fsm_builder(grammar: &Grammar) -> PyResult<Grammar> {
    let mut g = grammar.as_ref().clone();
    xgrammar::functor::GrammarFsmBuilder::apply(&mut g).map_err(map_error)?;
    Ok(Grammar::from(g))
}

#[pyfunction]
fn repetition_normalizer(grammar: &Grammar) -> Grammar {
    let mut g = grammar.as_ref().clone();
    xgrammar::functor::repetition_normalizer(&mut g);
    Grammar::from(g)
}
