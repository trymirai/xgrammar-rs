//! Cross-language bindings for the pure-Rust xgrammar core.
//!
//! Public types are annotated with `#[bindings::export(...)]` (the uzu approach), which emits
//! the per-backend glue (PyO3 / NAPI / UniFFI / wasm) gated behind the matching `bindings-*`
//! feature. The PyO3 module is assembled below by iterating the inventory of registered
//! classes.

#[cfg(feature = "bindings-uniffi")]
uniffi::setup_scaffolding!();

mod batch_matcher;
mod bitmask_util;
mod compiler;
mod config;
mod error;
mod grammar;
mod grammar_functor;
mod matcher;
mod testing;
mod tokenizer_info;
mod vocab_type;

/// The compiled PyO3 extension module (`xgrammar_rs`). Each `#[bindings::export]` type
/// registers itself via `inventory`; this loop adds them all to the module.
#[cfg(feature = "bindings-pyo3")]
#[pyo3::pymodule]
fn xgrammar_rs(
    m: &pyo3::Bound<'_, pyo3::types::PyModule>
) -> pyo3::PyResult<()> {
    use pyo3::types::PyModuleMethods;

    for entry in ::inventory::iter::<::bindings_types::PyClassRegistration>() {
        (entry.register)(m)?;
    }

    error::register_exceptions(m)?;
    batch_matcher::register(m)?;

    let py = m.py();
    let config_mod = pyo3::types::PyModule::new(py, "config")?;
    config::register(&config_mod)?;
    m.add_submodule(&config_mod)?;

    let testing_mod = pyo3::types::PyModule::new(py, "testing")?;
    testing::register(&testing_mod)?;

    let grammar_functor_mod =
        pyo3::types::PyModule::new(py, "grammar_functor")?;
    grammar_functor::register(&grammar_functor_mod)?;
    testing_mod.add_submodule(&grammar_functor_mod)?;

    m.add_submodule(&testing_mod)?;

    let kernels_mod = pyo3::types::PyModule::new(py, "kernels")?;
    m.add_submodule(&kernels_mod)?;

    Ok(())
}

#[cfg(feature = "bindings-pyo3")]
pyo3_stub_gen::define_stub_info_gatherer!(pyo3_bindings_annotations);
