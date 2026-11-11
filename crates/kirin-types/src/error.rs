//! Errors from checking terms.

use thiserror::Error;

use crate::{Sort, StaticTermId};

/// A static term that has no sort. `term` is where the error was found;
/// `shown` renders it for the message.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SortError<B> {
    #[error("bound variable {index} has no binder: it is under only {depth} binders")]
    UnboundVar {
        term: StaticTermId,
        index: usize,
        depth: usize,
    },
    #[error("`{shown}` expects {expected} arguments, but got {found}")]
    ArityMismatch {
        term: StaticTermId,
        shown: String,
        expected: usize,
        found: usize,
    },
    #[error("a product needs at least 2 components, but `{shown}` has {found}")]
    ShortProduct {
        term: StaticTermId,
        shown: String,
        found: usize,
    },
    #[error("`{shown}` has sort `{found}`, but `{expected}` was expected")]
    SortMismatch {
        term: StaticTermId,
        shown: String,
        expected: Sort<B>,
        found: Sort<B>,
    },
    #[error(
        "`{shown}` evaluates to a value of sort `{found}`, but its signature's result is `{expected}`"
    )]
    EvalSort {
        term: StaticTermId,
        shown: String,
        expected: Sort<B>,
        found: Sort<B>,
    },
}
