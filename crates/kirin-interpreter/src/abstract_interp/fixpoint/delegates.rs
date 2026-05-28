use kirin_ir::{CompileStage, Dialect, HasStageInfo, SSAValue, StageInfo};

use crate::{Env, EnvIndex, FixpointProfile, ForkEnv, InterpreterError, StageAccess};

use super::StandardFixpointInterpreter;

impl<'ir, P, Store, Deps> Env<P::Value> for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    Store: Env<P::Value>,
    P::Error: From<Store::Error>,
{
    type Error = P::Error;

    fn alloc(&mut self) -> EnvIndex {
        self.store.alloc()
    }

    fn free(&mut self, index: EnvIndex) -> Result<(), Self::Error> {
        self.store.free(index).map_err(P::Error::from)
    }

    fn read(&self, index: EnvIndex, value: SSAValue) -> Result<P::Value, Self::Error> {
        self.store.read(index, value).map_err(P::Error::from)
    }

    fn write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: P::Value,
    ) -> Result<(), Self::Error> {
        self.store.write(index, value, data).map_err(P::Error::from)
    }
}

impl<'ir, P, Store, Deps> ForkEnv<P::Value> for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    Store: ForkEnv<P::Value>,
    P::Error: From<Store::Error>,
{
    fn fork_env(&mut self, index: EnvIndex) -> Result<EnvIndex, Self::Error> {
        self.store.fork_env(index).map_err(P::Error::from)
    }
}

impl<'ir, P, Store, Deps, L> StageAccess<L> for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error> {
        let stage_info = self
            .pipeline
            .stage(stage)
            .ok_or_else(|| P::Error::from(InterpreterError::MissingStage(stage)))?;
        stage_info
            .try_stage_info()
            .ok_or(InterpreterError::MissingStageInfo(stage))
            .map_err(P::Error::from)
    }
}
