use kirin_arith::{Arith, ArithType};
use kirin_function::Lexical;
use kirin_ir::Dialect;

/// Arithmetic and functions without requiring comparison or branch capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "interpreter", derive(kirin_interpreter::Interpretable))]
#[kirin(type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
pub enum ScalarFunctionLanguage {
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Function(Lexical<ArithType>),
}
