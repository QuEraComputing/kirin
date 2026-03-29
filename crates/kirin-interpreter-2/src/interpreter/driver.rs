use kirin_ir::Statement;

use super::{Interpreter, Position};
use crate::{
    Machine, ValueStore,
    control::{Breakpoints, Fuel, Interrupt},
    result::{Run, Step, Suspension},
};

pub type StepResult<'ir, I> = Result<Step<<I as Machine<'ir>>::Effect>, <I as ValueStore>::Error>;

pub type RunResult<'ir, I> = Result<Run<<I as Machine<'ir>>::Stop>, <I as ValueStore>::Error>;

/// Shared driver loop for typed shells and typed stage views.
pub trait Driver<'ir>: Interpreter<'ir> + Position<'ir> + Fuel + Breakpoints + Interrupt {
    fn poll_execution_gate(&mut self) -> Result<Option<Statement>, Suspension>;

    fn stop_pending(&self) -> bool;

    fn take_stop(&mut self) -> Option<Self::Stop>;

    fn finish_step(&mut self, statement: Statement);

    fn step(&mut self) -> StepResult<'ir, Self>
    where
        Self::Effect: Clone,
    {
        let statement = match self.poll_execution_gate() {
            Ok(Some(statement)) => statement,
            Ok(None) => return Ok(Step::Completed),
            Err(reason) => return Ok(Step::Suspended(reason)),
        };

        let effect = self.interpret_current()?;
        self.consume_effect(effect.clone())?;
        if let Some(remaining) = self.fuel() {
            debug_assert!(remaining > 0, "fuel must be checked before step burn");
            self.set_fuel(Some(remaining - 1));
        }

        if !self.stop_pending() {
            self.finish_step(statement);
        }

        Ok(Step::Stepped(effect))
    }

    fn run(&mut self) -> RunResult<'ir, Self> {
        loop {
            let statement = match self.poll_execution_gate() {
                Ok(Some(statement)) => statement,
                Ok(None) => return Ok(Run::Completed),
                Err(reason) => return Ok(Run::Suspended(reason)),
            };

            let effect = self.interpret_current()?;
            self.consume_effect(effect)?;
            if let Some(remaining) = self.fuel() {
                debug_assert!(remaining > 0, "fuel must be checked before step burn");
                self.set_fuel(Some(remaining - 1));
            }

            if let Some(stop) = self.take_stop() {
                return Ok(Run::Stopped(stop));
            }

            self.finish_step(statement);
        }
    }

    fn run_until_break(&mut self) -> RunResult<'ir, Self> {
        self.run()
    }
}
