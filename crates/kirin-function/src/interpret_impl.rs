use kirin::prelude::{CompileTimeValue, Dialect, GetInfo, HasRegionBody, HasStageInfo};
use kirin_interpreter::{
    Continuation, Interpretable, Interpreter, InterpreterError, ProductValue, SSACFGRegion,
    StageResolutionError,
};
use smallvec::smallvec;

use crate::{Bind, Call, FunctionBody, Lambda, Lexical, Lifted, Return};

/// Shared interpret logic for any type with a single region body: resolve stage,
/// find the entry block, and jump to it.
fn interpret_region_body<'ir, I, L>(
    op: &impl HasRegionBody,
    interp: &mut I,
) -> Result<Continuation<I::Value, I::Ext>, I::Error>
where
    I: Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Interpretable<'ir, I> + 'ir,
{
    let stage = interp.resolve_stage::<L>()?;
    let entry = op
        .entry_block(stage)
        .ok_or_else(InterpreterError::missing_entry_block)?;
    Ok(Continuation::Jump(entry, smallvec![]))
}

impl<T: CompileTimeValue> SSACFGRegion for FunctionBody<T> {
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        HasRegionBody::entry_block(self, stage).ok_or_else(InterpreterError::missing_entry_block)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for FunctionBody<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        interpret_region_body(self, interp)
    }
}

impl<T: CompileTimeValue> SSACFGRegion for Lambda<T> {
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        HasRegionBody::entry_block(self, stage).ok_or_else(InterpreterError::missing_entry_block)
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lambda<T>
where
    I: Interpreter<'ir>,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        interpret_region_body(self, interp)
    }
}

impl<T> SSACFGRegion for Lexical<T>
where
    T: CompileTimeValue,
{
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        match self {
            Lexical::FunctionBody(op) => SSACFGRegion::entry_block(op, stage),
            Lexical::Lambda(op) => SSACFGRegion::entry_block(op, stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lexical<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            Lexical::FunctionBody(op) => op.interpret::<L>(interp),
            Lexical::Lambda(op) => op.interpret::<L>(interp),
            Lexical::Call(op) => op.interpret::<L>(interp),
            Lexical::Return(op) => op.interpret::<L>(interp),
        }
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Bind<T>
where
    I: Interpreter<'ir>,
    T: kirin::prelude::CompileTimeValue,
{
    fn interpret<L>(&self, _interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        Err(InterpreterError::custom(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "bind is not yet supported in the interpreter",
        ))
        .into())
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Call<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone,
    T: kirin::prelude::CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let stage_id = interp.active_stage();
        let stage = interp.resolve_stage::<L>()?;
        let target_name = stage
            .symbol_table()
            .resolve(self.target())
            .cloned()
            .unwrap_or_else(|| format!("{:?}", self.target()));
        let function = interp
            .pipeline()
            .resolve_function(stage, self.target())
            .ok_or(InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget { name: target_name },
            })?;
        let staged_function = interp
            .pipeline()
            .function_info(function)
            .and_then(|info| info.staged_function(stage_id))
            .ok_or(InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::MissingFunction { function },
            })?;
        let staged_info =
            staged_function
                .get_info(stage)
                .ok_or(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::MissingFunction { function },
                })?;
        let callee = staged_info
            .unique_live_specialization()
            .map_err(|error| match error {
                kirin::prelude::UniqueLiveSpecializationError::NoSpecialization => {
                    InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: StageResolutionError::NoSpecialization { staged_function },
                    }
                }
                kirin::prelude::UniqueLiveSpecializationError::Ambiguous { count } => {
                    InterpreterError::StageResolution {
                        stage: stage_id,
                        kind: StageResolutionError::AmbiguousSpecialization {
                            staged_function,
                            count,
                        },
                    }
                }
            })?;

        let args = self
            .args()
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<kirin_interpreter::Args<I::Value>, _>>()?;
        Ok(Continuation::Call {
            callee,
            stage: stage_id,
            args,
            results: self.results().iter().copied().collect(),
        })
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Return<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ProductValue,
    T: kirin::prelude::CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        let values: Vec<I::Value> = self
            .values
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<_, _>>()?;
        let product = <I::Value as ProductValue>::new_product(values);
        Ok(Continuation::Return(product))
    }
}

impl<T> SSACFGRegion for Lifted<T>
where
    T: CompileTimeValue,
{
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        match self {
            Lifted::FunctionBody(op) => SSACFGRegion::entry_block(op, stage),
            _ => Err(InterpreterError::missing_entry_block()),
        }
    }
}

impl<'ir, I, T> Interpretable<'ir, I> for Lifted<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone + ProductValue,
    T: CompileTimeValue,
{
    fn interpret<L>(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error>
    where
        I::StageInfo: HasStageInfo<L>,
        I::Error: From<InterpreterError>,
        L: Interpretable<'ir, I> + 'ir,
    {
        match self {
            Lifted::FunctionBody(op) => op.interpret::<L>(interp),
            Lifted::Bind(op) => op.interpret::<L>(interp),
            Lifted::Call(op) => op.interpret::<L>(interp),
            Lifted::Return(op) => op.interpret::<L>(interp),
        }
    }
}
