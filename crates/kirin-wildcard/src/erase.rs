use kirin::prelude::*;

/// Erases a matched statement, named by its position in the pattern body.
///
/// The statement is named by position rather than by one of its results
/// because kirin-ir statements take only SSA values as arguments, so a
/// statement without results could not be referenced otherwise.
///
/// The statement's results must have no remaining uses when the erase runs:
/// preceding [`Replace`](crate::Replace)s should redirect them, or the pattern
/// should check they are unused. If a result is still used, the rewrite will
/// fail and the pass will quarantine the stage (see
/// [`Quarantined`](kirin::ir::Quarantined)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$erase {target}")]
#[kirin(builders, type = T)]
pub struct Erase<T: CompileTimeValue> {
    target: usize,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Erase<T> {
    /// The 0-based position, in the pattern body, of the statement to erase.
    pub fn target(&self) -> usize {
        self.target
    }
}
