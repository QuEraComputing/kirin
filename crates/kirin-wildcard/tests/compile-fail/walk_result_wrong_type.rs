//! The native function computing a rewrite body's walk result must return a
//! `WalkResult`; one returning anything else is reported at its call.

use kirin::ir::{Rewriter, SSAValue};
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn decide(_rewriter: &mut Rewriter<'_, L>, _x: SSAValue) -> bool {
    true
}

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    rewrite!(r, {
        replace!(r, x);
        decide(x)
    })
}

fn main() {}
