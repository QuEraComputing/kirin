//! A generic rule only builds for a language containing every dialect its
//! bounds name.
use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_wildcard::{PatternLanguage, rewrite_rule};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(type = ArithType)]
enum OnlyConstants {
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
}

#[rewrite_rule(language = L)]
fn double_neg<L>()
where
    L: From<Arith<L::Type>>,
{
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    replace!(r, x)
}

fn main() {
    let mut stage = BuilderStageInfo::<PatternLanguage<OnlyConstants>>::default();
    double_neg(&mut stage);
}
