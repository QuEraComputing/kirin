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

impl From<Function<ArithType>> for HighLevel {
    fn from(value: Function<ArithType>) -> Self {
        Self::from(Lexical::from(value))
    }
}

impl From<Call<ArithType>> for HighLevel {
    fn from(value: Call<ArithType>) -> Self {
        Self::from(Lexical::from(value))
    }
}

impl From<Return<ArithType>> for HighLevel {
    fn from(value: Return<ArithType>) -> Self {
        Self::from(Lexical::from(value))
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

impl From<Function<ArithType>> for LowLevel {
    fn from(value: Function<ArithType>) -> Self {
        Self::from(Lifted::from(value))
    }
}

impl From<Call<ArithType>> for LowLevel {
    fn from(value: Call<ArithType>) -> Self {
        Self::from(Lifted::from(value))
    }
}

impl From<Return<ArithType>> for LowLevel {
    fn from(value: Return<ArithType>) -> Self {
        Self::from(Lifted::from(value))
    }
}
