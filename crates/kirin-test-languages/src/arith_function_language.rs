use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_ir::{CFG, Dialect, Signature};

/// Test language: Function + Arith + ControlFlow + Return.
/// Used for arith pipeline roundtrips and as bare (no-namespace) language.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum ArithFunctionLanguage {
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    Function {
        body: CFG,
        sig: Signature<ArithType>,
    },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    ControlFlow(ControlFlow<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

// Manual interpreter impls: the inline `Function` variant keeps this enum off
// the `#[derive(Interpretable)]` wraps-delegation path, so the delegation is
// written out by hand.
#[cfg(feature = "interpreter")]
mod interpreter {
    use kirin_interpreter::dialect::{
        CallableBody, ClassicLiveness, ClassicLivenessInterp, DemandInterp, DenseBackwardEffect,
        FunctionEntry, Interp, Interpretable, InterpreterError, StrongDemand,
    };
    use kirin_ir::{HasBottom, Product};

    use super::ArithFunctionLanguage;

    /// Backward demand: `Function` defines a body and is inert for demand;
    /// the wrapped dialects delegate to their own backward rules.
    impl<I> Interpretable<I, StrongDemand> for ArithFunctionLanguage
    where
        I: DemandInterp,
        I::Value: HasBottom + PartialEq,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            match self {
                ArithFunctionLanguage::Function { .. } => Ok(interp.effect()),
                ArithFunctionLanguage::Arith(op) => op.interpret(interp),
                ArithFunctionLanguage::ControlFlow(op) => op.interpret(interp),
                ArithFunctionLanguage::Return(op) => op.interpret(interp),
            }
        }
    }

    /// Classic per-point liveness: `Function` has no SSA operands (inert);
    /// the wrapped dialects delegate to their own dense rules.
    impl<I> Interpretable<I, ClassicLiveness> for ArithFunctionLanguage
    where
        I: ClassicLivenessInterp,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            match self {
                ArithFunctionLanguage::Function { .. } => Ok(DenseBackwardEffect::Next),
                ArithFunctionLanguage::Arith(op) => op.interpret(interp),
                ArithFunctionLanguage::ControlFlow(op) => op.interpret(interp),
                ArithFunctionLanguage::Return(op) => op.interpret(interp),
            }
        }
    }

    impl<I: Interp> FunctionEntry<I> for ArithFunctionLanguage {
        fn function_entry(
            &self,
            args: Product<I::Value>,
            interp: &mut I,
        ) -> Result<CallableBody<I::Value>, I::Error> {
            match self {
                ArithFunctionLanguage::Function { body, .. } => {
                    Ok(CallableBody::new(*body).args(args))
                }
                _ => Err(I::Error::from(InterpreterError::NotCallable(
                    interp.statement(),
                ))),
            }
        }
    }
}
