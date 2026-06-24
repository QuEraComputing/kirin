use kirin_ir::{Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{FunctionBody, InterpretCtx};

/// Statement semantics. The single trait dialect authors implement.
///
/// Generic over the **context type** `C` (an [`InterpretCtx`]), *not* the engine
/// type — that is the specialization boundary that keeps analyses disjoint. A
/// rule produces `C::Effect` (the analysis-specific effect algebra) through the
/// context's helpers. Forward rules implement `Interpretable<ForwardContext<'_, I>>`,
/// reading/writing through [`ForwardContext`](crate::ForwardContext)'s inherent
/// `ctx.read`/`ctx.write` helpers and returning [`ForwardEffect`](crate::ForwardEffect); they constrain
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

/// Monomorphic statement dispatch over a stage enum, keyed on the **context type**.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it.
///
/// `InterpDispatch` is parameterized by the **context type** `C` (an
/// [`InterpretCtx`]), *not* the engine — the context type is the specialization
/// boundary, here and in [`Interpretable`]. The engine asks itself to build *its*
/// context ([`I::context`](crate::Interp::context) →
/// [`I::Context<'_>`](crate::Interp::Context)) and passes it in; dispatch matches
/// the statement's language and forwards the
/// already-built `ctx` to the context-specialized [`Interpretable`]/[`FunctionEntry`]
/// rule, returning `C::Effect`. The forward engines build
/// [`ForwardContext<'_, I>`](crate::ForwardContext), so their `FrameDriver` bound is
/// `for<'a> InterpDispatch<ForwardContext<'a, I>>` (a *concrete* context type — no
/// `E0119`, no spurious `'static`), and forward dialect impls satisfy it unchanged.
/// A future analysis drives its own context type the same way (e.g.
/// `for<'a> InterpDispatch<LivenessContext<'a, I>>`), reusing this one generic
/// dispatch trait without overlapping the forward path.
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
