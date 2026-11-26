use kirin::prelude::*;

use crate::{Call, CallLike, Return};

/// How [`inline_calls`](super::inline_calls) recognizes this crate's call and
/// return statements in a language that wraps them, at any depth.
///
/// A language implements it by matching its own variants, for example
/// `HighLevel::Lexical(Lexical::Call(call)) => Some(call)`.
pub trait FunctionOps<T: CompileTimeValue> {
    /// The call `self` is, if it is one.
    fn as_call(&self) -> Option<&Call<T>>;
    /// The return `self` is, if it is one.
    fn as_return(&self) -> Option<&Return<T>>;
}

/// A call [`inline_calls`] inlines.
pub(super) struct InlinableCall {
    pub(super) target: SpecializedFunction,
    pub(super) args: Vec<SSAValue>,
    pub(super) results: Vec<SSAValue>,
}

/// `stmt` as a call to a specialized function in this stage, if it is one.
pub(super) fn inlinable_call<L, T>(stage: &StageInfo<L>, stmt: Statement) -> Option<InlinableCall>
where
    L: Dialect + FunctionOps<T>,
    T: CompileTimeValue,
{
    let Call::Specialized(call) = stmt.expect_info(stage).definition().as_call()? else {
        return None;
    };
    if call
        .stage()
        .is_some_and(|other| Some(other) != stage.stage_id())
    {
        return None;
    }
    Some(InlinableCall {
        target: call.target(),
        args: call.arguments().copied().collect(),
        results: call.results().map(|&result| result.into()).collect(),
    })
}
