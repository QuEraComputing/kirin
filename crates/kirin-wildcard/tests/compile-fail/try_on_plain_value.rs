//! `?` in the match needs the function to return an `Option`: the error
//! points at the call.
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn argument_index(_stage: &StageInfo<L>, _x: SSAValue) -> i64 {
    0
}

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    let index = argument_index(x)?;
    replace!(r, x)
}

fn main() {}
