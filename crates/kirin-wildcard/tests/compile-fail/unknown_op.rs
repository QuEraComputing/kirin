//! An op the dialect doesn't have: rustc finds no `Arith::new_ad`.
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::ad(x, y)];
    replace!(r, x)
}

fn main() {}
