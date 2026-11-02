//! `?` on a constraint would give a second way to fail the match, besides
//! the function returning `false`.
//!
//! The macro then emits only the error, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn rule() {
    let r = op![Arith::neg(x)];
    is_block_argument(x)?;
    replace!(r, x)
}

fn main() {}
