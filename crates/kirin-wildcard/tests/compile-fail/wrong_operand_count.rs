//! `neg` takes one operand, so its constructor `new_neg` takes two
//! arguments: the operand and the result.
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x, y)];
    replace!(r, x)
}

fn main() {}
