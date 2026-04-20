/// Infallible embedding of a type into a sum/wrapper type.
///
/// Implement this to lift a dialect-specific value (statement, type, cursor, etc.)
/// into a composite type that spans multiple dialects.
pub trait Lift<Total>: Sized {
    fn lift(self) -> Total;
}

/// Fallible extraction of a specific type from a sum/wrapper type.
///
/// The inverse of [`Lift`]. Returns `Err(self)` when `self` holds a different variant.
pub trait Project<Local>: Sized {
    fn try_project(self) -> Result<Local, Self>;
}

/// Convenience mirror of [`Lift`] — derived automatically for any `F: Lift<T>`.
pub trait LiftInto<T>: Sized {
    fn lift_into(self) -> T;
}

/// Convenience mirror of [`Project`] — derived automatically for any `F: Project<T>`.
pub trait ProjectInto<T>: Sized {
    fn project_into(self) -> Result<T, Self>;
}

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
