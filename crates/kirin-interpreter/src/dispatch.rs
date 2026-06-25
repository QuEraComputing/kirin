use kirin_ir::{Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{FunctionBody, Interp};

/// Statement semantics. The single trait dialect authors implement.
///
/// The engine type `I` is the object a rule receives directly; the `Kind`
/// parameter is a compile-time semantics marker (e.g.
/// [`ForwardEval`](crate::ForwardEval)) that selects *which* semantics this
/// impl describes. The same dialect type can carry one impl per `Kind` — e.g. a
/// forward-value rule and a future backward-liveness rule — without coherence
/// conflicts.
pub trait Interpretable<I: Interp, Kind>: Dialect {
    fn interpret(&self, interp: &mut I) -> Result<I::Effect, I::Error>;
}

/// Function-entry semantics for callable statements.
///
/// Implemented by statements that define function bodies (e.g.
/// `kirin_function::Function`); describes the [`FunctionBody`] an engine enters
/// when the function is invoked. Derived on language enums with
/// `#[derive(FunctionEntry)]` where `#[callable]` marks the variants that wrap
/// callable statements.
pub trait FunctionEntry<I: Interp>: Dialect {
    fn function_entry(
        &self,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error>;
}

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it. Keyed on the engine `I` and the
/// semantics `Kind`.
pub trait InterpDispatch<I: Interp, Kind>: StageMeta {
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error>;

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error>;
}

impl<I, Kind, L> InterpDispatch<I, Kind> for StageInfo<L>
where
    I: Interp,
    L: Dialect + Interpretable<I, Kind> + FunctionEntry<I>,
{
    fn dispatch_statement(
        &self,
        statement: Statement,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(interp)
    }

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<I::Value>,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        let definition = body.definition(self).clone();
        definition.function_entry(args, interp)
    }
}

/// Compile-time readiness proof for a future non-forward analysis.
///
/// This stays in `kirin-interpreter` because it tests the framework seam:
/// `Kind`-specialized `Interpretable` impls can coexist for one dialect
/// ([`ForwardEval`](crate::ForwardEval) and
/// [`BackwardLiveness`](crate::BackwardLiveness)), and a non-forward abstract
/// engine can implement `Interp + AbstractInterpreter` without `Env`.
#[cfg(test)]
mod tests {
    use std::fmt;

    use kirin_ir::{CompileStage, Dialect, HasBottom, HasTop, Lattice, Statement};

    use crate::{
        AbstractInterpreter, BackwardLiveness, EnvIndex, ForwardEffect, ForwardEval,
        ForwardEvalInterp, Interp, Interpretable, InterpreterError,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct MockType;

    impl fmt::Display for MockType {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("mock")
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Dialect)]
    #[kirin(type = MockType, crate = kirin_ir)]
    enum MockDialect {
        Op,
    }

    /// Tiny one-element lattice for the mock abstract engine.
    #[derive(Clone)]
    struct MockLattice;

    impl Lattice for MockLattice {
        fn join(&self, _other: &Self) -> Self {
            Self
        }

        fn meet(&self, _other: &Self) -> Self {
            Self
        }

        fn is_subseteq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl HasBottom for MockLattice {
        fn bottom() -> Self {
            Self
        }
    }

    impl HasTop for MockLattice {
        fn top() -> Self {
            Self
        }
    }

    /// Mock abstract engine that is not a forward engine.
    struct MockBackwardInterp;

    impl Interp for MockBackwardInterp {
        type Value = MockLattice;
        type Error = InterpreterError;
        type Effect = ();
        type Kind = BackwardLiveness;

        fn stage(&self) -> CompileStage {
            unimplemented!("mock engine: location is never read")
        }

        fn statement(&self) -> Statement {
            unimplemented!("mock engine: location is never read")
        }

        fn index(&self) -> EnvIndex {
            unimplemented!("mock engine: location is never read")
        }
    }

    impl AbstractInterpreter for MockBackwardInterp {}

    // A forward-value rule for the dialect, written against the generic forward
    // engine surface.
    impl<I> Interpretable<I, ForwardEval> for MockDialect
    where
        I: ForwardEvalInterp,
    {
        fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
            Ok(ForwardEffect::Next)
        }
    }

    // A backward-liveness rule for the *same* dialect, distinguished only by the
    // `Kind` marker — proving the two coexist without coherence conflicts.
    impl<I> Interpretable<I, BackwardLiveness> for MockDialect
    where
        I: Interp<Kind = BackwardLiveness, Effect = ()>,
    {
        fn interpret(&self, _interp: &mut I) -> Result<I::Effect, I::Error> {
            Ok(())
        }
    }

    #[test]
    fn forward_and_backward_kind_impls_coexist() {
        // If this module compiles, then:
        // - forward and backward `Interpretable` impls for the same dialect do
        //   not overlap (distinguished by the `Kind` marker — the two impl
        //   blocks above pass coherence together);
        // - `MockBackwardInterp` is an `Interp + AbstractInterpreter`;
        // - it does not need `Env`.
        fn assert_interp<T: Interp>() {}
        fn assert_abstract<T: AbstractInterpreter>() {}
        // The backward rule resolves for any backward engine; instantiating it
        // with the mock engine proves the `BackwardLiveness` impl is reachable.
        // The forward rule's coexistence is proven by the two impl blocks above
        // passing coherence together.
        fn assert_backward_rule<I: Interp<Kind = BackwardLiveness, Effect = ()>>()
        where
            MockDialect: Interpretable<I, BackwardLiveness>,
        {
        }

        assert_interp::<MockBackwardInterp>();
        assert_abstract::<MockBackwardInterp>();
        assert_backward_rule::<MockBackwardInterp>();
        let _ = MockType;
        let _ = MockDialect::Op;
    }
}
