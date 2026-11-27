//! Function types, for a dialect whose values are functions (e.g. closures).

use std::fmt;

use crate::{BaseSort, IndexSort, Sort, SortValue, StaticCon, StaticSig};

/// `Fn : ([type], [type]) ⇒ type`, the type `Fn(T̄, Ū)` of a function value
/// taking arguments `T̄` and returning results `Ū`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FnStaticCon;

impl<S: IndexSort> StaticSig<S> for FnStaticCon {
    fn signature(&self) -> StaticCon<S> {
        let types = Sort::Seq(BaseSort::Type);
        StaticCon {
            args: vec![types, types],
            result: BaseSort::Type,
        }
    }

    /// A function type is a type, not a value.
    fn eval(&self, _: &[SortValue<S::T>]) -> Option<SortValue<S::T>> {
        None
    }
}

impl fmt::Display for FnStaticCon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Fn")
    }
}
