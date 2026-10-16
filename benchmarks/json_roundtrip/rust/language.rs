use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_test_languages::SimpleType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint, Serialize, Deserialize)]
#[kirin(builders, type = SimpleType)]
pub enum BenchmarkLanguage {
    #[chumsky(format = "fn {:name}{signature} {body}")]
    Function {
        #[kirin(callable_body)]
        body: CFG,
        signature: Signature<SimpleType>,
    },
    #[wraps]
    Arith(Arith<SimpleType>),
    #[wraps]
    Cf(ControlFlow<SimpleType>),
    #[wraps]
    Return(Return<SimpleType>),
}

use kirin_interpreter::{ForwardEval, Interpretable, SparseForwardEffect, SparseForwardInterp};

impl<I> Interpretable<I, ForwardEval> for BenchmarkLanguage
where
    I: SparseForwardInterp,
    Arith<SimpleType>: Interpretable<I, ForwardEval>,
    ControlFlow<SimpleType>: Interpretable<I, ForwardEval>,
    Return<SimpleType>: Interpretable<I, ForwardEval>,
{
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
        match self {
            Self::Function { .. } => Ok(SparseForwardEffect::Next),
            Self::Arith(op) => op.interpret(interp),
            Self::Cf(op) => op.interpret(interp),
            Self::Return(op) => op.interpret(interp),
        }
    }
}
