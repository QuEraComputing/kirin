use kirin_ir::{CompileStage, Dialect, StageInfo};

use crate::{EnvIndex, InterpreterError, Location, StatementEffect};

pub trait StageAccess<L: Dialect> {
    type Error;

    fn stage_info(&self, stage: CompileStage) -> Result<&StageInfo<L>, Self::Error>;
}

pub trait StatementDispatch<L: Dialect, F, C, E, T> {
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E>;
}

pub trait Interpretable<I, F, C, E, T>: Dialect {
    fn interpret(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
    ) -> Result<StatementEffect<F, C, T>, E>;
}

impl<I, L, F, C, E, T> StatementDispatch<L, F, C, E, T> for I
where
    I: StageAccess<L, Error = E>,
    L: Interpretable<I, F, C, E, T>,
    E: From<InterpreterError>,
{
    fn dispatch_statement(
        &mut self,
        location: Location,
        env: EnvIndex,
    ) -> Result<StatementEffect<F, C, T>, E> {
        let statement = location
            .active_statement()
            .ok_or(InterpreterError::ExpectedActiveStatement(location))?;
        let definition = {
            let stage_info = self.stage_info(location.stage)?;
            statement.definition(stage_info).clone()
        };
        definition.interpret(location, env, self)
    }
}
