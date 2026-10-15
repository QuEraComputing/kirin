//! The attribute must name the language the pattern is written in.
//!
//! The macro then emits only the error, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule]
fn rule() {
    let r = op![Arith::neg(x)];
    replace!(r, x)
}

fn main() {}
