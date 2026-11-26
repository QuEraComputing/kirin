use std::fmt;

use kirin::prelude::*;

/// Why [`inline_calls`](super::inline_calls) left a call as a call: the IR is
/// valid, but this inliner does not handle it. Each names the call, which is
/// left untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotInlined {
    /// The callee has no body, or a body that is not a CFG.
    CalleeBodyNotCfg(Statement),
    /// The callee's body is a CFG with no blocks: there is nothing to inline.
    EmptyCalleeBody(Statement),
    /// `ret`, a return that exits the callee of `call`, is nested in a
    /// statement's body, where it cannot be turned into a branch to the code
    /// after the call.
    NestedReturn { call: Statement, ret: Statement },
}

impl fmt::Display for NotInlined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotInlined::CalleeBodyNotCfg(call) => {
                write!(f, "the callee of {call:?} has no CFG body")
            }
            NotInlined::EmptyCalleeBody(call) => {
                write!(f, "the callee of {call:?} has an empty body")
            }
            NotInlined::NestedReturn { call, ret } => write!(
                f,
                "the callee of {call:?} has a return, {ret:?}, nested in a statement's body"
            ),
        }
    }
}
