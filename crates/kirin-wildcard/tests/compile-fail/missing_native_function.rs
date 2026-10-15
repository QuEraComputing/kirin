//! A constraint calling a function that doesn't exist.
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    is_power_of_two(x);
    replace!(r, x)
}

fn main() {}
