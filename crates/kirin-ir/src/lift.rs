/// Default error type for failed lifts in dynamic contexts (effects, cursors).
///
/// Pure dialect wrapper impls always return `Ok`; they may use
/// [`core::convert::Infallible`] as their `Error` type to encode that guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftError;

/// Default error type for failed projections (wrong active variant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    InvalidVariant,
}

/// Core lift trait: `Self` can be constructed from `From`.
///
/// Modeled after [`TryFrom`](core::convert::TryFrom): the associated `Error` type
/// lets each impl declare its own failure mode. Pure dialect wrapper impls that always
/// succeed should use `type Error = `[`core::convert::Infallible`].
///
/// Derive `#[derive(Dialect)]` generates this automatically for each `#[wraps]` variant,
/// with `type Error = `[`LiftError`].
pub trait TryLiftFrom<From>: Sized {
    type Error;
    fn try_lift_from(from: From) -> Result<Self, Self::Error>;
}

/// Core project trait: `Self` can be projected into `To`.
///
/// Modeled after [`TryFrom`](core::convert::TryFrom). Derive `#[derive(Dialect)]`
/// generates this automatically for each `#[wraps]` variant, with
/// `type Error = `[`ProjectError`].
pub trait TryProjectTo<To>: Sized {
    type Error;
    fn try_project_to(self) -> Result<To, Self::Error>;
}

// --- Identity blanket impls (T → T is always infallible) ---

impl<T> TryLiftFrom<T> for T {
    type Error = core::convert::Infallible;
    fn try_lift_from(from: T) -> Result<T, core::convert::Infallible> {
        Ok(from)
    }
}

impl<T> TryProjectTo<T> for T {
    type Error = core::convert::Infallible;
    fn try_project_to(self) -> Result<T, core::convert::Infallible> {
        Ok(self)
    }
}

// --- Convenience traits (blanket-implemented via the core traits) ---

/// Infallible lift of `Self` into `To`. Panics if the underlying `TryLiftFrom` returns
/// an error (only possible in dynamic/effect contexts; pure dialect lifts are always `Ok`).
pub trait Lift<To>: Sized {
    fn lift(self) -> To;
}

impl<F, T: TryLiftFrom<F>> Lift<T> for F {
    fn lift(self) -> T {
        T::try_lift_from(self).unwrap_or_else(|_| panic!("lift failed"))
    }
}

/// Fallible lift of `Self` into `To`, exposing the impl's error type.
pub trait TryLift<To>: Sized {
    type Error;
    fn try_lift(self) -> Result<To, Self::Error>;
}

impl<F, T: TryLiftFrom<F>> TryLift<T> for F {
    type Error = <T as TryLiftFrom<F>>::Error;
    fn try_lift(self) -> Result<T, Self::Error> {
        T::try_lift_from(self)
    }
}

/// Infallible projection of `Self` into `To`. Panics if the variant doesn't match.
pub trait Project<To>: Sized {
    fn project(self) -> To;
}

impl<F: TryProjectTo<T>, T> Project<T> for F {
    fn project(self) -> T {
        self.try_project_to()
            .unwrap_or_else(|_| panic!("project failed: invalid variant"))
    }
}

/// Fallible projection of `Self` into `To`, exposing the impl's error type.
pub trait TryProject<To>: Sized {
    type Error;
    fn try_project(self) -> Result<To, Self::Error>;
}

impl<F: TryProjectTo<T>, T> TryProject<T> for F {
    type Error = <F as TryProjectTo<T>>::Error;
    fn try_project(self) -> Result<T, Self::Error> {
        self.try_project_to()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;

    // ---- Identity blanket (T → T, error = Infallible) ----

    #[test]
    fn identity_lift_is_noop() {
        let x: u32 = 42u32.lift();
        assert_eq!(x, 42);
    }

    #[test]
    fn identity_try_lift_is_ok() {
        let x: Result<u32, Infallible> = 42u32.try_lift();
        assert_eq!(x, Ok(42));
    }

    #[test]
    fn identity_project_is_noop() {
        let x: u32 = 42u32.project();
        assert_eq!(x, 42);
    }

    #[test]
    fn identity_try_project_is_ok() {
        let x: Result<u32, Infallible> = 42u32.try_project();
        assert_eq!(x, Ok(42));
    }

    // ---- Cross-type lift/project with concrete error types ----

    #[derive(Debug, PartialEq)]
    struct Inner(u32);

    #[derive(Debug, PartialEq)]
    enum Outer {
        A(Inner),
        B(u64),
    }

    impl TryLiftFrom<Inner> for Outer {
        type Error = LiftError;
        fn try_lift_from(from: Inner) -> Result<Outer, LiftError> {
            Ok(Outer::A(from))
        }
    }

    impl TryProjectTo<Inner> for Outer {
        type Error = ProjectError;
        fn try_project_to(self) -> Result<Inner, ProjectError> {
            match self {
                Outer::A(inner) => Ok(inner),
                _ => Err(ProjectError::InvalidVariant),
            }
        }
    }

    #[test]
    fn core_try_lift_from_direct() {
        let result = Outer::try_lift_from(Inner(7));
        assert_eq!(result, Ok(Outer::A(Inner(7))));
    }

    #[test]
    fn core_try_project_to_direct() {
        let result = Outer::A(Inner(7)).try_project_to();
        assert_eq!(result, Ok(Inner(7)));
    }

    #[test]
    fn lift_convenience() {
        let outer: Outer = Inner(7).lift();
        assert_eq!(outer, Outer::A(Inner(7)));
    }

    #[test]
    fn try_lift_convenience_ok() {
        let result: Result<Outer, LiftError> = Inner(7).try_lift();
        assert_eq!(result, Ok(Outer::A(Inner(7))));
    }

    #[test]
    fn project_convenience_ok() {
        let inner: Inner = Outer::A(Inner(7)).project();
        assert_eq!(inner, Inner(7));
    }

    #[test]
    fn try_project_convenience_ok() {
        let result: Result<Inner, ProjectError> = Outer::A(Inner(7)).try_project();
        assert_eq!(result, Ok(Inner(7)));
    }

    #[test]
    fn try_project_wrong_variant_returns_err() {
        let result: Result<Inner, ProjectError> = Outer::B(99).try_project();
        assert_eq!(result, Err(ProjectError::InvalidVariant));
    }

    #[test]
    #[should_panic(expected = "project failed: invalid variant")]
    fn project_wrong_variant_panics() {
        let _: Inner = Outer::B(99).project();
    }
}
