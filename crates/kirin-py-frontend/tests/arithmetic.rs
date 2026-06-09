//! Lowering of functions, arithmetic, assignment, return and type hints.

mod common;

use common::{add_module, arith_chain_module};
use kirin_py_frontend::lower_module;

#[test]
fn lowers_add() {
    let out = lower_module(&add_module()).unwrap();
    assert!(
        out.contains("stage @source fn @add(i64, i64) -> i64;"),
        "missing staged decl:\n{out}"
    );
    assert!(out.contains("add %a, %b -> i64"), "missing add op:\n{out}");
    assert!(out.contains("ret"), "missing return:\n{out}");
}

#[test]
fn lowers_all_arith_ops() {
    let out = lower_module(&arith_chain_module()).unwrap();
    for op in ["add", "sub", "mul", "div"] {
        assert!(out.contains(&format!("{op} ")), "missing {op}:\n{out}");
    }
}
