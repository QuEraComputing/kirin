/// Inject a component type into a composed coproduct type.
///
/// Mirrors dialect composition: dialect D injects into language L = D1|D2|...
/// via the enum constructor; dialect cursor C_D injects into C_L the same way.
/// NOT the same as `From` — `Lift` is semantically "coproduct injection".
pub trait Lift<From> {
    fn lift(from: From) -> Self;
}

/// Extract a component type from a composed coproduct type (partial).
///
/// Returns `Ok(component)` if `self` is this variant, `Err(self)` to pass
/// through unchanged.
pub trait Project<To>: Sized {
    fn project(self) -> Result<To, Self>;
}

/// Ergonomic dual of `Lift`: lift `self` into `T`.
pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

/// Ergonomic dual of `Project`: project `self` into `T`.
pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Result<T, Self>;
}

// Identity: every type lifts into itself.
impl<T> Lift<T> for T {
    fn lift(from: T) -> Self {
        from
    }
}

// Identity: projecting T out of T always succeeds.
impl<T> Project<T> for T {
    fn project(self) -> Result<T, T> {
        Ok(self)
    }
}

// Blanket: F lifts into T whenever T: Lift<F>.
impl<F, T: Lift<F>> LiftInto<T> for F {
    fn lift_into(self) -> T {
        T::lift(self)
    }
}

// Blanket: F projects into T whenever F: Project<T>.
impl<F: Project<T>, T> ProjectInto<T> for F {
    fn project_into(self) -> Result<T, Self> {
        self.project()
    }
}
