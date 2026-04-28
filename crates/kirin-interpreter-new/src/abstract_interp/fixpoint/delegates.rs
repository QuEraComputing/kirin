use std::hash::Hash;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, SSAValue, StageInfo};

use crate::{Env, EnvIndex, InterpreterError, StageAccess};

use super::{SimpleFixpointInterpreter, Summary};

impl<'ir, Stage, K, F, C, E, S, Store, V> Env<V>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    S: Summary,
    Store: Env<V, Error = E>,
{
    type Error = E;

    fn alloc(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.store.free(index)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<V, Self::Error> {
        self.store.read(index, value)
    }

    fn write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), Self::Error> {
        self.store.write(index, value, data)
    }
}

impl<'ir, Stage, K, F, C, E, S, Store, L> StageAccess<L>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    Stage: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    S: Summary,
    E: From<InterpreterError>,
{
    type Error = E;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or(InterpreterError::MissingStage(stage))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(E::from)
    }
}
