use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::FunctionBody;
use kirin_ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, Interpretable, CallSemantics)]
#[kirin(fn, type = ArithType, crate = kirin_ir)]
#[wraps]
pub enum CompositeLanguage {
    Arith(Arith<ArithType>),
    #[kirin(terminator)]
    ControlFlow(ControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    #[callable]
    FunctionBody(FunctionBody<ArithType>),
}

#[cfg(feature = "pretty")]
impl kirin_prettyless::PrettyPrint for CompositeLanguage {
    fn pretty_print<'a, L: Dialect + kirin_prettyless::PrettyPrint>(
        &self,
        doc: &'a kirin_prettyless::Document<'a, L>,
    ) -> kirin_prettyless::ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            CompositeLanguage::Arith(inner) => inner.pretty_print(doc),
            CompositeLanguage::ControlFlow(inner) => inner.pretty_print(doc),
            CompositeLanguage::Constant(inner) => inner.pretty_print(doc),
            CompositeLanguage::FunctionBody(inner) => inner.pretty_print(doc),
        }
    }
}
