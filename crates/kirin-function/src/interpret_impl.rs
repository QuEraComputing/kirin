use kirin::prelude::{Dialect, GetInfo, HasStageInfo};
use kirin_interpreter::{
    Continuation, Interpretable, Interpreter, InterpreterError, SSACFGRegion, StageResolutionError,
};
use smallvec::smallvec;

use crate::{Call, FunctionBody, Return};

impl<T> SSACFGRegion for FunctionBody<T>
where
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn entry_block<L: Dialect>(
        &self,
        stage: &kirin::prelude::StageInfo<L>,
    ) -> Result<kirin::prelude::Block, InterpreterError> {
        self.body
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for FunctionBody<T>
where
    I: Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    L: Dialect + 'ir,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let stage = interp.resolve_stage::<L>()?;
        let entry = self
            .body
            .blocks(stage)
            .next()
            .ok_or(InterpreterError::MissingEntry)?;
        Ok(Continuation::Jump(entry, smallvec![]))
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Call<T>
where
    I: Interpreter<'ir>,
    I::StageInfo: HasStageInfo<L>,
    I::Error: From<InterpreterError>,
    I::Value: Clone,
    L: Dialect + 'ir,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let stage_id = interp.active_stage();
        let stage = interp.resolve_stage::<L>()?;

        let target_name = stage
            .symbol_table()
            .resolve(self.target())
            .cloned()
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget {
                    name: format!("{:?}", self.target()),
                },
            })?;

        let global_sym = interp
            .pipeline()
            .lookup_symbol(&target_name)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget {
                    name: target_name.clone(),
                },
            })?;

        let function = interp
            .pipeline()
            .function_by_name(global_sym)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget { name: target_name },
            })?;

        let function_info =
            interp
                .pipeline()
                .function_info(function)
                .ok_or(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::MissingFunction { function },
                })?;
        let staged_function = function_info
            .staged_functions()
            .get(&stage_id)
            .copied()
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

        let mut live_specializations = staged_info
            .specializations()
            .iter()
            .filter(|spec| !spec.is_invalidated())
            .map(|spec| spec.id());
        let callee = match (live_specializations.next(), live_specializations.next()) {
            (None, _) => {
                return Err(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::NoSpecialization { staged_function },
                }
                .into());
            }
            (Some(callee), None) => callee,
            (Some(_), Some(_)) => {
                let count = staged_info
                    .specializations()
                    .iter()
                    .filter(|spec| !spec.is_invalidated())
                    .count();
                return Err(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::AmbiguousSpecialization {
                        staged_function,
                        count,
                    },
                }
                .into());
            }
        };

        let args = self
            .args()
            .iter()
            .map(|ssa| interp.read(*ssa))
            .collect::<Result<kirin_interpreter::Args<I::Value>, _>>()?;
        Ok(Continuation::Call {
            callee,
            stage: stage_id,
            args,
            result: self.result(),
        })
    }
}

impl<'ir, I, L, T> Interpretable<'ir, I, L> for Return<T>
where
    I: Interpreter<'ir>,
    I::Value: Clone,
    L: Dialect,
    T: kirin::prelude::CompileTimeValue + Default,
{
    fn interpret(&self, interp: &mut I) -> Result<Continuation<I::Value, I::Ext>, I::Error> {
        let v = interp.read(self.value)?;
        Ok(Continuation::Return(v))
    }
}
