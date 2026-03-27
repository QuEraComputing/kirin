use crate::Machine;

/// Immutable projection into a structural submachine.
pub trait ProjectMachine<T: ?Sized> {
    fn project(&self) -> &T;
}

/// Mutable projection into a structural submachine.
pub trait ProjectMachineMut<T: ?Sized> {
    fn project_mut(&mut self) -> &mut T;
}

/// Structural lifting from a submachine effect into a composed machine effect.
pub trait LiftEffect<'ir, Sub>: Machine<'ir>
where
    Sub: Machine<'ir>,
{
    fn lift_effect(effect: Sub::Effect) -> Self::Effect;
}

/// Structural lifting from a submachine stop into a composed machine stop.
pub trait LiftStop<'ir, Sub>: Machine<'ir>
where
    Sub: Machine<'ir>,
{
    fn lift_stop(stop: Sub::Stop) -> Self::Stop;
}

/// Identity projection — any machine projects to itself.
impl<T> ProjectMachine<T> for T {
    fn project(&self) -> &T {
        self
    }
}

/// Identity projection — any machine projects to itself (mutable).
impl<T> ProjectMachineMut<T> for T {
    fn project_mut(&mut self) -> &mut T {
        self
    }
}
