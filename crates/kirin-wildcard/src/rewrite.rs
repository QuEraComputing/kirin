use kirin::prelude::*;

/// Terminates a [`Pattern`](crate::Pattern) body, naming the root of the
/// match by its position in the pattern body and holding the replacement
/// actions in its body.
///
/// The root is named by position rather than by one of its results
/// because kirin-ir statements take only SSA values as arguments, so a
/// statement without results could not be referenced otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[chumsky(format = "$rewrite {root} {body}")]
#[kirin(terminator, builders, type = T)]
pub struct Rewrite<T: CompileTimeValue> {
    root: usize,
    body: Block,
    #[kirin(default)]
    marker: std::marker::PhantomData<T>,
}

impl<T: CompileTimeValue> Rewrite<T> {
    /// The 0-based position, in the pattern body, of the statement the match
    /// is rooted at.
    pub fn root(&self) -> usize {
        self.root
    }

    /// The replacement actions, run in order.
    pub fn body(&self) -> Block {
        self.body
    }
}
