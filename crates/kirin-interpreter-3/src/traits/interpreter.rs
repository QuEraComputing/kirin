use std::ops::ControlFlow;

use kirin_ir::{CompileStage, Function, Pipeline, SSAValue, SpecializedFunction};

use crate::{Effect, InterpError, InterpreterError, Machine};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolutionPolicy {
    #[default]
    UniqueLive,
}

pub trait ValueRead {
    type Value: Clone;

    fn read(&self, value: SSAValue) -> Result<Self::Value, InterpreterError>;
}

pub trait PipelineAccess {
    type StageInfo;

    fn pipeline(&self) -> &Pipeline<Self::StageInfo>;

    fn current_stage(&self) -> CompileStage;

    fn resolve_callee(
        &self,
        function: Function,
        args: &[<Self as ValueRead>::Value],
        policy: ResolutionPolicy,
    ) -> Result<SpecializedFunction, InterpreterError>
    where
        Self: ValueRead;
}

pub trait Interpreter: Machine + ValueRead + PipelineAccess + Sized {
    type Dialect;
    type DialectEffect;
    type DialectError;

    fn step(&mut self) -> Result<ControlFlow<Self::Value>, Self::Error>;

    fn run(&mut self) -> Result<Self::Value, Self::Error> {
        loop {
            match self.step()? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(value) => return Ok(value),
            }
        }
    }
}

pub trait Interpretable<I: Interpreter> {
    type Effect;
    type Error;

    #[allow(clippy::type_complexity)]
    fn interpret(
        &self,
        interp: &mut I,
    ) -> Result<Effect<I::Value, Self::Effect>, InterpError<Self::Error>>;
}
