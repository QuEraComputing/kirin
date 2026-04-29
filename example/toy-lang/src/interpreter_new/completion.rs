use std::convert::Infallible;

use kirin::ir::TryLiftFrom;
use kirin_interpreter_new::{ProjectOrSelf, StandardCompletion};
use kirin_scf::interpreter_new::ScfCompletion;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToyCompletion<V> {
    Standard(StandardCompletion<V>),
    Scf(ScfCompletion<V>),
}

impl<V> TryLiftFrom<StandardCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn try_lift_from(completion: StandardCompletion<V>) -> Result<Self, Self::Error> {
        Ok(Self::Standard(completion))
    }
}

impl<V> TryLiftFrom<ScfCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn try_lift_from(completion: ScfCompletion<V>) -> Result<Self, Self::Error> {
        Ok(Self::Scf(completion))
    }
}

impl<V> ProjectOrSelf<StandardCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn project_or_self(self) -> Result<StandardCompletion<V>, Self> {
        match self {
            Self::Standard(completion) => Ok(completion),
            other => Err(other),
        }
    }
}

impl<V> ProjectOrSelf<ScfCompletion<V>> for ToyCompletion<V> {
    type Error = Infallible;

    fn project_or_self(self) -> Result<ScfCompletion<V>, Self> {
        match self {
            Self::Scf(completion) => Ok(completion),
            other => Err(other),
        }
    }
}
