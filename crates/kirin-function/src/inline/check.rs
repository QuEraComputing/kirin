use kirin::ir::walk::{PreOrder, WalkOrder, WalkResult};
use kirin::prelude::*;

use super::ops::InlinableCall;
use super::{FunctionOps, InlineError, NotInlined};

/// The callee of a call that can be inlined, read before any edit.
pub(super) struct Callee {
    pub(super) body: CFG,
    pub(super) blocks: Vec<Block>,
    pub(super) entry: Block,
    pub(super) entry_args: Vec<SSAValue>,
    /// Every return of the callee: each is the terminator of one of its
    /// blocks.
    pub(super) returns: Vec<Statement>,
}

/// What [`check_call`] found a call to be.
pub(super) enum Checked {
    /// The call can be inlined: its callee.
    Inline(Callee),
    /// The call is left as a call.
    Skip(NotInlined),
}

/// Whether `stmt`, the call `call`, can be inlined. Fails if the call does
/// not match its callee. Edits nothing.
pub(super) fn check_call<L, T>(
    stage: &StageInfo<L>,
    stmt: Statement,
    call: &InlinableCall,
) -> Result<Checked, InlineError>
where
    L: Dialect + HasCallableBody + FunctionOps<T>,
    T: CompileTimeValue,
{
    let definition = call
        .target
        .get_info(stage)
        .map(|info| *info.definition())
        .filter(|definition| definition.get_info(stage).is_some())
        .ok_or(InlineError::UnknownCallee(stmt))?;
    let Some(Body::CFG(body)) = definition.expect_info(stage).definition().callable_body() else {
        return Ok(Checked::Skip(NotInlined::CalleeBodyNotCfg(stmt)));
    };
    let blocks: Vec<Block> = body.blocks(stage).collect();
    let &[entry, ..] = blocks.as_slice() else {
        return Ok(Checked::Skip(NotInlined::EmptyCalleeBody(stmt)));
    };
    let entry_args: Vec<SSAValue> = entry
        .expect_info(stage)
        .arguments()
        .iter()
        .map(|&arg| arg.into())
        .collect();
    if entry_args.len() != call.args.len() {
        return Err(InlineError::ArgumentCountMismatch {
            call: stmt,
            parameters: entry_args.len(),
            arguments: call.args.len(),
        });
    }
    let returns = match callee_returns::<L, T>(stage, stmt, definition, body) {
        Ok(returns) => returns,
        Err(reason) => return Ok(Checked::Skip(reason)),
    };
    for &ret in &returns {
        let returned = ret.expect_info(stage).definition().arguments().count();
        if returned != call.results.len() {
            return Err(InlineError::ReturnCountMismatch {
                call: stmt,
                results: call.results.len(),
                returned,
            });
        }
    }
    Ok(Checked::Inline(Callee {
        body,
        blocks,
        entry,
        entry_args,
        returns,
    }))
}

/// Every return of `body`, the body of `definition`, the callee of `call`,
/// in walk order. Fails with [`NotInlined::NestedReturn`] if a return that
/// exits the callee is not the terminator of one of `body`'s blocks. The
/// bodies of callables nested in the callee are not walked: their returns
/// exit them.
fn callee_returns<L, T>(
    stage: &StageInfo<L>,
    call: Statement,
    definition: Statement,
    body: CFG,
) -> Result<Vec<Statement>, NotInlined>
where
    L: Dialect + HasCallableBody + FunctionOps<T>,
    T: CompileTimeValue,
{
    let mut returns = Vec::new();
    let mut nested = None;
    // Pre-order, so a nested callable is visited before its body and
    // `Skip` can leave that body out; it steps through the IR's links,
    // with no queue to allocate.
    PreOrder::walk(stage, definition, |stage, stmt| {
        if stmt == definition {
            return WalkResult::Advance;
        }
        let op = stmt.expect_info(stage).definition();
        if op.callable_body().is_some() {
            return WalkResult::Skip;
        }
        if op.as_return().is_none() {
            return WalkResult::Advance;
        }
        let in_callee_block = matches!(
            *stmt.parent(stage),
            Some(StatementParent::Block(block))
                if block.expect_info(stage).parent() == Some(BlockParent::CFG(body))
        );
        if in_callee_block {
            returns.push(stmt);
            WalkResult::Advance
        } else {
            nested = Some(stmt);
            WalkResult::Interrupt
        }
    });
    match nested {
        Some(ret) => Err(NotInlined::NestedReturn { call, ret }),
        None => Ok(returns),
    }
}
