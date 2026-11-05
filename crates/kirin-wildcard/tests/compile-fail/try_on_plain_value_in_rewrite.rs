//! `?` in the `rewrite!` body needs the function to return an `Option` or a
//! `Result`: the error points at the call.
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn value(_rewriter: &mut Rewriter<'_, L>, x: SSAValue) -> SSAValue {
    x
}

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    rewrite!(r, {
        let y = value(x)?;
        replace!(r, y);
    })
}

fn main() {}
