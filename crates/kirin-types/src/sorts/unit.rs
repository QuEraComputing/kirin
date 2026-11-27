//! The unit type, for a dialect with a value that carries no information.

use std::fmt;

use crate::{BaseSort, IndexSort, SortValue, StaticCon, StaticSig};

/// `unit : () ⇒ type`. Unlike the empty list `[]`, which is no values at
/// all, `unit` is the type of one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitStaticCon;

impl<S: IndexSort> StaticSig<S> for UnitStaticCon {
    fn signature(&self) -> StaticCon<S> {
        StaticCon {
            args: vec![],
            result: BaseSort::Type,
        }
    }

    /// `unit` is a type, not a value.
    fn eval(&self, _: &[SortValue<S::T>]) -> Option<SortValue<S::T>> {
        None
    }
}

impl fmt::Display for UnitStaticCon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unit")
    }
}
