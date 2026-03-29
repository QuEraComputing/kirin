use crate::{
    ConsumeEffect, Interpretable, Lift, Machine, ProjectMachine, ProjectMachineMut, StageAccess,
    ValueStore, control::Directive,
};

use crate::InterpreterError;

/// Typed single-stage shell contract over one top-level machine.
pub trait Interpreter<'ir>: ValueStore<Error: From<InterpreterError>> + StageAccess<'ir> {
    type Machine: Machine<'ir>
        + ConsumeEffect<
            'ir,
            Output: Lift<
                Directive<
                    <Self::Machine as Machine<'ir>>::Stop,
                    <Self::Machine as Machine<'ir>>::Seed,
                >,
            >,
            Error: Into<Self::Error>,
        >;

    fn machine(&self) -> &Self::Machine;
    fn machine_mut(&mut self) -> &mut Self::Machine;

    fn interpret_current(&mut self)
    -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>;

    /// Consume a directive (the shell's effect type) by applying it to
    /// interpreter state: advance cursor, push/pop blocks, record stops, etc.
    fn consume_effect(
        &mut self,
        directive: Directive<
            <Self::Machine as Machine<'ir>>::Stop,
            <Self::Machine as Machine<'ir>>::Seed,
        >,
    ) -> Result<(), Self::Error>;

    fn project_machine<'a, Sub: ?Sized>(&'a self) -> &'a Sub
    where
        'ir: 'a,
        Self: Sized,
        Self::Machine: ProjectMachine<Sub>,
    {
        self.machine().project()
    }

    fn project_machine_mut<'a, Sub: ?Sized>(&'a mut self) -> &'a mut Sub
    where
        'ir: 'a,
        Self: Sized,
        Self::Machine: ProjectMachineMut<Sub>,
    {
        self.machine_mut().project_mut()
    }

    fn interpret_local<D>(&mut self, stmt: &D) -> Result<D::Effect, Self::Error>
    where
        Self: Sized,
        D: Interpretable<'ir, Self>,
        D::Error: Into<Self::Error>,
    {
        stmt.interpret(self).map_err(Into::into)
    }

    fn interpret_lifted<D>(
        &mut self,
        stmt: &D,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, Self::Error>
    where
        Self: Sized,
        D: Interpretable<'ir, Self>,
        D::Effect: Lift<<Self::Machine as Machine<'ir>>::Effect>,
        D::Error: Into<Self::Error>,
    {
        stmt.interpret(self).map_err(Into::into).map(Lift::lift)
    }
}
