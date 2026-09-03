//! Mixed graph/SSA test language: regular SSA IR (arith + cf + function
//! calls over `CFG` bodies) combined with `DiGraph` computational-graph
//! bodies — the acceptance shape from issue #667. Adds a linear (`Block`-
//! bodied) callable and an inline graph-owning statement so both interpreter
//! entry paths (call and `Push`) are exercised.

use kirin_arith::{Arith, ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::{Call, Return};
use kirin_ir::{
    Block, CFG, DiGraph, Dialect, Placeholder as _, ResultValue, SSAValue, Signature, UnGraph,
};

#[derive(Debug, Clone, PartialEq, Dialect)]
#[cfg_attr(feature = "parser", derive(kirin_chumsky::HasParser))]
#[cfg_attr(feature = "pretty", derive(kirin_derive_chumsky::PrettyPrint))]
#[kirin(builders, type = ArithType, crate = kirin_ir)]
#[cfg_attr(feature = "parser", chumsky(crate = kirin_chumsky))]
#[cfg_attr(feature = "pretty", pretty(crate = kirin_prettyless))]
pub enum GraphFunctionLanguage {
    /// Standard CFG-bodied function.
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    Function {
        body: CFG,
        sig: Signature<ArithType>,
    },
    /// DiGraph-bodied callable (a computational graph as a function body).
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    GraphFunction {
        body: DiGraph,
        sig: Signature<ArithType>,
    },
    /// Linear (single-`Block`) callable: a flat instruction list.
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    LinearFunction {
        body: Block,
        sig: Signature<ArithType>,
    },
    /// UnGraph-bodied callable. The framework has no default walker for an
    /// undirected graph body: calling one requires the compiler to supply a
    /// custom `CallBodyTraversal`, otherwise the engine reports
    /// `NoDefaultWalker`.
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "fn {:name}{sig} {body}")
    )]
    UnGraphFunction {
        body: UnGraph,
        sig: Signature<ArithType>,
    },
    /// Inline graph evaluation: enters its owned digraph via a pushed frame.
    #[cfg_attr(
        any(feature = "parser", feature = "pretty"),
        chumsky(format = "$graph_eval {lhs}, {rhs} {graph} -> {result:type}")
    )]
    GraphEval {
        lhs: SSAValue,
        rhs: SSAValue,
        graph: DiGraph,
        result: ResultValue,
    },
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
    #[wraps]
    Constant(Constant<ArithValue, ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

// Manual interpreter impls: the inline function/graph variants keep this
// enum off the `#[derive(Interpretable)]` wraps-delegation path.
#[cfg(feature = "interpreter")]
mod interpreter {
    use kirin_arith::{ArithValue, CheckedDiv, CheckedRem, interpreter::DivisionByZero};
    use kirin_interpreter::BranchCondition;
    use kirin_interpreter::{
        CallableBody, ClassicLiveness, ClassicLivenessInterp, DemandInterp, DenseBackwardEffect,
        DiGraphFrame, ForwardEval, FunctionEntry, Interpretable, SparseForwardEffect,
        SparseForwardInterp, StrongDemand,
    };
    use kirin_ir::{HasBottom, Product, SSAValue};

    use super::GraphFunctionLanguage;

    impl<I> Interpretable<I, ForwardEval> for GraphFunctionLanguage
    where
        I: SparseForwardInterp,
        I::Frame: From<DiGraphFrame<I::Value, I::Error>>,
        I::Value: std::ops::Add<Output = I::Value>
            + std::ops::Sub<Output = I::Value>
            + std::ops::Mul<Output = I::Value>
            + std::ops::Neg<Output = I::Value>
            + CheckedDiv
            + CheckedRem
            + BranchCondition
            + TryFrom<ArithValue>,
        I::Error: From<DivisionByZero> + From<<I::Value as TryFrom<ArithValue>>::Error>,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            match self {
                GraphFunctionLanguage::Function { .. }
                | GraphFunctionLanguage::GraphFunction { .. }
                | GraphFunctionLanguage::LinearFunction { .. }
                | GraphFunctionLanguage::UnGraphFunction { .. } => Ok(SparseForwardEffect::Next),
                GraphFunctionLanguage::GraphEval {
                    lhs,
                    rhs,
                    graph,
                    result,
                } => {
                    let args: Product<I::Value> = [interp.read(*lhs)?, interp.read(*rhs)?]
                        .into_iter()
                        .collect();
                    // A nested (uncallable) graph body: no function activation,
                    // no callee resolution — the operation pushes the walker
                    // directly into the current activation.
                    let frame = DiGraphFrame::new(interp.stage(), interp.index(), *graph, args);
                    Ok(SparseForwardEffect::Push {
                        frame: frame.into(),
                        results: [SSAValue::from(*result)].into_iter().collect(),
                    })
                }
                GraphFunctionLanguage::Arith(op) => op.interpret(interp),
                GraphFunctionLanguage::Cf(op) => op.interpret(interp),
                GraphFunctionLanguage::Constant(op) => op.interpret(interp),
                GraphFunctionLanguage::Call(op) => op.interpret(interp),
                GraphFunctionLanguage::Return(op) => op.interpret(interp),
            }
        }
    }

    impl<I> Interpretable<I, StrongDemand> for GraphFunctionLanguage
    where
        I: DemandInterp,
        I::Value: HasBottom + PartialEq,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            match self {
                GraphFunctionLanguage::Function { .. }
                | GraphFunctionLanguage::GraphFunction { .. }
                | GraphFunctionLanguage::LinearFunction { .. }
                | GraphFunctionLanguage::UnGraphFunction { .. } => Ok(interp.effect()),
                GraphFunctionLanguage::GraphEval { .. } => interp.demand_uses_if_observable(self),
                GraphFunctionLanguage::Arith(op) => op.interpret(interp),
                GraphFunctionLanguage::Cf(op) => op.interpret(interp),
                GraphFunctionLanguage::Constant(op) => op.interpret(interp),
                GraphFunctionLanguage::Call(op) => op.interpret(interp),
                GraphFunctionLanguage::Return(op) => op.interpret(interp),
            }
        }
    }

    impl<I> Interpretable<I, ClassicLiveness> for GraphFunctionLanguage
    where
        I: ClassicLivenessInterp,
    {
        fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error> {
            match self {
                GraphFunctionLanguage::Function { .. }
                | GraphFunctionLanguage::GraphFunction { .. }
                | GraphFunctionLanguage::LinearFunction { .. }
                | GraphFunctionLanguage::UnGraphFunction { .. } => Ok(DenseBackwardEffect::Next),
                GraphFunctionLanguage::GraphEval { .. } => interp.gen_uses_kill_defs(self),
                GraphFunctionLanguage::Arith(op) => op.interpret(interp),
                GraphFunctionLanguage::Cf(op) => op.interpret(interp),
                GraphFunctionLanguage::Constant(op) => op.interpret(interp),
                GraphFunctionLanguage::Call(op) => op.interpret(interp),
                GraphFunctionLanguage::Return(op) => op.interpret(interp),
            }
        }
    }

    impl FunctionEntry for GraphFunctionLanguage {
        fn function_entry(&self) -> Option<CallableBody> {
            match self {
                GraphFunctionLanguage::Function { body, .. } => Some(CallableBody::new(*body)),
                GraphFunctionLanguage::GraphFunction { body, .. } => Some(CallableBody::new(*body)),
                GraphFunctionLanguage::LinearFunction { body, .. } => {
                    Some(CallableBody::new(*body))
                }
                GraphFunctionLanguage::UnGraphFunction { body, .. } => {
                    Some(CallableBody::new(*body))
                }
                _ => None,
            }
        }
    }
}
