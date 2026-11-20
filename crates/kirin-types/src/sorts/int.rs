//! Integers: an index sort with literals, linear arithmetic, comparison, and
//! the singleton type of an integer.

use std::fmt;

use crate::{IndexSort, Project, Sort, SortValue, StaticCon, StaticSig};

/// `int`: an index sort, introduced like any user-defined base sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntSort;

/// The type that values of sort `int` evaluate to.
/// Wraps around `i64` so other dialect sorts can also use `i64` as their `T`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IntSortT(pub i64);

impl IndexSort for IntSort {
    type T = IntSortT;

    fn sort_of(_: &IntSortT) -> Self {
        IntSort
    }

    fn name(&self) -> &'static str {
        "int"
    }
}

/// Integer index terms: literals, linear arithmetic, comparison, and the
/// singleton type of an integer. Equality
/// is [`BoolStaticCon::Equal`](crate::BoolStaticCon::Equal).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntStaticCon {
    /// `+ : (int, int) ⇒ int`
    Add,
    /// `- : (int, int) ⇒ int`
    Sub,
    /// `k · _ : (int) ⇒ int`,
    /// multiplication by a literal only, so arithmetic stays linear.
    Mul(i64),
    /// `≥ : (int, int) ⇒ bool`
    Geq,
    /// `int : (int) ⇒ type`, the type of runtime int parameterized their value
    IntOf,
}

impl<S> StaticSig<S> for IntStaticCon
where
    S: IndexSort + From<IntSort>,
    S::T: From<IntSortT> + Project<IntSortT>,
{
    fn signature(&self) -> StaticCon<S> {
        let int = Sort::Index(S::from(IntSort));
        match self {
            Self::Add | Self::Sub => StaticCon {
                args: vec![int, int],
                result: int,
            },
            Self::Mul(_) => StaticCon {
                args: vec![int],
                result: int,
            },
            Self::Geq => StaticCon {
                args: vec![int, int],
                result: Sort::Bool,
            },
            Self::IntOf => StaticCon {
                args: vec![int],
                result: Sort::Type,
            },
        }
    }

    fn eval(&self, args: &[SortValue<S::T>]) -> Option<SortValue<S::T>> {
        let int = |value: &SortValue<S::T>| value.project::<IntSortT>().map(|n| n.0);
        let ext = |n: i64| SortValue::Index(IntSortT(n).into());
        match (self, args) {
            (Self::Add, [a, b]) => int(a)?.checked_add(int(b)?).map(ext),
            (Self::Sub, [a, b]) => int(a)?.checked_sub(int(b)?).map(ext),
            (Self::Mul(k), [a]) => k.checked_mul(int(a)?).map(ext),
            (Self::Geq, [a, b]) => Some(SortValue::Bool(int(a)? >= int(b)?)),
            // `int(n)` is a type, not a value
            (Self::IntOf, _) => None,
            _ => unreachable!("`eval` takes one value per argument of `signature`"),
        }
    }
}

impl fmt::Display for IntStaticCon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => f.write_str("+"),
            Self::Sub => f.write_str("-"),
            Self::Mul(k) => write!(f, "{k}·"),
            Self::Geq => f.write_str("≥"),
            Self::IntOf => f.write_str("int"),
        }
    }
}
