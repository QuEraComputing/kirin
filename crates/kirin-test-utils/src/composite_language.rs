use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::FunctionBody;
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[cfg_attr(feature = "pretty", derive(kirin_chumsky_derive::PrettyPrint))]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[pretty(crate = kirin_prettyless)]
#[wraps]
pub enum CompositeLanguage {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}
