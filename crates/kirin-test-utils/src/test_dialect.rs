use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_derive_interpreter::{CallSemantics, Interpretable};
use kirin_function::FunctionBody;
use kirin_ir::*;
use kirin_prettyless::{ArenaDoc, Document, PrettyPrint};

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

impl PrettyPrint for TestDialect {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            TestDialect::Arith(inner) => inner.pretty_print(doc),
            TestDialect::ControlFlow(inner) => inner.pretty_print(doc),
            TestDialect::Constant(inner) => inner.pretty_print(doc),
            TestDialect::FunctionBody(inner) => inner.pretty_print(doc),
        }
    }
}
