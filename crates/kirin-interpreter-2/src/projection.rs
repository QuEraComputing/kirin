/// Immutable projection into a structural submachine.
pub trait ProjectMachine<T: ?Sized> {
    fn project(&self) -> &T;
}

/// Mutable projection into a structural submachine.
pub trait ProjectMachineMut<T: ?Sized> {
    fn project_mut(&mut self) -> &mut T;
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
