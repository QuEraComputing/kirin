/// Inject `self` into a composed coproduct type `Total`.
///
/// This is the "inclusion" morphism in the coproduct algebra. Given a dialect-specific
/// object and a total composed type, `lift` embeds the local object into the total.
///
/// # Examples
/// - `BlockCursor<V, L>: Lift<HighLevelCursor<V>>` — lift a dialect block cursor into the composed cursor coproduct
/// - `IfCursor<V, L>: Lift<HighLevelCursor<V>>` — lift an SCF cursor into the composed cursor coproduct
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Extract a component from a composed coproduct (partial — may fail).
///
/// Returns `Ok(local)` if `self` contains the `Local` variant, or `Err(self)` to return
/// ownership back to the caller when the variant doesn't match. This is algebraically
/// cleaner than `Option<Local>` since it avoids discarding the original on failure.
///
/// The identity impl `impl<T> Project<T> for T` always succeeds.
pub trait Project<Local>: Sized {
    fn try_project(self) -> Result<Local, Self>;
}

/// Ergonomic alias: `self.lift_into()` instead of `Lift::<T>::lift(self)`.
pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

/// Ergonomic alias: `self.project_into()` instead of `Project::<T>::try_project(self)`.
pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Result<T, Self>;
}

// Identity impls: every type lifts/projects to itself trivially.
impl<T> Lift<T> for T {
    fn lift(self) -> T {
        self
    }
}

impl<T> Project<T> for T {
    fn try_project(self) -> Result<T, T> {
        Ok(self)
    }
}

impl<F: Lift<T>, T> LiftInto<T> for F {
    fn lift_into(self) -> T {
        self.lift()
    }
}

impl<F: Project<T>, T> ProjectInto<T> for F {
    fn project_into(self) -> Result<T, Self> {
        self.try_project()
    }
}
