use kirin_arith::{Arith, ArithType};
use kirin_ir::{Dialect, SSAValue};

/// Test language: Arith + an `effect` that consumes a value and has no results.
///
/// `effect` is a statement without results that is not a terminator. Used to
/// test rewrite rules that erase such a statement or are rooted at one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
pub enum EffectLanguage {
    #[wraps]
    Arith(Arith<ArithType>),
    Effect {
        value: SSAValue,
    },
}
