/// Inject `self` into a composed coproduct type `Total`.
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Extract a component type from a composed coproduct (partial).
pub trait Project<Local>: Sized {
    fn project(self) -> Option<Local>;
}

/// Ergonomic dual of `Lift`: lift `self` into `T`.
pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

/// Ergonomic dual of `Project`: project `self` into `T`.
pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Option<T>;
}

impl<T> Lift<T> for T {
    fn lift(self) -> T {
        self
    }
}

impl<T> Project<T> for T {
    fn project(self) -> Option<T> {
        Some(self)
    }
}

impl<F: Lift<T>, T> LiftInto<T> for F {
    fn lift_into(self) -> T {
        self.lift()
    }
}

impl<F: Project<T>, T> ProjectInto<T> for F {
    fn project_into(self) -> Option<T> {
        self.project()
    }
}
