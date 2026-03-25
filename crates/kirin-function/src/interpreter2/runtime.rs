use kirin::prelude::{
    Block, CompileTimeValue, Dialect, GetInfo, SSAValue, SpecializedFunction, StageInfo, Symbol,
};
use kirin_interpreter_2::{
    ConsumeEffect, ExecutionSeed, Interpretable, Interpreter, InterpreterError, LiftEffect,
    LiftStop, ProductValue, ProjectMachine, ProjectMachineMut, StageAccess, StageResolutionError,
    ValueStore, interpreter::Position, interpreter::SingleStage,
};

use super::Machine;

pub trait Runtime<'ir, T>:
    Interpreter<'ir> + Position<'ir> + ValueStore<Error = <Self as Interpreter<'ir>>::Error>
where
    T: CompileTimeValue,
    <Self as ValueStore>::Value: Clone + ProductValue,
{
    type Dialect: Dialect + 'ir;

    fn stage_info(&self) -> &'ir StageInfo<Self::Dialect>;

    fn function_machine(&self) -> &Machine<<Self as ValueStore>::Value>;

    fn function_machine_mut(&mut self) -> &mut Machine<<Self as ValueStore>::Value>;

    fn take_value_bindings(&mut self) -> Vec<(SSAValue, <Self as ValueStore>::Value)>;

    fn replace_value_bindings(
        &mut self,
        bindings: Vec<(SSAValue, <Self as ValueStore>::Value)>,
    ) -> Vec<(SSAValue, <Self as ValueStore>::Value)>;

    fn bind_function_args(
        &mut self,
        block: Block,
        args: &[<Self as ValueStore>::Value],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;

    fn resume_seed_after_current(&self)
    -> Result<ExecutionSeed, <Self as Interpreter<'ir>>::Error>;

    fn resolve_callee(
        &self,
        target: Symbol,
    ) -> Result<SpecializedFunction, <Self as Interpreter<'ir>>::Error>
    where
        <Self as Interpreter<'ir>>::Error: From<InterpreterError>,
    {
        let stage_id = self.active_stage();
        let stage = self.stage_info();

        let target_name = stage
            .symbol_table()
            .resolve(target)
            .cloned()
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget {
                    name: format!("{target:?}"),
                },
            })?;

        let global_symbol = self.pipeline().lookup_symbol(&target_name).ok_or_else(|| {
            InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget {
                    name: target_name.clone(),
                },
            }
        })?;

        let function = self
            .pipeline()
            .function_by_name(global_symbol)
            .ok_or_else(|| InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::UnknownTarget { name: target_name },
            })?;

        let function_info =
            self.pipeline()
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

        match (live_specializations.next(), live_specializations.next()) {
            (None, _) => Err(InterpreterError::StageResolution {
                stage: stage_id,
                kind: StageResolutionError::NoSpecialization { staged_function },
            }
            .into()),
            (Some(callee), None) => Ok(callee),
            (Some(_), Some(_)) => {
                let count = staged_info
                    .specializations()
                    .iter()
                    .filter(|spec| !spec.is_invalidated())
                    .count();
                Err(InterpreterError::StageResolution {
                    stage: stage_id,
                    kind: StageResolutionError::AmbiguousSpecialization {
                        staged_function,
                        count,
                    },
                }
                .into())
            }
        }
    }

    fn entry_block(
        &self,
        callee: SpecializedFunction,
    ) -> Result<Block, <Self as Interpreter<'ir>>::Error>
    where
        <Self as Interpreter<'ir>>::Error: From<InterpreterError>,
    {
        let stage = self.stage_info();
        let spec_info =
            callee
                .get_info(stage)
                .ok_or_else(|| InterpreterError::StageResolution {
                    stage: self.active_stage(),
                    kind: StageResolutionError::MissingCallee { callee },
                })?;
        let body = *spec_info.body();
        let region = body
            .regions(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)?;

        region
            .blocks(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)
            .map_err(Into::into)
    }
}

impl<'ir, L, T, V, M, E> Runtime<'ir, T> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir + Interpretable<'ir, SingleStage<'ir, L, V, M, E>, Machine = M>,
    T: CompileTimeValue,
    V: Clone + ProductValue + 'ir,
    M: kirin_interpreter_2::Machine<'ir>
        + ConsumeEffect<'ir>
        + ProjectMachine<Machine<V>>
        + ProjectMachineMut<Machine<V>>
        + LiftEffect<'ir, Machine<V>>
        + LiftStop<'ir, Machine<V>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    type Dialect = L;

    fn stage_info(&self) -> &'ir StageInfo<Self::Dialect> {
        self.active_stage_info::<L>()
    }

    fn function_machine(&self) -> &Machine<V> {
        self.project_machine::<Machine<V>>()
    }

    fn function_machine_mut(&mut self) -> &mut Machine<V> {
        self.project_machine_mut::<Machine<V>>()
    }

    fn take_value_bindings(&mut self) -> Vec<(SSAValue, V)> {
        Self::take_value_bindings(self)
    }

    fn replace_value_bindings(&mut self, bindings: Vec<(SSAValue, V)>) -> Vec<(SSAValue, V)> {
        Self::replace_value_bindings(self, bindings)
    }

    fn bind_function_args(
        &mut self,
        block: Block,
        args: &[V],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        Self::bind_block_args(self, block, args)
    }

    fn resume_seed_after_current(
        &self,
    ) -> Result<ExecutionSeed, <Self as Interpreter<'ir>>::Error> {
        Self::resume_seed_after_current(self).map_err(Into::into)
    }
}
