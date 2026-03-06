use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{Dialect, Region};

/// Test language with namespace prefixes on wraps variants.
/// Arith ops become `arith.add`, ControlFlow becomes `cf.br`, Return becomes `func.ret`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum NamespacedLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "{body}")
    )]
    Function { body: Region },
    #[wraps]
    #[cfg_attr(any(feature = "parser", feature = "pretty"), chumsky(format = "arith"))]
    Arith(Arith<ArithType>),
    #[wraps]
    #[cfg_attr(any(feature = "parser", feature = "pretty"), chumsky(format = "cf"))]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    #[cfg_attr(any(feature = "parser", feature = "pretty"), chumsky(format = "func"))]
    Return(Return<ArithType>),
}
