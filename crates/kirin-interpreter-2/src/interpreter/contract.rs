use crate::{
    ConsumeEffect, Interpretable, Machine, ProjectMachine, ProjectMachineMut, StageAccess,
    ValueStore,
};

use crate::InterpreterError;

/// Typed interpreter shell contract.
///
/// An interpreter IS a [`Machine`] whose [`Effect`](Machine::Effect) is the
/// shell-level control type (e.g. [`Directive`]).  It consumes those effects
/// terminally via [`ConsumeEffect<'ir, ()>`](ConsumeEffect).
///
/// How the shell is *composed* with an inner dialect machine is an
/// implementation detail (e.g. `SingleStage` carries an `M` type parameter).
/// At the trait level we only care about the top-level machine interface.
pub trait Interpreter<'ir>:
    ValueStore<Error: From<InterpreterError>>
    + StageAccess<'ir>
    + Machine<'ir>
    + ConsumeEffect<'ir, (), Error = <Self as ValueStore>::Error>
{
    /// Interpret the current statement and return the top-level effect.
    ///
    /// Implementations encapsulate the full pipeline: interpret the statement,
    /// feed the dialect effect through the inner machine, and return the
    /// resulting shell-level effect.
    fn interpret_current(&mut self) -> Result<Self::Effect, <Self as ValueStore>::Error>;

    fn project_machine<'a, Sub: ?Sized>(&'a self) -> &'a Sub
    where
        'ir: 'a,
        Self: Sized + ProjectMachine<Sub>,
    {
        self.project()
    }

    fn project_machine_mut<'a, Sub: ?Sized>(&'a mut self) -> &'a mut Sub
    where
        'ir: 'a,
        Self: Sized + ProjectMachineMut<Sub>,
    {
        self.project_mut()
    }

    fn interpret_local<D>(&mut self, stmt: &D) -> Result<D::Effect, <Self as ValueStore>::Error>
    where
        Self: Sized,
        D: Interpretable<'ir, Self>,
        D::Error: Into<<Self as ValueStore>::Error>,
    {
        stmt.interpret(self).map_err(Into::into)
    }
}
