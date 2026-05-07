use std::hash::Hash;

use kirin_ir::{CompileStage, Dialect, HasStageInfo, LiftFrom, SSAValue, StageInfo};

use crate::{Env, EnvIndex, ForkEnv, InterpreterError, StageAccess};

use super::{SimpleFixpointInterpreter, Summary};

impl<'ir, Stage, K, F, C, E, S, Store, V> Env<V>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    S: Summary,
    Store: Env<V>,
    E: LiftFrom<Store::Error>,
{
    type Error = E;

    fn alloc(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.store.free(index).map_err(E::lift_from)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<V, Self::Error> {
        self.store.read(index, value).map_err(E::lift_from)
    }

    fn write(&mut self, index: EnvIndex, value: SSAValue, data: V) -> Result<(), Self::Error> {
        self.store.write(index, value, data).map_err(E::lift_from)
    }
}

impl<'ir, Stage, K, F, C, E, S, Store, V> ForkEnv<V>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    K: Clone + Eq + Hash,
    S: Summary,
    Store: ForkEnv<V>,
    E: LiftFrom<Store::Error>,
{
    fn fork_env(&mut self, index: EnvIndex) -> Result<EnvIndex, Self::Error> {
        self.store.fork_env(index).map_err(E::lift_from)
    }
}

impl<'ir, Stage, K, F, C, E, S, Store, L> StageAccess<L>
    for SimpleFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store>
where
    Stage: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    S: Summary,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| E::lift_from(InterpreterError::MissingStage(stage)))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(E::lift_from)
    }
}
