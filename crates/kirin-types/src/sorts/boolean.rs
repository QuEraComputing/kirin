//! Booleans: literals, propositional connectives, equality, and the singleton
//! type of a proposition, over the built-in sort `bool`.

use std::fmt;

use crate::{IndexSort, Sort, SortValue, StaticCon, StaticSig};

/// Boolean static terms. `bool` is a built-in sort, so unlike `int` there is
/// no base sort to contribute, only constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoolStaticCon<S> {
    /// `¬ : (bool) ⇒ bool`
    Not,
    /// `∧ : (bool, bool) ⇒ bool`
    And,
    /// `∨ : (bool, bool) ⇒ bool`
    Or,
    /// `⇒ : (bool, bool) ⇒ bool`
    Implies,
    /// `= : (σ, σ) ⇒ bool`, equality at the sort `σ`: one constructor per sort.
    Equal(Sort<S>),
    /// `bool : (bool) ⇒ type`, the singleton type of the boolean `B`: branching
    /// on its value establishes `B` in one arm and `¬B` in the other.
    BoolOf,
}

impl<S: IndexSort> StaticSig<S> for BoolStaticCon<S> {
    fn signature(&self) -> StaticCon<S> {
        let (args, result) = match *self {
            Self::Not => (vec![Sort::Bool], Sort::Bool),
            Self::And | Self::Or | Self::Implies => (vec![Sort::Bool, Sort::Bool], Sort::Bool),
            Self::Equal(sort) => (vec![sort, sort], Sort::Bool),
            Self::BoolOf => (vec![Sort::Bool], Sort::Type),
        };
        StaticCon { args, result }
    }

    fn eval(&self, args: &[SortValue<S::T>]) -> Option<SortValue<S::T>> {
        let value = match (self, args) {
            (Self::Not, [p]) => !p.as_bool()?,
            (Self::And, [p, q]) => p.as_bool()? && q.as_bool()?,
            (Self::Or, [p, q]) => p.as_bool()? || q.as_bool()?,
            (Self::Implies, [p, q]) => !p.as_bool()? || q.as_bool()?,
            // Both arguments have sort `σ`, so their values compare directly
            (Self::Equal(_), [a, b]) => a == b,
            // `bool(B)` is a type, not a value
            (Self::BoolOf, _) => return None,
            _ => unreachable!("`eval` takes one value per argument of `signature`"),
        };
        Some(SortValue::Bool(value))
    }
}

impl<S> fmt::Display for BoolStaticCon<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Not => "¬",
            Self::And => "∧",
            Self::Or => "∨",
            Self::Implies => "⇒",
            Self::Equal(_) => "=",
            Self::BoolOf => "bool",
        })
    }
}
