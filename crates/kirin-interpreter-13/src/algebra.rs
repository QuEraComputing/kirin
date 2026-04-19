use kirin_ir::Dialect;

/// Inject `self` into a composed coproduct type `Total`.
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Extract a component from a composed coproduct (partial — may fail).
///
/// Returns `Ok(local)` if `self` contains the `Local` variant, or `Err(self)` to return
/// ownership back to the caller when the variant doesn't match.
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

/// Marker trait for cursor types that serve a single dialect at a single stage.
///
/// Implementing this on cursor type `C` opts it into the blanket `CallSeam<L>` impl
/// in `kirin-function`. Multi-stage cursor types MUST NOT implement this — they
/// provide their own `CallSeam` impl with cross-stage dispatch logic.
///
/// # Example
/// ```ignore
/// impl<V: Clone> SingleStageCursorFor<HighLevel> for HighLevelCursor<V> {}
/// // MultiCursor<V> does NOT implement this.
/// ```
pub trait SingleStageCursorFor<L: Dialect> {}
