//! A rule registers one function per name, so it cannot call one function
//! both with and without `?`.
//!
//! The macro then emits only the error, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    let a = argument_index(x)?;
    let b = argument_index(x);
    replace!(r, x)
}

fn main() {}
