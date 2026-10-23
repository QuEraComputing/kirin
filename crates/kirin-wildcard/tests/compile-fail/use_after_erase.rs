//! After `erase!` removes an op, its results are gone too, so redirecting
//! `r` afterwards is reported at `r`.
//!
//! The macro then emits only the error, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    rewrite!(r, {
        erase!(r);
        replace!(r, x);
    })
}

fn main() {}
