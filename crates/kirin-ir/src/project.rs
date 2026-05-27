/// Default error type for failed projections (wrong active variant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    InvalidVariant,
}

/// Core project trait: `Self` can be projected into `To`.
///
/// This is the inverse direction of [`core::convert::TryFrom`]: given a sum
/// type `Self`, attempt to extract the variant of type `To`. Derive macros
/// generate this automatically for each `#[wraps]` variant, with
/// `type Error = `[`ProjectError`].
pub trait TryProjectTo<To>: Sized {
    type Error;
    fn try_project_to(self) -> Result<To, Self::Error>;
}

impl<T> TryProjectTo<T> for T {
    type Error = core::convert::Infallible;
    fn try_project_to(self) -> Result<T, core::convert::Infallible> {
        Ok(self)
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

    #[derive(Debug, PartialEq)]
    struct Inner(u32);

    #[derive(Debug, PartialEq)]
    enum Outer {
        A(Inner),
        B(u64),
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
