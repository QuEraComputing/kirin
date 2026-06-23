use kirin_ir::{CompileStage, Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{EnvIndex, ForwardContext, FunctionBody, Interp, InterpretCtx};

/// Statement semantics. The single trait dialect authors implement.
///
/// Generic over the **context type** `C` (an [`InterpretCtx`]), *not* the engine
/// type — that is the specialization boundary that keeps analyses disjoint. A
/// rule produces `C::Effect` (the analysis-specific effect algebra) through the
/// context's helpers. Forward rules implement `Interpretable<ForwardContext<'_, I>>`,
/// reading/writing through [`ForwardContext`]'s [`ForwardCtx`](crate::ForwardCtx)
/// helpers and returning [`ForwardEffect`](crate::ForwardEffect); they constrain
/// only the value domain (`I::Value: Add + ...`) and error lifting (`I::Error:
/// From<MyError>`), and the *same* impl drives concrete execution and forward
/// abstract interpretation, the difference living in the value type.
///
/// A future analysis implements `Interpretable<TheirContext<'_, I>>` for its own
/// distinct context type — because `ForwardContext` and `TheirContext` are
/// different type constructors, the impls do not overlap (no `E0119`), even though
/// both are generic over the engine `I`.
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
///
/// `InterpDispatch` stays parameterized by the **engine** `I` (it is the
/// engine/compiler-author seam); it constructs the forward [`ForwardContext`] and
/// calls the context-specialized [`Interpretable`]/[`FunctionEntry`] rules.
pub trait InterpDispatch<I: Interp>: StageMeta {
    fn dispatch_statement(
        &self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error>;

    fn dispatch_function_entry(
        &self,
        stage: CompileStage,
        body: Statement,
        args: Product<I::Value>,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error>;
}

impl<I, L> InterpDispatch<I> for StageInfo<L>
where
    I: Interp,
    L: Dialect
        + for<'a> Interpretable<ForwardContext<'a, I>>
        + for<'a> FunctionEntry<ForwardContext<'a, I>>,
{
    fn dispatch_statement(
        &self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<I::Effect, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(&mut ForwardContext::new(interp, stage, statement, env))
    }

    fn dispatch_function_entry(
        &self,
        stage: CompileStage,
        body: Statement,
        args: Product<I::Value>,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<FunctionBody<I::Value>, I::Error> {
        let definition = body.definition(self).clone();
        definition.function_entry(args, &mut ForwardContext::new(interp, stage, body, env))
    }
}
