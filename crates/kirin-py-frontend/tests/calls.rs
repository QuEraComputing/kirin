//! Lowering of calls between kernels (two-pass module lowering).
//!
//! Beyond "a call appears", these tests pin down the *body* the lowering emits:
//! the SSA data flow, operand evaluation order, and how a call's result is (or
//! isn't) consumed. SSA result ids are allocation-dependent, so we never
//! hard-code them — we read the id a line *defines* and assert how later lines
//! *use* it. (Exact textual stability is covered separately by the roundtrip
//! test over `all_modules()`.)

mod common;

use common::{
    call_module, combine_call_module, discarded_call_module, factorial_module, nested_call_module,
};
use kirin_py_frontend::lower_module;

/// The slice of `ir` for one function: its `stage` decl plus its `specialize`
/// body, up to where the next function's decl begins. Lets each assertion scope
/// to a single kernel rather than matching ops in a sibling function.
fn func<'a>(ir: &'a str, name: &str) -> &'a str {
    let marker = format!("stage @source fn @{name}(");
    let start = ir
        .find(&marker)
        .unwrap_or_else(|| panic!("no declaration for @{name}:\n{ir}"));
    let after = start + marker.len();
    let end = ir[after..]
        .find("stage @source fn @")
        .map_or(ir.len(), |i| after + i);
    &ir[start..end]
}

/// The `%id` defined by the first line in `ir` containing `needle`
/// (lines have the form `%id = <op> ...;`). Panics if there is no such line or
/// it defines no `%result`.
fn defines<'a>(ir: &'a str, needle: &str) -> &'a str {
    let line = ir
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?}:\n{ir}"));
    line.trim_start()
        .split_whitespace()
        .next()
        .filter(|t| t.starts_with('%'))
        .unwrap_or_else(|| panic!("line defines no %result: {line:?}"))
}

#[test]
fn lowers_calls_between_kernels() {
    let out = lower_module(&call_module()).unwrap();

    // Pass 1 declares both kernels so calls can resolve by name.
    assert!(
        out.contains("stage @source fn @helper(i64) -> i64;"),
        "missing helper decl:\n{out}"
    );
    assert!(
        out.contains("stage @source fn @main(i64) -> i64;"),
        "missing main decl:\n{out}"
    );

    // helper(x): doubles its argument and returns that value.
    let helper = func(&out, "helper");
    assert!(
        helper.contains("^entry(%x: i64)"),
        "helper should bind %x:\n{helper}"
    );
    let doubled = defines(helper, "add %x, %x -> i64");
    assert!(
        helper.contains(&format!("ret {doubled};")),
        "helper should return its add result {doubled}:\n{helper}"
    );

    // main(y): calls @helper with its block argument and returns the result.
    let main = func(&out, "main");
    let call = defines(main, "call.named @helper (%y) -> i64");
    assert!(
        main.contains(&format!("ret {call};")),
        "main should return the call result {call}:\n{main}"
    );
}

#[test]
fn lowers_nested_call_argument() {
    // `return inc(inc(y))`: the inner call's result must thread straight into
    // the outer call's operand list — a call result used as a call operand.
    let out = lower_module(&nested_call_module()).unwrap();
    let main = func(&out, "main");

    let inner = defines(main, "call.named @inc (%y) -> i64");
    let outer = defines(main, &format!("call.named @inc ({inner}) -> i64"));
    assert_ne!(inner, outer, "the two calls must produce distinct results");
    assert!(
        main.contains(&format!("ret {outer};")),
        "main should return the outer call result {outer}:\n{main}"
    );
}

#[test]
fn lowers_call_with_computed_args_and_consumed_result() {
    // `z = combine(x + 1, x - 1); return z + z`: arguments are compound
    // expressions (lowered to temps *before* the call) and the call's result is
    // consumed by later arithmetic.
    let out = lower_module(&combine_call_module()).unwrap();

    let combine = func(&out, "combine");
    assert!(
        combine.contains("mul %a, %b -> i64"),
        "combine should multiply its args:\n{combine}"
    );

    let main = func(&out, "main");
    // Each operand is computed into its own temp ahead of the call.
    let lhs = defines(main, "add %x,");
    let rhs = defines(main, "sub %x,");
    let call = defines(main, &format!("call.named @combine ({lhs}, {rhs}) -> i64"));
    // The call result then feeds `z + z` and is returned.
    let sum = defines(main, &format!("add {call}, {call} -> i64"));
    assert!(
        main.contains(&format!("ret {sum};")),
        "main should return {sum} (z + z):\n{main}"
    );
}

#[test]
fn lowers_discarded_call_result() {
    // Bare-expression call: `effect(y)` is emitted, but `main` returns its own
    // argument — the call's result is produced yet never consumed.
    let out = lower_module(&discarded_call_module()).unwrap();
    let main = func(&out, "main");

    let call = defines(main, "call.named @effect (%y) -> i64");
    assert!(
        main.contains("ret %y;"),
        "main should return its block argument %y:\n{main}"
    );
    assert!(
        !main.contains(&format!("ret {call};")),
        "the call result {call} must be discarded, not returned:\n{main}"
    );
}

#[test]
fn lowers_recursive_self_call() {
    // factorial calls *itself*: the pass-1 declaration lets the name resolve
    // inside its own body — here within the `else` branch of an scf.if, with the
    // recursive result feeding `n * factorial(n - 1)`.
    let out = lower_module(&factorial_module()).unwrap();
    let factorial = func(&out, "factorial");

    assert!(factorial.contains("if "), "missing scf.if:\n{factorial}");
    let rec = defines(factorial, "call.named @factorial (");
    assert!(
        factorial.contains(&format!("mul %n, {rec} -> i64")),
        "recursive result {rec} should feed `n * factorial(n-1)`:\n{factorial}"
    );
}
