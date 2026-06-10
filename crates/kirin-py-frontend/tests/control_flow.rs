//! Lowering of structured control flow: `if`/`else` (with comparisons) and
//! `for`-range loops with carried accumulators.

mod common;

use common::{if_without_else_module, pick_module, sum_to_module};
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
fn lowers_if_without_else_carries_variable() {
    // `if x > 0: y = 100` with no else, over a `y` defined beforehand: the join
    // must carry `y` out of the `if` even though only the then-branch assigns
    // it, so the `if` produces a result and *both* branches yield a value (the
    // then-branch the new 100, the empty else-branch the prior y).
    let out = lower_module(&if_without_else_module()).unwrap();
    assert!(
        out.contains("= if "),
        "if must produce a result (merged y):\n{out}"
    );
    assert!(out.contains("then"), "missing then branch:\n{out}");
    assert!(out.contains("else"), "missing else branch:\n{out}");
    assert!(
        out.contains("constant 100"),
        "missing then-branch write:\n{out}"
    );
    assert_eq!(
        out.matches("yield ").count(),
        2,
        "both branches must yield a carried value:\n{out}"
    );
}

#[test]
fn lowers_for_loop_with_accumulator() {
    let out = lower_module(&sum_to_module()).unwrap();
    assert!(out.contains("for "), "missing scf.for:\n{out}");
    assert!(out.contains("iter_args"), "missing iter_args:\n{out}");
    assert!(out.contains("yield"), "missing yield:\n{out}");
}
