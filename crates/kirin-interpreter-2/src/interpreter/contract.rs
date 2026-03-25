use crate::{
    ConsumeEffect, Interpretable, LiftEffect, LiftStop, Machine, ProjectMachine, ProjectMachineMut,
    StageAccess, ValueStore, control::Shell,
};

/// Typed single-stage shell contract over one top-level machine.
pub trait Interpreter<'ir>: ValueStore + StageAccess<'ir> {
    type Machine: Machine<'ir> + ConsumeEffect<'ir>;
    type Error;

    fn machine(&self) -> &Self::Machine;
    fn machine_mut(&mut self) -> &mut Self::Machine;

    fn interpret_current(
        &mut self,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, <Self as Interpreter<'ir>>::Error>;

    fn consume_effect(
        &mut self,
        effect: <Self::Machine as Machine<'ir>>::Effect,
    ) -> Result<Shell<<Self::Machine as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>;

    fn consume_control(
        &mut self,
        control: Shell<<Self::Machine as Machine<'ir>>::Stop>,
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

    fn lift_effect<Sub: Machine<'ir>>(
        &self,
        effect: <Sub as Machine<'ir>>::Effect,
    ) -> <Self::Machine as Machine<'ir>>::Effect
    where
        Self: Sized,
        Self::Machine: LiftEffect<'ir, Sub>,
    {
        <Self::Machine as LiftEffect<'ir, Sub>>::lift_effect(effect)
    }

    fn lift_stop<Sub: Machine<'ir>>(
        &self,
        stop: <Sub as Machine<'ir>>::Stop,
    ) -> <Self::Machine as Machine<'ir>>::Stop
    where
        Self: Sized,
        Self::Machine: LiftStop<'ir, Sub>,
    {
        <Self::Machine as LiftStop<'ir, Sub>>::lift_stop(stop)
    }

    fn interpret_local<D>(
        &mut self,
        stmt: &D,
    ) -> Result<<D::Machine as Machine<'ir>>::Effect, <Self as Interpreter<'ir>>::Error>
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
        Self::Machine: LiftEffect<'ir, D::Machine>,
        D::Error: Into<<Self as Interpreter<'ir>>::Error>,
    {
        let effect = self.interpret_local(stmt)?;
        Ok(self.lift_effect::<D::Machine>(effect))
    }

    fn consume_local_effect<Sub>(
        &mut self,
        effect: <Sub as Machine<'ir>>::Effect,
    ) -> Result<Shell<<Sub as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>
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

    fn consume_lifted_effect<Sub: Machine<'ir>>(
        &mut self,
        effect: <Sub as Machine<'ir>>::Effect,
    ) -> Result<Shell<<Self::Machine as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>
    where
        Self: Sized,
        Self::Machine: LiftEffect<'ir, Sub>,
    {
        self.consume_effect(self.lift_effect::<Sub>(effect))
    }

    fn consume_local_control<Sub: Machine<'ir>>(
        &mut self,
        control: Shell<<Sub as Machine<'ir>>::Stop>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>
    where
        Self: Sized,
        Self::Machine: LiftStop<'ir, Sub>,
    {
        self.consume_control(control.map_stop(|stop| self.lift_stop::<Sub>(stop)))
    }
}
