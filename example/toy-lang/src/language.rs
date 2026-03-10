use kirin::prelude::*;
use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_cmp::Cmp;
use kirin_constant::Constant;
use kirin_function::{Lexical, Lifted};
use kirin_scf::StructuredControlFlow;

/// Source-stage language: structured control flow + lexical lambdas.
///
/// This now composes the built-in dialect enums directly instead of inlining
/// their `Block`/`Region`-containing operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum HighLevel {
    #[wraps]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
pub enum LowLevel {
    #[wraps]
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
