//! A generic rule must bound its language by every dialect it names: the
//! macro cannot write these bounds, since it does not know the dialects'
//! type parameters.
use kirin_arith::Arith;
use kirin_wildcard::rewrite_rule;

#[rewrite_rule(language = L)]
fn double_neg<L>() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    replace!(r, x)
}

fn main() {}
