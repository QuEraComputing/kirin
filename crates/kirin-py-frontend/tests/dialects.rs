//! Lower richer, per-dialect Python kernel fixtures through the full
//! CPython `ast` → PyO3 → `kirin-lower` path, then assert that the expected
//! dialect operations appear and that the lowered `.kirin` re-parses
//! (print → parse → print is stable).
//!
//! Each `kernels/<dialect>.py` fixture focuses on one dialect's lowering.

use kirin::prelude::*;
use kirin_py_frontend::PyPipeline;
use pyo3::prelude::*;

/// Parse `src` with CPython's `ast` module and lower it to `.kirin`.
fn lower(src: &str) -> String {
    Python::attach(|py| {
        let ast = py.import("ast").expect("import ast");
        let module = ast.call_method1("parse", (src,)).expect("ast.parse");
        kirin_py_frontend::lower_ast(&module).expect("lowering should succeed")
    })
}

/// The lowered IR must be valid, re-parseable Kirin.
fn assert_roundtrips(ir: &str) {
    let mut pipeline = PyPipeline::new();
    ParsePipelineText::parse(&mut pipeline, ir).expect("lowered .kirin should parse back");
    assert_eq!(ir.trim_end(), pipeline.sprint().trim_end());
}

/// The slice of `ir` for one lowered kernel: its `stage` decl plus its
/// `specialize` body, up to where the next kernel's decl begins — so an
/// assertion can target a single function rather than the whole module.
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

#[test]
fn arith_dialect() {
    let ir = lower(include_str!("kernels/arith.py"));
    for op in ["add", "sub", "mul", "div"] {
        assert!(ir.contains(&format!("{op} %")), "missing arith.{op}:\n{ir}");
    }
    assert!(ir.contains("constant "), "missing constant op:\n{ir}");
    assert_roundtrips(&ir); 
}

#[test]
fn cmp_dialect() {
    let ir = lower(include_str!("kernels/cmp.py"));
    for op in ["eq", "ne", "lt", "le", "gt", "ge"] {
        assert!(ir.contains(&format!("{op} %")), "missing cmp.{op}:\n{ir}");
    }
    // `clamp` lowers its comparisons inside nested scf.if branches.
    assert!(ir.contains("if %"), "missing scf.if for clamp:\n{ir}");
    assert_roundtrips(&ir);
}

#[test]
fn scf_dialect() {
    let ir = lower(include_str!("kernels/scf.py"));
    assert!(ir.contains("if %"), "missing scf.if:\n{ir}");
    assert!(ir.contains("for %"), "missing scf.for:\n{ir}");
    assert!(ir.contains("iter_args"), "missing iter_args:\n{ir}");
    assert!(ir.contains("yield"), "missing yield:\n{ir}");
    assert_roundtrips(&ir);
}

#[test]
fn scf_if_without_else() {
    // An `if` with no `else` over a variable defined beforehand. Through real
    // CPython parsing the missing else arrives as `orelse=[]`; the lowering must
    // still produce an scf.if *result* that merges the conditional write with
    // the fall-through prior value. Regression test for the join set — a write
    // in only one branch used to be dropped, so the result was never carried.
    let ir = lower(include_str!("kernels/scf.py"));

    // relu: `y = 0; if x > 0: y = x; return y`
    let relu = func(&ir, "relu");
    assert!(
        relu.contains("= if "),
        "relu's if must produce a result:\n{relu}"
    );
    assert_eq!(
        relu.matches("yield ").count(),
        2,
        "both branches (incl. the empty else) must yield a value:\n{relu}"
    );

    // count_positive: if-without-else updating a loop-carried accumulator —
    // the nested if produces a result that the loop body then yields.
    let cp = func(&ir, "count_positive");
    assert!(cp.contains("for %"), "missing scf.for:\n{cp}");
    assert!(
        cp.contains("= if "),
        "nested if-without-else must produce a result:\n{cp}"
    );

    assert_roundtrips(&ir);
}

#[test]
fn func_dialect() {
    let ir = lower(include_str!("kernels/func.py"));
    assert!(ir.contains("call.named @inc"), "missing call to inc:\n{ir}");
    assert!(
        ir.contains("call.named @factorial"),
        "missing recursive call:\n{ir}"
    );
    assert!(ir.contains("ret "), "missing return:\n{ir}");
    assert_roundtrips(&ir);
}
