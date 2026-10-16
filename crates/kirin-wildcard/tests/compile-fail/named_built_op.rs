//! An op built in the `rewrite!` body is never the root, replaced, or
//! erased, so naming it with `@` is reported at the `@`.
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
        let n @ y = op![Arith::neg(x)];
        replace!(r, y);
    })
}

fn main() {}
