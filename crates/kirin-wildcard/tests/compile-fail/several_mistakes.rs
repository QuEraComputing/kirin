//! Every mistake in the match is reported at once: an op that doesn't feed
//! the root, and two holes no op reads. The hole `z` is unbound only
//! because its op is unreachable, so it isn't reported again.
//!
//! The macro then emits only the errors, so the imports go unused.
#![allow(unused_imports)]

use kirin_arith::Arith;
use kirin_test_languages::GraphFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

fn is_power_of_two(x: i64) -> bool {
    x > 0 && (x & (x - 1)) == 0
}

#[rewrite_rule(language = L)]
fn rule() {
    is_power_of_two(a);
    is_power_of_two(b);
    let unused = op![Arith::neg(z)];
    let r = op![Arith::neg(x)];
    replace!(r, x)
}

fn main() {}
