//! `config` submodule — recursion depth and serialization version.

use pyo3::{prelude::*, types::PyModuleMethods};

/// Registers the config submodule functions.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(get_max_recursion_depth, m)?)?;
    m.add_function(wrap_pyfunction!(set_max_recursion_depth, m)?)?;
    m.add_function(wrap_pyfunction!(get_serialization_version, m)?)?;
    m.add_function(wrap_pyfunction!(get_bitmask_dl_type, m)?)?;
    Ok(())
}

#[pyfunction]
fn get_max_recursion_depth() -> i32 {
    xgrammar::support::get_max_recursion_depth()
}

#[pyfunction]
fn set_max_recursion_depth(max_recursion_depth: i32) -> PyResult<()> {
    xgrammar::support::set_max_recursion_depth(max_recursion_depth).map_err(crate::error::map_error)
}

#[pyfunction]
fn get_serialization_version() -> &'static str {
    xgrammar::config::get_serialization_version()
}

/// Returns `(code, bits, lanes)` for the bitmask DLPack dtype (int32).
#[pyfunction]
fn get_bitmask_dl_type() -> (u8, u8, u16) {
    let t = xgrammar::matcher::get_bitmask_dl_type();
    (t.code, t.bits, t.lanes)
}
