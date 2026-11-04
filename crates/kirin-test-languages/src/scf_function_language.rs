use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{CFG, Dialect, Signature};
use kirin_scf::StructuredControlFlow;

/// Test language: Function + Arith + ControlFlow + StructuredControlFlow +
/// Return. Used where a statement nested in a function body owns blocks of
/// its own, such as `scf.if`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum ScfFunctionLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    Function {
        #[kirin(callable_body)]
        body: CFG,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Scf(StructuredControlFlow<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}
