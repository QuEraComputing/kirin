//! `?` on a constraint needs the function to return an `Option<bool>` or a
//! `Result<bool, _>`: the error points at the call.
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn is_block_argument(_stage: &StageInfo<L>, _x: SSAValue) -> bool {
    true
}

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    is_block_argument(x)?;
    replace!(r, x)
}

fn main() {}
