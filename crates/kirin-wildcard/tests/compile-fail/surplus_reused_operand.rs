//! A surplus operand is reported where the rule writes it: here the second
//! `x` in `neg(a, x)`, not the hole's first use in `neg(x)` above.
use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a, x)];
    replace!(r, x)
}

fn main() {}
