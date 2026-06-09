//! Mapping from the Python type-hint subset to the `ArithType` lattice.

use crate::ast::PyType;
use kirin_arith::ArithType;

/// Map a Python type annotation to an `ArithType`.
///
/// `int`/`bool` → `i64` (the `ArithType` lattice has no dedicated bool, and
/// comparison results are `i64` by convention); `float` → `f64`; a missing
/// annotation defaults to `i64`.
pub fn map_type(t: Option<&PyType>) -> ArithType {
    match t {
        Some(PyType::Float) => ArithType::F64,
        Some(PyType::Int) | Some(PyType::Bool) | None => ArithType::I64,
    }
}
