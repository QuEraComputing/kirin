use kirin_ir::LiftFrom;

use crate::{EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError, Location};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatementFrame {
    pub location: Location,
    pub env: EnvIndex,
}

impl StatementFrame {
    pub fn new(location: Location, env: EnvIndex) -> Self {
        Self { location, env }
    }
}

impl HasLocation for StatementFrame {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, F, C, E> Frame<I, F, C, E> for StatementFrame
where
    E: LiftFrom<InterpreterError>,
{
    fn step(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(
            InterpreterError::UnexpectedStatementFrameStep(self.location),
        ))
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}
