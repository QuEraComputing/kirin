use std::convert::Infallible;

pub trait Lift<From> {
    fn lift(from: From) -> Self;
}

pub trait Project<To> {
    fn project(self) -> To;
}

pub trait TryLift<From>: Sized {
    type Error;

    fn try_lift(from: From) -> Result<Self, Self::Error>;
}

pub trait TryProject<To>: Sized {
    type Error;

    fn try_project(self) -> Result<To, Self::Error>;
}

pub trait LiftInto<Target>: Sized {
    fn lift_into(self) -> Target;
}

pub trait TryLiftInto<Target>: Sized {
    type Error;

    fn try_lift(self) -> Result<Target, Self::Error>;
}

impl<T> Lift<T> for T {
    fn lift(from: T) -> Self {
        from
    }
}

impl<T> Project<T> for T {
    fn project(self) -> T {
        self
    }
}

impl<F, T> TryLift<F> for T
where
    T: Lift<F>,
{
    type Error = Infallible;

    fn try_lift(from: F) -> Result<Self, Self::Error> {
        Ok(Self::lift(from))
    }
}

impl<F, T> TryProject<T> for F
where
    F: Project<T>,
{
    type Error = Infallible;

    fn try_project(self) -> Result<T, Self::Error> {
        Ok(self.project())
    }
}

impl<F, T> LiftInto<T> for F
where
    T: Lift<F>,
{
    fn lift_into(self) -> T {
        T::lift(self)
    }
}

impl<F, T> TryLiftInto<T> for F
where
    T: TryLift<F>,
{
    type Error = <T as TryLift<F>>::Error;

    fn try_lift(self) -> Result<T, Self::Error> {
        T::try_lift(self)
    }
}
