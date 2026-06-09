//! Semantic tests: lower a kernel to IR and *run* it through the interpreter,
//! asserting the computed result. This goes beyond the roundtrip tests (which
//! only prove the IR is well-formed) — here we prove the translation actually
//! computes the right answer.

mod common;

use common::{add_module, call_module, factorial_module, pick_module, sum_to_module};
use kirin_py_frontend::{lower_to_pipeline, run_i64};

/// Lower `module`, then run `function`(`args`) and return its `i64` result.
fn run(module: kirin_py_frontend::ast::Module, function: &str, args: &[i64]) -> i64 {
    let pipeline = lower_to_pipeline(&module).expect("lowering should succeed");
    run_i64(&pipeline, function, args).expect("execution should succeed")
}

#[test]
fn executes_arithmetic() {
    assert_eq!(run(add_module(), "add", &[3, 5]), 8);
}

#[test]
fn executes_if_else_both_branches() {
    // pick(c, a, b) = a + b if c > 0 else a - b
    assert_eq!(run(pick_module(), "pick", &[1, 5, 3]), 8); // then-branch
    assert_eq!(run(pick_module(), "pick", &[-1, 5, 3]), 2); // else-branch
}

#[test]
fn executes_for_loop_accumulator() {
    // sum_to(n) = 0 + 1 + ... + (n-1)
    assert_eq!(run(sum_to_module(), "sum_to", &[5]), 10);
    assert_eq!(run(sum_to_module(), "sum_to", &[0]), 0);
    assert_eq!(run(sum_to_module(), "sum_to", &[10]), 45);
}

#[test]
fn executes_cross_kernel_call() {
    // main(y) = helper(y) = y + y
    assert_eq!(run(call_module(), "main", &[5]), 10);
    assert_eq!(run(call_module(), "main", &[21]), 42);
}

#[test]
fn executes_recursion() {
    // factorial(n) via if/else value-merge + recursive call
    assert_eq!(run(factorial_module(), "factorial", &[1]), 1);
    assert_eq!(run(factorial_module(), "factorial", &[5]), 120);
}
