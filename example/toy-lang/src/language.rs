use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_derive_interpreter::{Interpretable, SSACFGRegion};
use kirin_function::{Call, Function, Lexical, Lifted, Return};
use kirin_interpreter_new::FunctionEntry;
use kirin_scf::StructuredControlFlow;

/// Source-stage language: structured control flow + lexical lambdas.
///
/// `#[derive(Interpretable)]` auto-generates inner-type bounds.
/// `#[derive(SSACFGRegion)]` delegates `entry_block` to `#[callable]` variants,
/// which provides blanket `CallSemantics` via the Lexical variant.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Dialect,
    FunctionEntry,
    HasParser,
    PrettyPrint,
    Interpretable,
    SSACFGRegion,
)]
#[kirin(builders, type = ArithType)]
pub enum HighLevel {
    #[wraps]
    #[callable]
    Lexical(Lexical<ArithType>),
    #[wraps]
    Structured(StructuredControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
}

impl TryLiftFrom<Function<ArithType>> for HighLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Function<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lexical::lift_from(value)))
    }
}

impl TryLiftFrom<Call<ArithType>> for HighLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Call<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lexical::lift_from(value)))
    }
}

impl TryLiftFrom<Return<ArithType>> for HighLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Return<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lexical::lift_from(value)))
    }
}

/// Lowered-stage language: unstructured CF + lifted functions.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Dialect,
    FunctionEntry,
    HasParser,
    PrettyPrint,
    Interpretable,
    SSACFGRegion,
)]
#[kirin(builders, type = ArithType)]
pub enum LowLevel {
    #[wraps]
    #[callable]
    Lifted(Lifted<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cmp(Cmp<ArithType>),
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
}

impl TryLiftFrom<Function<ArithType>> for LowLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Function<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lifted::lift_from(value)))
    }
}

impl TryLiftFrom<Call<ArithType>> for LowLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Call<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lifted::lift_from(value)))
    }
}

impl TryLiftFrom<Return<ArithType>> for LowLevel {
    type Error = core::convert::Infallible;

    fn try_lift_from(value: Return<ArithType>) -> Result<Self, Self::Error> {
        Ok(Self::lift_from(Lifted::lift_from(value)))
    }
}
