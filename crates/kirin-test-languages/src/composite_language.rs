use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::{FunctionBody, Return};
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
#[wraps]
pub enum CompositeLanguage {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}
