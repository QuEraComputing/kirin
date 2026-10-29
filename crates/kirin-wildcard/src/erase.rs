use kirin::prelude::*;

use crate::OpRef;

/// Erases a matched op.
///
/// The op is held as an [`OpRef`] rather than through one of its results, so
/// an op without results can be erased too.
///
/// The op's results must have no remaining uses when the erase runs:
/// preceding [`Replace`](crate::Replace)s should redirect them, or the pattern
/// should check they are unused. If a result is still used, the rewrite will
/// fail and the pass will quarantine the stage (see
/// [`Quarantined`](kirin::ir::Quarantined)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$erase {target}")]
#[kirin(builders, type = T)]
pub struct Erase<T: CompileTimeValue> {
    #[kirin(into)]
    target: OpRef,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Erase<T> {
    /// The matched op to erase, a statement of the pattern body.
    pub fn target(&self) -> Statement {
        self.target.0
    }
}
