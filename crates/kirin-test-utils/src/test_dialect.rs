use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::FunctionBody;
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
pub enum TestDialect {
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}
