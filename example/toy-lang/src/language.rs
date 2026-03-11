use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_derive_interpreter::{Interpretable, SSACFGRegion};
use kirin_function::{Lexical, Lifted};
use kirin_scf::StructuredControlFlow;

/// Source-stage language: structured control flow + lexical lambdas.
///
/// `#[derive(Interpretable)]` auto-generates inner-type bounds.
/// `#[derive(SSACFGRegion)]` delegates `entry_block` to `#[callable]` variants,
/// which provides blanket `CallSemantics` via the Lexical variant.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable, SSACFGRegion,
)]
#[kirin(fn, type = ArithType)]
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

/// Lowered-stage language: unstructured CF + lifted functions.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint, Interpretable, SSACFGRegion,
)]
#[kirin(fn, type = ArithType)]
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
