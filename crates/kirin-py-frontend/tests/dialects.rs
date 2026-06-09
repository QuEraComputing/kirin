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
