//! Lowering of structured control flow: `if`/`else` (with comparisons) and
//! `for`-range loops with carried accumulators.

mod common;

use common::{pick_module, sum_to_module};
use kirin_py_frontend::lower_module;

#[test]
fn lowers_if_else_with_comparison() {
    let out = lower_module(&pick_module()).unwrap();
    assert!(out.contains("gt %c"), "missing comparison:\n{out}");
    assert!(out.contains("if "), "missing scf.if:\n{out}");
    assert!(out.contains("then"), "missing then branch:\n{out}");
    assert!(out.contains("else"), "missing else branch:\n{out}");
    assert!(out.contains("yield"), "missing yield:\n{out}");
}

#[test]
fn lowers_for_loop_with_accumulator() {
    let out = lower_module(&sum_to_module()).unwrap();
    assert!(out.contains("for "), "missing scf.for:\n{out}");
    assert!(out.contains("iter_args"), "missing iter_args:\n{out}");
    assert!(out.contains("yield"), "missing yield:\n{out}");
}
