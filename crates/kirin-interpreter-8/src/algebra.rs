/// Inject `self` into a composed coproduct type `Total`.
///
/// Mirrors dialect composition: dialect D injects into language L = D1|D2|...
/// via the enum constructor; dialect cursor C_D injects into C_L the same way.
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Extract a component type from a composed coproduct (partial).
///
/// Returns `Some(component)` if `self` is this variant, `None` otherwise.
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

// Identity: every type lifts into itself.
impl<T> Lift<T> for T {
    fn lift(self) -> T {
        self
    }
}

// Identity: projecting T out of T always succeeds.
impl<T> Project<T> for T {
    fn project(self) -> Option<T> {
        Some(self)
    }
}

// Blanket: F: Lift<T> implies F: LiftInto<T>.
impl<F: Lift<T>, T> LiftInto<T> for F {
    fn lift_into(self) -> T {
        self.lift()
    }
}

// Blanket: F: Project<T> implies F: ProjectInto<T>.
impl<F: Project<T>, T> ProjectInto<T> for F {
    fn project_into(self) -> Option<T> {
        self.project()
    }
}
