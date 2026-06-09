//! Kirin's Python **front-end**: lower a subset of Python to Kirin IR, run it,
//! and expose it to Python via PyO3.
//!
//! "Front-end lowering" here means **Python AST → Kirin IR** — distinct from
//! Kirin's internal *stage* lowering (`@source` → `@lowered`), which is a
//! separate concept inside the IR pipeline.
//!
//! Layers in this one crate:
//! - [`ast`] — a pure-Rust mirror of the supported Python AST subset (`PyAst`).
//! - the `lower` visitor + [`PyLang`] — walk the mirror and drive the IR builder
//!   ([`lower_module`] prints `.kirin`; [`lower_to_pipeline`] returns the IR).
//! - [`run_i64`] — execute a lowered kernel through the interpreter.
//! - the PyO3 bridge — `convert` (CPython `ast` → `PyAst`) + the `_kirin_py`
//!   module exposing [`lower_source`] to the `@kernel` decorator
//!   (`python/kirin_rs/__init__.py`).

use pyo3::prelude::*;

// --- lowering core (pure Rust; no PyO3) ---
pub mod ast;
mod error;
mod interpreter;
mod language;
mod lower;
mod scope;
mod ty;

// --- PyO3 bridge ---
mod convert;

pub use error::LowerError;
pub use interpreter::{PyInterpError, run_i64};
pub use language::{PyLang, PyPipeline};
pub use lower::{lower_module, lower_to_pipeline};

/// Lower an `ast.Module` (CPython AST node) to Kirin `.kirin` IR text.
///
/// This is the plain-Rust entry point (callable from integration tests via the
/// crate's rlib); [`lower_source`] is the `#[pyfunction]` wrapper exposed to
/// Python.
pub fn lower_ast(module: &Bound<'_, PyAny>) -> PyResult<String> {
    let ast = convert::module(module)?;
    lower_module(&ast)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("lowering failed: {e}")))
}

/// Lower an `ast.Module` to Kirin `.kirin` IR text (Python-facing entry point).
#[pyfunction]
fn lower_source(module: &Bound<'_, PyAny>) -> PyResult<String> {
    lower_ast(module)
}

#[pymodule]
fn _kirin_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(lower_source, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use pyo3::prelude::*;

    /// Parity test: build a CPython AST in-process and lower it through the
    /// bridge, asserting the lowered IR matches what the core produces.
    #[test]
    fn lowers_a_parsed_function() {
        Python::attach(|py| {
            let src = "def add(a: int, b: int) -> int:\n    return a + b\n";
            let ast = py.import("ast").unwrap();
            let module = ast.call_method1("parse", (src,)).unwrap();
            let out = super::lower_source(&module).unwrap();
            assert!(
                out.contains("stage @source fn @add(i64, i64) -> i64;"),
                "{out}"
            );
            assert!(out.contains("add %a, %b -> i64"), "{out}");
        });
    }
}
