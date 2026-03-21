use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{Dialect, Region, Signature};

/// Test language: Function + Arith + ControlFlow + Return.
/// Used for arith pipeline roundtrips and as bare (no-namespace) language.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum ArithFunctionLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    Function {
        body: Region,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
