//! Lowering of calls between kernels (two-pass module lowering).

mod common;

use common::call_module;
use kirin_py_frontend::lower_module;

#[test]
fn lowers_calls_between_kernels() {
    let out = lower_module(&call_module()).unwrap();
    assert!(
        out.contains("stage @source fn @helper(i64) -> i64;"),
        "missing helper decl:\n{out}"
    );
    assert!(
        out.contains("stage @source fn @main(i64) -> i64;"),
        "missing main decl:\n{out}"
    );
    assert!(out.contains("call.named @helper"), "missing call:\n{out}");
}
