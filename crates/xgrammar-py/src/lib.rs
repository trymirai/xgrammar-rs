//! Cross-language bindings for the pure-Rust xgrammar core.
//!
//! Public types are annotated with `#[bindings::export(...)]` (the uzu approach), which emits
//! the per-backend glue (PyO3 / NAPI / UniFFI / wasm) gated behind the matching `bindings-*`
//! feature. The PyO3 module is assembled below by iterating the inventory of registered
//! classes.

#[cfg(feature = "bindings-uniffi")]
uniffi::setup_scaffolding!();

mod grammar;

/// The compiled PyO3 extension module (`xgrammar_rs`). Each `#[bindings::export]` type
/// registers itself via `inventory`; this loop adds them all to the module.
#[cfg(feature = "bindings-pyo3")]
#[pyo3::pymodule]
fn xgrammar_rs(
    m: &pyo3::Bound<'_, pyo3::types::PyModule>
) -> pyo3::PyResult<()> {
    for entry in ::inventory::iter::<::bindings_types::PyClassRegistration>() {
        (entry.register)(m)?;
    }
    Ok(())
}

#[cfg(feature = "bindings-pyo3")]
pyo3_stub_gen::define_stub_info_gatherer!(pyo3_bindings_annotations);
