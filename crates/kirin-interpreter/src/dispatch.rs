use kirin_ir::{CompileStage, Dialect, Product, StageInfo, StageMeta, Statement};

use crate::{Ctx, Effect, EnvIndex, Interp, Scope};

/// Statement semantics. The single trait dialect authors implement.
///
/// Implementations are generic over the interpreter `I` and constrain only
/// the value domain (`I::Value: Add + ...`) and error lifting
/// (`I::Error: From<MyError>`). The same impl drives concrete execution and
/// abstract interpretation; the difference lives in the value type.
pub trait Interpretable<I: Interp>: Dialect {
    fn interpret(&self, ctx: &mut Ctx<'_, I>) -> Result<Effect<I::Value, I::Error>, I::Error>;
}

/// Function-entry semantics for callable statements.
///
/// Implemented by statements that define function bodies (e.g.
/// `kirin_function::Function`); describes the scope an engine enters when the
/// function is invoked. Derived on language enums with `#[derive(FunctionEntry)]`
/// where `#[callable]` marks the variants that wrap callable statements.
pub trait FunctionEntry<I: Interp>: Dialect {
    fn function_entry(
        &self,
        args: Product<I::Value>,
        ctx: &mut Ctx<'_, I>,
    ) -> Result<Scope<I::Value, I::Error>, I::Error>;
}

/// Monomorphic statement dispatch over a stage enum.
///
/// Mirrors `ParseDispatch` from the parser: multi-stage pipelines add
/// `#[derive(InterpDispatch)]` to their stage enum; single-language pipelines
/// (`Pipeline<StageInfo<L>>`) get the blanket impl below. Engines route every
/// statement execution and function entry through this trait; compiler
/// authors derive it and never call it.
pub trait InterpDispatch<I: Interp>: StageMeta {
    fn dispatch_statement(
        &self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<Effect<I::Value, I::Error>, I::Error>;

    fn dispatch_function_entry(
        &self,
        stage: CompileStage,
        body: Statement,
        args: Product<I::Value>,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<Scope<I::Value, I::Error>, I::Error>;
}

impl<I, L> InterpDispatch<I> for StageInfo<L>
where
    I: Interp,
    L: Dialect + Interpretable<I> + FunctionEntry<I>,
{
    fn dispatch_statement(
        &self,
        stage: CompileStage,
        statement: Statement,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<Effect<I::Value, I::Error>, I::Error> {
        let definition = statement.definition(self).clone();
        definition.interpret(&mut Ctx::new(interp, stage, statement, env))
    }

    fn dispatch_function_entry(
        &self,
        stage: CompileStage,
        body: Statement,
        args: Product<I::Value>,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<Scope<I::Value, I::Error>, I::Error> {
        let definition = body.definition(self).clone();
        definition.function_entry(args, &mut Ctx::new(interp, stage, body, env))
    }
}
