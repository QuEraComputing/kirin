use crate::{ConsumeEffect, Control, Machine, StageAccess, ValueStore};

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
    ) -> Result<Control<<Self::Machine as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>;

    fn consume_control(
        &mut self,
        control: Control<<Self::Machine as Machine<'ir>>::Stop>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error>;
}
