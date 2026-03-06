use kirin_arith::ArithType;
use kirin_bitwise::Bitwise;
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{Dialect, Region};

/// Test language: Function + Bitwise + ControlFlow + Return.
/// Used for bitwise pipeline roundtrip tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum BitwiseFunctionLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "{body}")
    )]
    Function { body: Region },
    #[wraps]
    Bitwise(Bitwise<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
