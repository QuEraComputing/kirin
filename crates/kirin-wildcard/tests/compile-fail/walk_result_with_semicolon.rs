//! The walk result is the tail expression of the `rewrite!` body, so
//! `skip!()` followed by a semicolon is reported at the macro.
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
        replace!(r, x);
        skip!();
    })
}

fn main() {}
