use kirin::prelude::*;

use crate::OpRef;

/// Terminates a [`Pattern`](crate::Pattern) body, naming the root of the
/// match and holding the replacement actions in its body.
///
/// The root is held as an [`OpRef`] rather than through one of its results,
/// so an op without results can be the root too.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, PrettyPrint)]
#[chumsky(format = "$rewrite {root} {body}")]
#[kirin(terminator, builders, type = T)]
pub struct Rewrite<T: CompileTimeValue> {
    #[kirin(into)]
    root: OpRef,
    body: Block,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Rewrite<T> {
    /// The matched op the match is rooted at, a statement of the pattern
    /// body.
    pub fn root(&self) -> Statement {
        self.root.0
    }

    /// The replacement actions, run in order.
    pub fn body(&self) -> Block {
        self.body
    }
}
