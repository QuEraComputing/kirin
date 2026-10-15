//! A language that doesn't contain the op's dialect has no `From` impl for
//! it.
use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_wildcard::rewrite_rule;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(type = ArithType)]
enum OnlyConstants {
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
}

#[rewrite_rule(language = OnlyConstants)]
fn rule() {
    let r = op![Arith::neg(x)];
    replace!(r, x)
}

fn main() {}
