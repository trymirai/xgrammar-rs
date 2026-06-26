//! Cross-language bindings for the pure-Rust xgrammar core.
//!
//! Public types are annotated with `#[bindings::export(...)]` (the uzu approach), which emits
//! the per-backend glue (PyO3 / NAPI / UniFFI / wasm) gated behind the matching `bindings-*`
//! feature. The PyO3 module is assembled below by iterating the inventory of registered
//! classes.

#[cfg(feature = "bindings-uniffi")]
uniffi::setup_scaffolding!();

// Core types — exported to every backend via `#[bindings::export]`.
mod compiler;
mod error;
mod grammar;
mod matcher;
mod tokenizer_info;
mod vocab_type;

// PyO3-only surface: the torch/DLPack bitmask helpers, the `testing`/`config`/`kernels`
// submodules, and the batch matcher. These use the CPython C-API directly and only exist
// for the Python backend; the other backends expose the core grammar API only.
#[cfg(feature = "bindings-pyo3")]
mod batch_matcher;
#[cfg(feature = "bindings-pyo3")]
mod bitmask_util;
#[cfg(feature = "bindings-pyo3")]
mod config;
#[cfg(feature = "bindings-pyo3")]
mod grammar_functor;
#[cfg(feature = "bindings-pyo3")]
mod kernels;
#[cfg(feature = "bindings-pyo3")]
mod testing;

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
    kernels::register(&kernels_mod)?;
    m.add_submodule(&kernels_mod)?;

    Ok(())
}

#[cfg(feature = "bindings-pyo3")]
pyo3_stub_gen::define_stub_info_gatherer!(pyo3_bindings_annotations);
