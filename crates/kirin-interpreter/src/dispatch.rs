use kirin_ir::{Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{FunctionBody, InterpretCtx};

/// Statement semantics. The single trait dialect authors implement.
///
/// The type parameter is the context API a rule receives. Forward rules use
/// [`ForwardContext`](crate::ForwardContext); another analysis can define a
/// different context with different helpers and effect type.
pub trait Interpretable<C: InterpretCtx>: Dialect {
    fn interpret(&self, ctx: &mut C) -> Result<C::Effect, C::Error>;
}

/// Function-entry semantics for callable statements.
///
/// Implemented by statements that define function bodies (e.g.
/// `kirin_function::Function`); describes the [`FunctionBody`] an engine enters
/// when the function is invoked. Like [`Interpretable`], it is specialized on the
/// context type `C`. Derived on language enums with `#[derive(FunctionEntry)]`
/// where `#[callable]` marks the variants that wrap callable statements.
pub trait FunctionEntry<C: InterpretCtx>: Dialect {
    fn function_entry(
        &self,
        args: Product<C::Value>,
        ctx: &mut C,
    ) -> Result<FunctionBody<C::Value>, C::Error>;
}

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it.
pub trait InterpDispatch<C: InterpretCtx>: StageMeta {
    fn dispatch_statement(&self, statement: Statement, ctx: &mut C) -> Result<C::Effect, C::Error>;

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<C::Value>,
        ctx: &mut C,
    ) -> Result<FunctionBody<C::Value>, C::Error>;
}

impl<C, L> InterpDispatch<C> for StageInfo<L>
where
    C: InterpretCtx,
    L: Dialect + Interpretable<C> + FunctionEntry<C>,
{
    fn dispatch_statement(&self, statement: Statement, ctx: &mut C) -> Result<C::Effect, C::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(ctx)
    }

    fn dispatch_function_entry(
        &self,
        body: Statement,
        args: Product<C::Value>,
        ctx: &mut C,
    ) -> Result<FunctionBody<C::Value>, C::Error> {
        let definition = body.definition(self).clone();
        definition.function_entry(args, ctx)
    }
}

/// Compile-time readiness proof for a future non-forward analysis.
///
/// This stays in `kirin-interpreter` because it tests the framework seam:
/// context-specialized `Interpretable` impls can coexist, and a non-forward
/// abstract engine can implement `Interp + AbstractInterpreter` without
/// `Env`.
#[cfg(test)]
mod tests {
    use std::fmt;
    use std::marker::PhantomData;

    use kirin_ir::{CompileStage, Dialect, HasBottom, HasTop, Lattice, Statement as IrStatement};

    use crate::{
        AbstractInterpreter, EnvIndex, ForwardContext, ForwardEffect, ForwardInterp, Interp,
        InterpretCtx, Interpretable, InterpreterError,
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

    /// Distinct non-forward context type.
    struct MockBackwardContext<'a, I>(PhantomData<&'a mut I>);

    impl<I> InterpretCtx for MockBackwardContext<'_, I> {
        type Value = MockLattice;
        type Error = InterpreterError;
        type Effect = ();
    }

    impl Interp for MockBackwardInterp {
        type Value = MockLattice;
        type Error = InterpreterError;
        type Effect = ();
        type Context<'a> = MockBackwardContext<'a, Self>;

        fn context<'a>(
            &'a mut self,
            _stage: CompileStage,
            _statement: IrStatement,
            _index: EnvIndex,
        ) -> MockBackwardContext<'a, Self> {
            MockBackwardContext(PhantomData)
        }
    }

    impl AbstractInterpreter for MockBackwardInterp {}

    impl<I> Interpretable<ForwardContext<'_, I>> for MockDialect
    where
        I: ForwardInterp,
    {
        fn interpret(&self, _ctx: &mut ForwardContext<'_, I>) -> Result<I::Effect, I::Error> {
            Ok(ForwardEffect::Next)
        }
    }

    impl<I> Interpretable<MockBackwardContext<'_, I>> for MockDialect {
        fn interpret(&self, _ctx: &mut MockBackwardContext<'_, I>) -> Result<(), InterpreterError> {
            Ok(())
        }
    }

    #[test]
    fn forward_and_backward_context_impls_coexist() {
        // If this module compiles, then:
        // - forward and backward `Interpretable` impls for the same dialect do
        //   not overlap;
        // - `MockBackwardInterp` is an `Interp + AbstractInterpreter`;
        // - it does not need `Env`.
        fn assert_interp<T: Interp>() {}
        fn assert_abstract<T: AbstractInterpreter>()
        where
            T::Value: HasBottom + HasTop,
        {
        }

        assert_interp::<MockBackwardInterp>();
        assert_abstract::<MockBackwardInterp>();
        let _ = MockType;
        let _ = MockDialect::Op;
    }
}
