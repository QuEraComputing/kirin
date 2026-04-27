use crate::{FrameEffect, Location};

pub trait HasLocation {
    fn location(&self) -> Location;
}

pub trait Frame<I, F, C, E>: HasLocation {
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E>;

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E>;
}

pub trait ProjectOrSelf<To>: Sized {
    type Error;

    fn project_or_self(self) -> Result<To, Self>;
}

impl<T> ProjectOrSelf<T> for T {
    type Error = core::convert::Infallible;

    fn project_or_self(self) -> Result<T, Self> {
        Ok(self)
    }
}
