//! A walk result must come from a native function in the `rewrite!` body,
//! which runs only once the rule applies, so one computed in the match is
//! reported at its name.
//!
//! The macro then emits only the error, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    let w = decide(x);
    rewrite!(r, {
        replace!(r, x);
        w
    })
}

fn main() {}
