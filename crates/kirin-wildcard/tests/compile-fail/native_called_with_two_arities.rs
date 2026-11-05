//! Every native call is compiled against its function, so a call with the
//! wrong number of arguments is reported at that call, even when another
//! call passes the right number.
use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn same(_stage: &StageInfo<L>, _x: SSAValue) -> bool {
    true
}

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::add(x, y)];
    same(x);
    same(x, y);
    erase!(r)
}

fn main() {}
