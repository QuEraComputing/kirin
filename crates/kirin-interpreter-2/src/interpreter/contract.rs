use crate::{
    ConsumeEffect, Interpretable, Lift, Machine, ProjectMachine, ProjectMachineMut, StageAccess,
    ValueStore, control::Directive,
};

use crate::InterpreterError;

/// Typed single-stage shell contract over one top-level machine.
pub trait Interpreter<'ir>: ValueStore + StageAccess<'ir> {
    type Machine: Machine<'ir> + ConsumeEffect<'ir>;
    type Error: From<InterpreterError>;

    fn machine(&self) -> &Self::Machine;
    fn machine_mut(&mut self) -> &mut Self::Machine;

    fn interpret_current(
        &mut self,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, <Self as Interpreter<'ir>>::Error>;

    #[allow(clippy::type_complexity)]
    fn consume_effect(
        &mut self,
        effect: <Self::Machine as Machine<'ir>>::Effect,
    ) -> Result<
        Directive<<Self::Machine as Machine<'ir>>::Stop, <Self::Machine as Machine<'ir>>::Seed>,
        <Self as Interpreter<'ir>>::Error,
    >;

    fn consume_control(
        &mut self,
        control: Directive<
            <Self::Machine as Machine<'ir>>::Stop,
            <Self::Machine as Machine<'ir>>::Seed,
        >,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;

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

    fn interpret_local<D>(
        &mut self,
        stmt: &D,
    ) -> Result<D::Effect, <Self as Interpreter<'ir>>::Error>
    where
        Self: Sized,
        D: Interpretable<'ir, Self>,
        D::Error: Into<<Self as Interpreter<'ir>>::Error>,
    {
        stmt.interpret(self).map_err(Into::into)
    }

    fn interpret_lifted<D>(
        &mut self,
        stmt: &D,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, <Self as Interpreter<'ir>>::Error>
    where
        Self: Sized,
        D: Interpretable<'ir, Self>,
        D::Effect: Lift<<Self::Machine as Machine<'ir>>::Effect>,
        D::Error: Into<<Self as Interpreter<'ir>>::Error>,
    {
        stmt.interpret(self).map_err(Into::into).map(Lift::lift)
    }

    fn consume_local_effect<Sub>(
        &mut self,
        effect: <Sub as Machine<'ir>>::Effect,
    ) -> Result<
        Directive<<Sub as Machine<'ir>>::Stop, <Sub as Machine<'ir>>::Seed>,
        <Self as Interpreter<'ir>>::Error,
    >
    where
        Self: Sized,
        Sub: Machine<'ir> + ConsumeEffect<'ir>,
        Self::Machine: ProjectMachineMut<Sub>,
        <Sub as ConsumeEffect<'ir>>::Error: Into<<Self as Interpreter<'ir>>::Error>,
    {
        self.project_machine_mut::<Sub>()
            .consume_effect(effect)
            .map_err(Into::into)
    }

    #[allow(clippy::type_complexity)]
    fn consume_lifted_effect<E>(
        &mut self,
        effect: E,
    ) -> Result<
        Directive<<Self::Machine as Machine<'ir>>::Stop, <Self::Machine as Machine<'ir>>::Seed>,
        <Self as Interpreter<'ir>>::Error,
    >
    where
        Self: Sized,
        E: Lift<<Self::Machine as Machine<'ir>>::Effect>,
    {
        self.consume_effect(effect.lift())
    }

    fn consume_local_control<S>(
        &mut self,
        control: Directive<S, <Self::Machine as Machine<'ir>>::Seed>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>
    where
        Self: Sized,
        S: Lift<<Self::Machine as Machine<'ir>>::Stop>,
    {
        self.consume_control(control.map_stop(Lift::lift))
    }
}
